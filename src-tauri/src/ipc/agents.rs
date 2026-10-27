//! SCR-09 Agent Studio commands. The engine lives in `crate::agents`; this
//! layer gives it the database, the log tabs (as read-only terminals in the
//! process manager) and the change events.
//!
//! Every command is async: the engine spawns tokio tasks, which needs the
//! runtime a sync command does not run on.

use super::commands::{buffer_bytes, StatusEvent};
use super::{events, AppState};
use crate::agents::adapter::{self, Cli};
use crate::agents::engine::{
    AgentEnv, AiPush, AttachPr, Engine, NewRun, PendingPush, PrRepo, PrRequest, PrText, QaAnswer,
    RunDetail, Verification,
};
use crate::agents::git;
use crate::agents::log::AgentLog;
use crate::agents::models::{self, ModelInfo};
use crate::agents::workspace;
use crate::clock::now_unix;
use crate::db::repos::agents::{
    self as repo, task_status, CheckProposal, CliProfile, Limits, Project, ProjectSettings,
    RoleCli, Run, TaskDraft, Usage,
};
use crate::db::Db;
use crate::error::{Error, Result};
use crate::process::manager::Live;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub struct AgentsState {
    pub engine: Arc<Engine>,
    /// Model lists read from the CLIs, until asked to read them again.
    models: tokio::sync::Mutex<std::collections::HashMap<&'static str, Vec<ModelInfo>>>,
}

impl AgentsState {
    pub fn new(app: &AppHandle) -> Self {
        let state = app.state::<AppState>();
        // Nothing drives runs left behind by the last session.
        let now = now_unix();
        let _ = state.db.write(|c| {
            repo::pause_orphaned_runs(c, &now)?;
            repo::close_orphaned_invocations(c, &now)
        });
        Self {
            engine: Engine::new(Arc::new(TauriAgentEnv { app: app.clone() })),
            models: Default::default(),
        }
    }
}

struct TauriAgentEnv {
    app: AppHandle,
}

const AGENT_SESSION: &str = "agent-session";
const AGENT_GROUP: &str = "agent";

impl AgentEnv for TauriAgentEnv {
    fn db(&self) -> &Db {
        &self.app.state::<AppState>().inner().db
    }

    fn data_dir(&self) -> PathBuf {
        crate::app::data_dir(&self.app).unwrap_or_else(|_| std::env::temp_dir())
    }

    fn open_log(&self, id: &str, _title: &str) -> Arc<AgentLog> {
        let state = self.app.state::<AppState>();
        let log = Arc::new(AgentLog::new(
            id.to_string(),
            buffer_bytes(&state),
            Some(state.ipc.clone()),
        ));
        state.processes.reserve(id);
        state.processes.insert(Live {
            run_event_id: id.to_string(),
            command_id: id.to_string(),
            session_id: AGENT_SESSION.into(),
            group_id: AGENT_GROUP.into(),
            pid: 0,
            pty: log.clone(),
            shell_kind: "agent".into(),
            history_level: 0,
        });
        self.status(id, "running", None);
        log
    }

    fn close_log(&self, id: &str, ok: bool) {
        self.app.state::<AppState>().processes.remove(id);
        self.status(
            id,
            if ok { "success" } else { "failed" },
            Some(if ok { 0 } else { 1 }),
        );
    }

    fn changed(&self, run_id: &str) {
        let _ = self.app.emit(
            events::AGENTS_CHANGED,
            serde_json::json!({ "run_id": run_id }),
        );
    }
}

impl TauriAgentEnv {
    fn status(&self, id: &str, status: &str, exit_code: Option<i32>) {
        let _ = self.app.emit(
            events::PROCESS_STATUS,
            StatusEvent {
                run_event_id: id.into(),
                command_id: id.into(),
                session_id: AGENT_SESSION.into(),
                status: status.into(),
                exit_code,
                pid: None,
            },
        );
    }
}

fn engine(app: &AppHandle) -> Arc<Engine> {
    Arc::clone(&app.state::<AgentsState>().engine)
}

fn db(app: &AppHandle) -> &Db {
    &app.state::<AppState>().inner().db
}

// ---- CLIs -----------------------------------------------------------------

#[derive(Serialize)]
pub struct CliInfo {
    pub cli: &'static str,
    pub path: Option<String>,
    pub version: Option<String>,
    pub profile: CliProfile,
}

async fn version_of(program: &Path) -> Option<String> {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(program)
            .arg("--version")
            .env("PATH", adapter::child_path(program))
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().find(|line| !line.trim().is_empty())?;
    Some(adapter::shorten(line.trim(), 60))
}

/// Installed CLIs with their versions and saved profiles ("Đội & CLI").
#[tauri::command(rename_all = "snake_case")]
pub async fn agents_detect(app: AppHandle) -> Result<Vec<CliInfo>> {
    let mut out = Vec::new();
    for cli in Cli::ALL {
        let path = adapter::find(cli);
        let version = match &path {
            Some(path) => version_of(path).await,
            None => None,
        };
        let profile = db(&app).read(|c| repo::profile_get(c, cli.as_str()))?;
        out.push(CliInfo {
            cli: cli.as_str(),
            path: path.map(|path| path.to_string_lossy().into_owned()),
            version,
            profile,
        });
    }
    Ok(out)
}

/// The models `cli` offers, for suggestions; read once, then from memory
/// unless `refresh`.
#[tauri::command(rename_all = "snake_case")]
pub async fn agents_models(app: AppHandle, cli: String, refresh: bool) -> Result<Vec<ModelInfo>> {
    let cli = Cli::parse(&cli)?;
    let state = app.state::<AgentsState>();
    // Held while reading: several fields asking at once read the CLI once.
    let mut cache = state.models.lock().await;
    if !refresh {
        if let Some(models) = cache.get(cli.as_str()) {
            return Ok(models.clone());
        }
    }
    let program =
        adapter::find(cli).ok_or_else(|| Error::msg(format!("Chưa cài {}", cli.as_str())))?;
    let models = models::list(cli, &program).await?;
    cache.insert(cli.as_str(), models.clone());
    Ok(models)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_profile_save(app: AppHandle, profile: CliProfile) -> Result<CliProfile> {
    db(&app).write(|c| {
        repo::profile_save(c, &profile)?;
        repo::profile_get(c, &profile.cli)
    })
}

// ---- projects -------------------------------------------------------------

#[derive(Serialize)]
pub struct RepoStatus {
    /// Relative to the project folder, `.` for the folder itself.
    pub path: String,
    /// Checked-out branch; None when detached or not a repo any more.
    pub branch: Option<String>,
    /// Tracked files with uncommitted changes.
    pub dirty: usize,
    pub is_repo: bool,
}

#[derive(Serialize)]
pub struct ProjectView {
    #[serde(flatten)]
    pub project: Project,
    /// Live git state of each repo, in the order of `project.repos`.
    pub status: Vec<RepoStatus>,
}

async fn project_view(project: Project) -> ProjectView {
    let mut status = Vec::new();
    for repo in &project.repos {
        let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
        let is_repo = git::repo_root(&root).await.is_ok();
        let (branch, dirty) = if is_repo {
            (
                git::current_branch(&root).await.ok().flatten(),
                git::dirty_files(&root).await.map(|f| f.len()).unwrap_or(0),
            )
        } else {
            (None, 0)
        };
        status.push(RepoStatus {
            path: repo.path.clone(),
            branch,
            dirty,
            is_repo,
        });
    }
    ProjectView { project, status }
}

/// Opens a folder in Agent Studio: a git repo (its root), or a folder that
/// holds several repos (fe/, be/, docs/...).
#[tauri::command(rename_all = "snake_case")]
pub async fn agents_project_open(app: AppHandle, path: String) -> Result<ProjectView> {
    let (root, repos) = workspace::detect(Path::new(path.trim())).await?;
    let root = root.to_string_lossy().into_owned();
    let id = uuid::Uuid::new_v4().to_string();
    let project = db(&app).write(|c| repo::project_open(c, &id, &root, &repos, &now_unix()))?;
    Ok(project_view(project).await)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_project_list(app: AppHandle) -> Result<Vec<Project>> {
    db(&app).read(repo::project_list)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_project_get(app: AppHandle, project_id: String) -> Result<ProjectView> {
    let project = db(&app).read(|c| repo::project_get(c, &project_id))?;
    Ok(project_view(project).await)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_project_save(
    app: AppHandle,
    project_id: String,
    settings: ProjectSettings,
) -> Result<ProjectView> {
    let settings = repo::normalize_settings(settings)?;
    let project = db(&app).write(|c| {
        repo::project_save(c, &project_id, &settings)?;
        repo::project_get(c, &project_id)
    })?;
    Ok(project_view(project).await)
}

// ---- runs -----------------------------------------------------------------

#[derive(Serialize)]
pub struct RunSummary {
    #[serde(flatten)]
    pub run: Run,
    pub tasks_total: usize,
    pub tasks_done: usize,
    pub usage: Usage,
    pub driving: bool,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_list(app: AppHandle, project_id: String) -> Result<Vec<RunSummary>> {
    let engine = engine(&app);
    let runs = db(&app).read(|c| {
        repo::run_list(c, &project_id)?
            .into_iter()
            .map(|run| {
                let tasks = repo::task_list(c, &run.id)?;
                let usage = repo::usage(c, &run.id)?;
                Ok((run, tasks, usage))
            })
            .collect::<Result<Vec<_>>>()
    })?;
    Ok(runs
        .into_iter()
        .map(|(run, tasks, usage)| RunSummary {
            driving: engine.is_driving(&run.id),
            tasks_total: tasks.len(),
            tasks_done: tasks
                .iter()
                .filter(|t| task_status::is_settled(&t.status))
                .count(),
            usage,
            run,
        })
        .collect())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_get(app: AppHandle, run_id: String) -> Result<RunDetail> {
    engine(&app).detail(&run_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_start(app: AppHandle, run: NewRun) -> Result<Run> {
    engine(&app).start_run(run).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_pause(app: AppHandle, run_id: String) -> Result<()> {
    engine(&app).pause(&run_id);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_stop(app: AppHandle, run_id: String) -> Result<()> {
    engine(&app).stop(&run_id);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_resume(app: AppHandle, run_id: String) -> Result<()> {
    engine(&app).resume(&run_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_set_limits(app: AppHandle, run_id: String, limits: Limits) -> Result<()> {
    engine(&app).set_limits(&run_id, limits)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_update(
    app: AppHandle,
    run_id: String,
    title: Option<String>,
    request: Option<String>,
) -> Result<Run> {
    engine(&app).set_info(&run_id, title, request)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_plan_approve(
    app: AppHandle,
    run_id: String,
    design: String,
    contract: String,
    tasks: Vec<TaskDraft>,
    checks: Option<Vec<CheckProposal>>,
) -> Result<()> {
    engine(&app).approve_plan(&run_id, design, contract, tasks, checks.unwrap_or_default())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_plan_revise(
    app: AppHandle,
    run_id: String,
    design: String,
    feedback: String,
) -> Result<()> {
    engine(&app).revise_plan(&run_id, design, feedback)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_merge(app: AppHandle, run_id: String) -> Result<()> {
    engine(&app).merge_into_base(&run_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_keep(app: AppHandle, run_id: String) -> Result<()> {
    engine(&app).keep_branch(&run_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_set_models(
    app: AppHandle,
    run_id: String,
    leader_model: Option<String>,
    leader_effort: Option<String>,
    roles: Vec<RoleCli>,
) -> Result<()> {
    engine(&app).set_models(&run_id, leader_model, leader_effort, roles)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_tasks_reassign(
    app: AppHandle,
    run_id: String,
    task_ids: Vec<String>,
    cli: String,
    model: Option<String>,
    effort: Option<String>,
    timeout_minutes: Option<u32>,
) -> Result<()> {
    engine(&app).reassign_tasks(&run_id, &task_ids, &cli, model, effort, timeout_minutes)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_task_set_timeout(
    app: AppHandle,
    task_id: String,
    timeout_minutes: Option<u32>,
) -> Result<()> {
    engine(&app).set_task_timeout(&task_id, timeout_minutes)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_follow_up(
    app: AppHandle,
    run_id: String,
    request: String,
    push_pr: Option<bool>,
) -> Result<()> {
    engine(&app)
        .follow_up(&run_id, request, push_pr.unwrap_or(false))
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_pr_draft(app: AppHandle, run_id: String) -> Result<PrRequest> {
    engine(&app).pr_draft(&run_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_leader_chat(
    app: AppHandle,
    run_id: String,
    message: String,
) -> Result<repo::ChatMessage> {
    engine(&app).leader_chat(&run_id, message).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_research_answer(
    app: AppHandle,
    run_id: String,
    answers: Vec<QaAnswer>,
) -> Result<()> {
    engine(&app).answer_research(&run_id, answers)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_research_again(
    app: AppHandle,
    run_id: String,
    feedback: String,
    answers: Vec<QaAnswer>,
) -> Result<()> {
    engine(&app).research_again(&run_id, feedback, answers)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_fix_push(
    app: AppHandle,
    run_id: String,
    note: Option<String>,
) -> Result<()> {
    engine(&app).fix_push(&run_id, note.unwrap_or_default())
}

#[tauri::command(rename_all = "snake_case")]
pub fn agents_push_failure_dismiss(app: AppHandle, run_id: String) -> Result<()> {
    engine(&app).dismiss_push_failure(&run_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_pr_ai_draft(
    app: AppHandle,
    run_id: String,
    repos: Vec<PrRepo>,
    note: Option<String>,
) -> Result<PrText> {
    engine(&app)
        .pr_ai_draft(&run_id, repos, note.unwrap_or_default())
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_open_prs(
    app: AppHandle,
    run_id: String,
    request: PrRequest,
) -> Result<Run> {
    engine(&app).open_pull_requests(&run_id, request).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_attach_pr(app: AppHandle, run_id: String, attach: AttachPr) -> Result<Run> {
    engine(&app).attach_pull_request(&run_id, attach).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_pr_pending(app: AppHandle, run_id: String) -> Result<Vec<PendingPush>> {
    engine(&app).pending_pushes(&run_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_push_prs(app: AppHandle, run_id: String) -> Result<Run> {
    engine(&app).push_to_pull_requests(&run_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_ai_push(app: AppHandle, run_id: String, request: AiPush) -> Result<()> {
    engine(&app).ai_push(&run_id, request)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_human_submit(
    app: AppHandle,
    task_id: String,
    results: Vec<repo::CaseResult>,
    passed: bool,
    note: String,
    attachments: Vec<String>,
) -> Result<()> {
    engine(&app).submit_human_test(&task_id, results, passed, note, attachments)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_human_reprepare(app: AppHandle, task_id: String) -> Result<()> {
    engine(&app).reprepare_human(&task_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_human_chat(
    app: AppHandle,
    task_id: String,
    message: String,
) -> Result<repo::ChatMessage> {
    engine(&app).human_chat(&task_id, message).await
}

/// A screenshot, as base64 (pasted or picked in the page).
#[tauri::command(rename_all = "snake_case")]
pub async fn agents_human_attach(
    app: AppHandle,
    task_id: String,
    name: String,
    data: String,
) -> Result<String> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|error| Error::msg(format!("Ảnh không đọc được: {error}")))?;
    engine(&app).human_attach(&task_id, &name, &bytes)
}

/// A screenshot of a person's task, as base64.
#[tauri::command(rename_all = "snake_case")]
pub async fn agents_human_file(app: AppHandle, task_id: String, name: String) -> Result<String> {
    use base64::Engine as _;
    let bytes = engine(&app).human_file(&task_id, &name)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_verify_prs(app: AppHandle, run_id: String) -> Result<Verification> {
    engine(&app).verify_pull_requests(&run_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_discard(app: AppHandle, run_id: String) -> Result<()> {
    engine(&app).discard(&run_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_clean_worktrees(app: AppHandle, run_id: String) -> Result<()> {
    engine(&app).clean_worktrees(&run_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_delete(app: AppHandle, run_id: String) -> Result<()> {
    engine(&app).delete(&run_id).await
}

#[derive(Serialize)]
pub struct DiffStat {
    pub files: u64,
    pub added: u64,
    pub removed: u64,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_diffstat(app: AppHandle, run_id: String) -> Result<DiffStat> {
    let (files, added, removed) = engine(&app).run_diffstat(&run_id).await?;
    Ok(DiffStat {
        files,
        added,
        removed,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_run_diff(app: AppHandle, run_id: String) -> Result<String> {
    engine(&app).run_diff(&run_id).await
}

// ---- tasks ----------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_task_retry(app: AppHandle, task_id: String) -> Result<()> {
    engine(&app).retry_task(&task_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_task_skip(app: AppHandle, task_id: String) -> Result<()> {
    engine(&app).skip_task(&task_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_task_rereview(app: AppHandle, task_id: String) -> Result<()> {
    engine(&app).rereview_task(&task_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_task_reassign(
    app: AppHandle,
    task_id: String,
    cli: String,
    model: Option<String>,
    effort: Option<String>,
) -> Result<()> {
    engine(&app).reassign_task(&task_id, &cli, model, effort)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_task_request_fix(
    app: AppHandle,
    task_id: String,
    feedback: String,
) -> Result<()> {
    engine(&app).request_fix(&task_id, &feedback)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_task_approve(app: AppHandle, task_id: String) -> Result<()> {
    engine(&app).approve_task(&task_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agents_task_diff(app: AppHandle, task_id: String) -> Result<String> {
    engine(&app).task_diff(&task_id).await
}

/// The log tab of a process, for "Xem log". A tab the process manager no
/// longer holds (an old process, or one from before a restart) is rebuilt
/// from the raw stream saved on disk.
#[tauri::command(rename_all = "snake_case")]
pub async fn agents_open_log(app: AppHandle, invocation_id: String) -> Result<String> {
    let state = app.state::<AppState>();
    let invocation = state.db.read(|c| {
        repo::invocation_get(c, &invocation_id)?
            .ok_or_else(|| Error::msg("Không tìm thấy lượt gọi agent"))
    })?;
    let id = invocation.terminal_id.clone();
    if state.processes.get(&id).is_some() || state.processes.finished(&id).is_some() {
        return Ok(id);
    }
    if invocation.finished_at.is_none() {
        // Between the database row and its tab: the tab comes in a moment.
        return Err(Error::msg("Agent đang khởi động"));
    }
    let cli = Cli::parse(&invocation.cli)?;
    let raw = crate::app::data_dir(&app)
        .map_err(|error| Error::msg(error.to_string()))?
        .join("agents")
        .join("runs")
        .join(&invocation.run_id)
        .join(format!("{}.jsonl", invocation.id));
    let text =
        std::fs::read_to_string(&raw).map_err(|_| Error::msg("Log của lượt gọi này không còn"))?;
    let log = Arc::new(AgentLog::new(id.clone(), buffer_bytes(&state), None));
    log.heading(&format!(
        "{} · {} (log đã lưu)",
        invocation.cli, invocation.step
    ));
    let mut parser = adapter::Parser::new(cli);
    for line in text.lines() {
        for entry in parser.feed(line) {
            log.line(&entry);
        }
    }
    match &invocation.error {
        Some(error) => log.result(false, error),
        None if invocation.ok == Some(true) => log.result(true, "Xong"),
        None => {}
    }
    // In and straight out: the manager keeps it as a finished terminal.
    state.processes.reserve(&id);
    state.processes.insert(Live {
        run_event_id: id.clone(),
        command_id: id.clone(),
        session_id: AGENT_SESSION.into(),
        group_id: AGENT_GROUP.into(),
        pid: 0,
        pty: log,
        shell_kind: "agent".into(),
        history_level: 0,
    });
    state.processes.remove(&id);
    Ok(id)
}
