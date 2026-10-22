//! The run state machine (SCR-09 §3). One driver task per active run walks
//! the stages; it stops at a gate, on something only a person can decide,
//! when paused, or when the run is done. Everything it decides is in the
//! database first, so a driver can be started again at any point (after a
//! pause, an error or an app restart) and pick up where the last one left.
//!
//! Phase 1: tasks run one at a time, in dependency order.

use super::adapter::{self, Access, Cli, Outcome};
use super::git::{self, MergeError};
use super::log::AgentLog;
use super::process::{self, RunError, Spawn};
use super::prompts;
use super::workspace;
use crate::clock::now_unix;
use crate::db::repos::agents::{
    self as repo, run_status, stage, task_status, CheckResult, Limits, Project, Review, RoleCli,
    Run, Task, TaskDraft,
};
use crate::db::Db;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::watch;

const CHECK_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// What the engine needs from the app.
pub trait AgentEnv: Send + Sync + 'static {
    fn db(&self) -> &Db;
    /// Worktrees, schema files and raw logs live under `<data_dir>/agents`.
    fn data_dir(&self) -> PathBuf;
    fn find_cli(&self, cli: Cli) -> Option<PathBuf> {
        adapter::find(cli)
    }
    /// A read-only terminal tab for one process.
    fn open_log(&self, id: &str, title: &str) -> Arc<AgentLog>;
    fn close_log(&self, id: &str, ok: bool);
    /// Something about the run changed: the UI reloads it.
    fn changed(&self, run_id: &str);
}

/// "Lượt mới" (MOD-21).
#[derive(Debug, Clone, Deserialize)]
pub struct NewRun {
    pub project_id: String,
    pub request: String,
    #[serde(default)]
    pub title: Option<String>,
    pub leader_cli: String,
    pub roles: Vec<RoleCli>,
    #[serde(default = "yes")]
    pub plan_gate: bool,
    #[serde(default = "yes")]
    pub merge_gate: bool,
    #[serde(default)]
    pub limits: Option<Limits>,
}

fn yes() -> bool {
    true
}

/// The leader's plan, as its schema has it.
#[derive(Debug, Deserialize)]
struct PlanAnswer {
    contract: String,
    tasks: Vec<PlanTask>,
}

#[derive(Debug, Deserialize)]
struct PlanTask {
    key: String,
    title: String,
    role: String,
    description: String,
    acceptance: Vec<String>,
    scope: Vec<String>,
    depends_on: Vec<String>,
    #[serde(default)]
    repos: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunDetail {
    pub run: Run,
    pub tasks: Vec<Task>,
    pub invocations: Vec<repo::Invocation>,
    pub usage: repo::Usage,
    pub driving: bool,
}

enum Flow {
    Continue,
    Stop,
}

enum StepError {
    Cancelled,
    Failed(String),
}

impl From<Error> for StepError {
    fn from(error: Error) -> Self {
        Self::Failed(error.to_string())
    }
}

type Step<T> = std::result::Result<T, StepError>;

struct Control {
    pause: AtomicBool,
    stop: watch::Sender<bool>,
}

pub struct Engine {
    env: Arc<dyn AgentEnv>,
    drivers: Mutex<HashMap<String, Arc<Control>>>,
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
}

impl Engine {
    pub fn new(env: Arc<dyn AgentEnv>) -> Arc<Self> {
        Arc::new(Self {
            env,
            drivers: Mutex::new(HashMap::new()),
        })
    }

    fn db(&self) -> &Db {
        self.env.db()
    }

    pub fn is_driving(&self, run_id: &str) -> bool {
        self.drivers
            .lock()
            .expect("agent drivers")
            .contains_key(run_id)
    }

    pub fn detail(&self, run_id: &str) -> Result<RunDetail> {
        let (run, tasks, invocations, usage) = self.db().read(|c| {
            Ok((
                repo::run_get(c, run_id)?,
                repo::task_list(c, run_id)?,
                repo::invocation_list(c, run_id)?,
                repo::usage(c, run_id)?,
            ))
        })?;
        Ok(RunDetail {
            run,
            tasks,
            invocations,
            usage,
            driving: self.is_driving(run_id),
        })
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.env.data_dir().join("agents").join("runs").join(run_id)
    }

    fn worktree_dir(&self, run_id: &str, name: &str) -> PathBuf {
        self.env
            .data_dir()
            .join("agents")
            .join("wt")
            .join(short(run_id))
            .join(name)
    }

    fn integration_dir(&self, run_id: &str) -> PathBuf {
        self.worktree_dir(run_id, "_integration")
    }

    fn update_run(&self, run_id: &str, edit: impl FnOnce(&mut Run)) -> Result<Run> {
        let run = self.db().write(|c| {
            let mut run = repo::run_get(c, run_id)?;
            edit(&mut run);
            run.updated_at = now_unix();
            repo::run_save(c, &run)?;
            Ok(run)
        })?;
        self.env.changed(run_id);
        Ok(run)
    }

    fn update_task(&self, task_id: &str, edit: impl FnOnce(&mut Task)) -> Result<Task> {
        let task = self.db().write(|c| {
            let mut task = repo::task_get(c, task_id)?;
            edit(&mut task);
            repo::task_save(c, &task)?;
            Ok(task)
        })?;
        self.env.changed(&task.run_id);
        Ok(task)
    }

    // ---- lifecycle --------------------------------------------------------

    /// Creates a run and starts its driver.
    pub async fn start_run(self: &Arc<Self>, new: NewRun) -> Result<Run> {
        let request = new.request.trim().to_string();
        if request.is_empty() {
            return Err(Error::msg("Hãy nhập yêu cầu"));
        }
        Cli::parse(&new.leader_cli)?;
        let mut roles: Vec<RoleCli> = Vec::new();
        for role in new.roles {
            let name = role.role.trim().to_lowercase();
            if name.is_empty() {
                continue;
            }
            Cli::parse(&role.cli)?;
            if roles.iter().any(|r| r.role == name) {
                return Err(Error::msg(format!("Vai bị trùng: {name}")));
            }
            roles.push(RoleCli {
                role: name,
                cli: role.cli,
            });
        }
        if roles.is_empty() {
            return Err(Error::msg("Hãy khai báo ít nhất một vai cho worker"));
        }
        let limits = new.limits.unwrap_or_default();
        if limits.max_rounds == 0 || limits.max_invocations == 0 || limits.timeout_minutes == 0 {
            return Err(Error::msg("Giới hạn phải lớn hơn 0"));
        }

        let project = self.db().read(|c| repo::project_get(c, &new.project_id))?;
        let mut repos = Vec::new();
        for config in &project.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &config.path);
            let label = workspace::label(&config.path);
            git::repo_root(&root)
                .await
                .map_err(|error| Error::msg(format!("{label}{error}")))?;
            let base_branch = match config.base_branch.clone() {
                Some(branch) => branch,
                None => git::current_branch(&root).await?.ok_or_else(|| {
                    Error::msg(format!(
                        "{label}Repo đang ở trạng thái detached HEAD; hãy checkout một nhánh"
                    ))
                })?,
            };
            let base_commit = git::commit_of(&root, &base_branch)
                .await
                .map_err(|error| Error::msg(format!("{label}{error}")))?;
            repos.push(repo::RunRepo {
                path: config.path.clone(),
                base_branch,
                base_commit,
            });
        }
        let Some(first) = repos.first().cloned() else {
            return Err(Error::msg("Dự án không có git repo nào"));
        };

        let id = uuid::Uuid::new_v4().to_string();
        let now = now_unix();
        let title = new
            .title
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| adapter::shorten(request.lines().next().unwrap_or(&request), 60));
        let run = Run {
            integration_branch: format!("cm/{}/integration", short(&id)),
            id,
            project_id: project.id,
            title,
            request,
            leader_cli: new.leader_cli,
            roles,
            plan_gate: new.plan_gate,
            merge_gate: new.merge_gate,
            limits,
            stage: stage::DESIGN.into(),
            status: run_status::RUNNING.into(),
            status_note: None,
            design_md: None,
            contract: None,
            leader_session: None,
            pending_feedback: None,
            base_branch: first.base_branch,
            base_commit: first.base_commit,
            repos,
            integration_checks: None,
            created_at: now.clone(),
            updated_at: now,
            finished_at: None,
        };
        self.db().write(|c| repo::run_insert(c, &run))?;
        self.drive(&run.id);
        Ok(run)
    }

    /// Starts the driver of a run unless one is already working on it.
    pub fn drive(self: &Arc<Self>, run_id: &str) {
        let control = {
            let mut drivers = self.drivers.lock().expect("agent drivers");
            if drivers.contains_key(run_id) {
                return;
            }
            let (stop, _) = watch::channel(false);
            let control = Arc::new(Control {
                pause: AtomicBool::new(false),
                stop,
            });
            drivers.insert(run_id.to_string(), Arc::clone(&control));
            control
        };
        let _ = self.update_run(run_id, |run| {
            run.status = run_status::RUNNING.into();
            run.status_note = None;
        });
        let engine = Arc::clone(self);
        let run_id = run_id.to_string();
        tokio::spawn(async move {
            engine.drive_loop(&run_id, &control).await;
            engine
                .drivers
                .lock()
                .expect("agent drivers")
                .remove(&run_id);
            engine.env.changed(&run_id);
        });
    }

    /// Stops after the current step.
    pub fn pause(&self, run_id: &str) {
        if let Some(control) = self.drivers.lock().expect("agent drivers").get(run_id) {
            control.pause.store(true, Ordering::SeqCst);
            let _ = self.update_run(run_id, |run| {
                run.status_note = Some("Sẽ dừng sau bước hiện tại".into());
            });
        }
    }

    /// Ends the running agent process now; its step is done again on resume.
    pub fn stop(&self, run_id: &str) {
        if let Some(control) = self.drivers.lock().expect("agent drivers").get(run_id) {
            control.pause.store(true, Ordering::SeqCst);
            control.stop.send_replace(true);
        }
    }

    /// Resumes a paused, failed or blocked run.
    pub fn resume(self: &Arc<Self>, run_id: &str) -> Result<()> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        match run.stage.as_str() {
            stage::PLAN_GATE => {
                return Err(Error::msg("Lượt chạy đang chờ duyệt kế hoạch (Cổng 1)"))
            }
            stage::MERGE_GATE => return Err(Error::msg("Lượt chạy đang chờ duyệt merge (Cổng 2)")),
            stage::DONE => return Err(Error::msg("Lượt chạy đã kết thúc")),
            _ => {}
        }
        self.drive(run_id);
        Ok(())
    }

    async fn wait_stopped(&self, run_id: &str) {
        for _ in 0..200 {
            if !self.is_driving(run_id) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn require_idle(&self, run_id: &str) -> Result<()> {
        if self.is_driving(run_id) {
            return Err(Error::msg("Lượt chạy đang chạy; hãy tạm dừng trước"));
        }
        Ok(())
    }

    async fn drive_loop(self: &Arc<Self>, run_id: &str, control: &Control) {
        loop {
            let run = match self.db().read(|c| repo::run_get(c, run_id)) {
                Ok(run) => run,
                Err(_) => return,
            };
            if control.pause.load(Ordering::SeqCst) {
                let _ = self.update_run(run_id, |run| {
                    run.status = run_status::PAUSED.into();
                    run.status_note = Some("Đã tạm dừng".into());
                });
                return;
            }
            let flow = match run.stage.as_str() {
                stage::DESIGN => self.design(&run, control).await,
                stage::PLAN => self.plan(&run, control).await,
                stage::PLAN_GATE => self.wait(&run, "Chờ duyệt thiết kế & kế hoạch (Cổng 1)"),
                stage::IMPLEMENT => self.implement(&run, control).await,
                stage::INTEGRATE => self.integrate(&run, control).await,
                stage::MERGE_GATE => self.wait(&run, "Chờ duyệt merge (Cổng 2)"),
                _ => Ok(Flow::Stop),
            };
            match flow {
                Ok(Flow::Continue) => continue,
                Ok(Flow::Stop) => return,
                Err(StepError::Cancelled) => {
                    let _ = self.update_run(run_id, |run| {
                        run.status = run_status::PAUSED.into();
                        run.status_note = Some("Đã dừng agent đang chạy".into());
                    });
                    return;
                }
                Err(StepError::Failed(message)) => {
                    let _ = self.update_run(run_id, |run| {
                        run.status = run_status::FAILED.into();
                        run.status_note = Some(message);
                    });
                    return;
                }
            }
        }
    }

    fn wait(&self, run: &Run, note: &str) -> Step<Flow> {
        self.update_run(&run.id, |run| {
            run.status = run_status::WAITING.into();
            run.status_note = Some(note.into());
        })?;
        Ok(Flow::Stop)
    }

    // ---- invoking CLIs ----------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    async fn invoke(
        &self,
        run: &Run,
        task: Option<&Task>,
        cli_name: &str,
        step_name: &str,
        title: &str,
        access: Access,
        cwd: &Path,
        resume: Option<&str>,
        prompt: String,
        schema: Option<&Value>,
        control: &Control,
    ) -> Step<Outcome> {
        let usage = self.db().read(|c| repo::usage(c, &run.id))?;
        if usage.invocations >= run.limits.max_invocations as u64 {
            return Err(StepError::Failed(format!(
                "Đã dùng hết {} lượt gọi agent của lượt chạy; tăng giới hạn để tiếp tục",
                run.limits.max_invocations
            )));
        }
        let cli = Cli::parse(cli_name)?;
        let program = self.env.find_cli(cli).ok_or_else(|| {
            StepError::Failed(format!(
                "Chưa cài {} (không tìm thấy {})",
                cli.as_str(),
                cli.binaries().join(" / ")
            ))
        })?;
        let profile = self.db().read(|c| repo::profile_get(c, cli_name))?;

        let dir = self.run_dir(&run.id);
        std::fs::create_dir_all(&dir).map_err(Error::from)?;
        let id = uuid::Uuid::new_v4().to_string();
        let schema_file = match schema {
            Some(schema) if cli.has_schema_flag() && cli != Cli::Claude => {
                let path = dir.join(format!("schema-{step_name}.json"));
                std::fs::write(&path, schema.to_string()).map_err(Error::from)?;
                Some(path)
            }
            _ => None,
        };
        let args = adapter::args(
            cli,
            &adapter::Request {
                prompt,
                cwd,
                access,
                resume,
                schema,
                schema_file: schema_file.as_deref(),
                profile: &profile,
            },
        );

        let terminal_id = format!("agent:{id}");
        let mut invocation = repo::Invocation {
            id: id.clone(),
            run_id: run.id.clone(),
            task_id: task.map(|task| task.id.clone()),
            cli: cli_name.to_string(),
            step: step_name.to_string(),
            session_id: resume.map(str::to_string),
            terminal_id: terminal_id.clone(),
            started_at: now_unix(),
            finished_at: None,
            ok: None,
            error: None,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: None,
        };
        self.db()
            .write(|c| repo::invocation_insert(c, &invocation))?;
        self.env.changed(&run.id);

        let log = self.env.open_log(&terminal_id, title);
        log.heading(title);
        log.info(&format!(
            "{} · {}{}",
            cwd.display(),
            match access {
                Access::ReadOnly => "chỉ đọc",
                Access::Write => "ghi trong worktree",
            },
            if profile.bypass_permissions && access == Access::Write {
                " · TỰ DUYỆT MỌI THỨ"
            } else {
                ""
            }
        ));
        let started = Instant::now();
        let result = process::run(
            Spawn {
                cli,
                program: &program,
                args,
                cwd,
                timeout: Duration::from_secs(run.limits.timeout_minutes as u64 * 60),
                raw_log: Some(dir.join(format!("{id}.jsonl"))),
            },
            &log,
            control.stop.subscribe(),
        )
        .await;

        invocation.finished_at = Some(now_unix());
        let elapsed = started.elapsed().as_secs();
        let outcome = match result {
            Ok(outcome) => {
                invocation.session_id = outcome.session_id.clone().or(invocation.session_id);
                invocation.input_tokens = outcome.input_tokens;
                invocation.output_tokens = outcome.output_tokens;
                invocation.cost_usd = outcome.cost_usd;
                invocation.ok = Some(outcome.error.is_none());
                invocation.error = outcome.error.clone();
                if outcome.denials > 0 {
                    log.info(&format!(
                        "{} thao tác bị từ chối vì thiếu quyền",
                        outcome.denials
                    ));
                }
                match &outcome.error {
                    None => log.result(true, &format!("Xong sau {elapsed}s")),
                    Some(error) => log.result(false, error),
                }
                Ok(outcome)
            }
            Err(error) => {
                invocation.ok = Some(false);
                invocation.error = Some(error.to_string());
                log.result(false, &error.to_string());
                Err(match error {
                    RunError::Cancelled => StepError::Cancelled,
                    RunError::TimedOut => StepError::Failed(format!(
                        "{} chạy quá {} phút",
                        cli.as_str(),
                        run.limits.timeout_minutes
                    )),
                    RunError::Failed(message) => StepError::Failed(message),
                })
            }
        };
        let _ = self.db().write(|c| repo::invocation_save(c, &invocation));
        self.env
            .close_log(&terminal_id, invocation.ok == Some(true));
        self.env.changed(&run.id);
        outcome
    }

    /// A leader step with a structured answer; one more try when the answer
    /// does not fit.
    #[allow(clippy::too_many_arguments)]
    async fn ask_json<T>(
        &self,
        run: &Run,
        task: Option<&Task>,
        step_name: &str,
        title: &str,
        cwd: &Path,
        resume: Option<&str>,
        prompt: String,
        schema: &Value,
        control: &Control,
        parse: impl Fn(Value) -> Result<T>,
    ) -> Step<(T, Outcome)> {
        let leader = run.leader_cli.clone();
        let mut outcome = self
            .invoke(
                run,
                task,
                &leader,
                step_name,
                title,
                Access::ReadOnly,
                cwd,
                resume,
                prompt,
                Some(schema),
                control,
            )
            .await?;
        for attempt in 0..2 {
            if let Some(error) = &outcome.error {
                return Err(StepError::Failed(format!("Leader ({leader}): {error}")));
            }
            let problem = match outcome.json() {
                Some(value) => match parse(value) {
                    Ok(answer) => return Ok((answer, outcome)),
                    Err(error) => error.to_string(),
                },
                None => "no JSON object found in the answer".to_string(),
            };
            let session = outcome.session_id.clone();
            if attempt == 1 || session.is_none() {
                return Err(StepError::Failed(format!(
                    "Leader trả kết quả không dùng được: {problem}"
                )));
            }
            outcome = self
                .invoke(
                    run,
                    task,
                    &leader,
                    step_name,
                    &format!("{title} (sửa JSON)"),
                    Access::ReadOnly,
                    cwd,
                    session.as_deref(),
                    prompts::fix_json(&problem),
                    Some(schema),
                    control,
                )
                .await?;
        }
        unreachable!("the loop returns")
    }

    // ---- stages -----------------------------------------------------------

    fn project(&self, run: &Run) -> Result<Project> {
        self.db().read(|c| repo::project_get(c, &run.project_id))
    }

    async fn design(&self, run: &Run, control: &Control) -> Step<Flow> {
        let project = self.project(run)?;
        let feedback = run
            .pending_feedback
            .clone()
            .filter(|_| run.leader_session.is_some());
        let (prompt, resume, title) = match &feedback {
            Some(feedback) => (
                prompts::redesign(run, feedback),
                run.leader_session.as_deref(),
                format!("{} · thiết kế lại", run.leader_cli),
            ),
            None => (
                prompts::design(run, &project),
                None,
                format!("{} · thiết kế", run.leader_cli),
            ),
        };
        let outcome = self
            .invoke(
                run,
                None,
                &run.leader_cli,
                "design",
                &title,
                Access::ReadOnly,
                Path::new(&project.path),
                resume,
                prompt,
                None,
                control,
            )
            .await?;
        if let Some(error) = outcome.error {
            return Err(StepError::Failed(format!(
                "Leader ({}): {error}",
                run.leader_cli
            )));
        }
        let design = outcome.text.trim().to_string();
        if design.is_empty() {
            return Err(StepError::Failed("Leader không trả về bản thiết kế".into()));
        }
        self.update_run(&run.id, |run| {
            run.design_md = Some(design);
            if outcome.session_id.is_some() {
                run.leader_session = outcome.session_id;
            }
            run.stage = stage::PLAN.into();
        })?;
        Ok(Flow::Continue)
    }

    fn role_cli(run: &Run, role: &str) -> String {
        run.roles
            .iter()
            .find(|r| r.role == role)
            .map(|r| r.cli.clone())
            .unwrap_or_else(|| run.leader_cli.clone())
    }

    async fn plan(&self, run: &Run, control: &Control) -> Step<Flow> {
        let project = self.project(run)?;
        let roles: Vec<String> = run.roles.iter().map(|r| r.role.clone()).collect();
        let repos: Vec<String> = run.repos.iter().map(|r| r.path.clone()).collect();
        let schema = prompts::plan_schema(&roles, &repos);
        let parse = |value: Value| -> Result<(String, Vec<TaskDraft>)> {
            let answer: PlanAnswer = serde_json::from_value(value)
                .map_err(|error| Error::msg(format!("plan does not match the schema: {error}")))?;
            let drafts: Vec<TaskDraft> = answer
                .tasks
                .into_iter()
                .map(|task| TaskDraft {
                    cli: Self::role_cli(run, &task.role),
                    key: task.key,
                    title: task.title,
                    role: task.role,
                    description: task.description,
                    acceptance: task.acceptance,
                    scope: task.scope,
                    depends_on: task.depends_on,
                    repos: task.repos,
                })
                .collect();
            let drafts = Self::fill_repos(run, drafts);
            Ok((answer.contract, repo::check_plan(&drafts, &repos)?))
        };
        let ((contract, drafts), outcome) = self
            .ask_json(
                run,
                None,
                "plan",
                &format!("{} · kế hoạch", run.leader_cli),
                Path::new(&project.path),
                run.leader_session.as_deref(),
                prompts::plan(run),
                &schema,
                control,
                parse,
            )
            .await?;
        self.replace_tasks(&run.id, &drafts)?;
        let next = if run.plan_gate {
            stage::PLAN_GATE
        } else {
            stage::IMPLEMENT
        };
        self.update_run(&run.id, |run| {
            run.contract = Some(contract);
            run.pending_feedback = None;
            if outcome.session_id.is_some() {
                run.leader_session = outcome.session_id;
            }
            run.stage = next.into();
        })?;
        Ok(Flow::Continue)
    }

    /// A one-repo project needs no repo in its tasks.
    fn fill_repos(run: &Run, mut drafts: Vec<TaskDraft>) -> Vec<TaskDraft> {
        if let [only] = run.repos.as_slice() {
            for draft in drafts.iter_mut().filter(|draft| draft.repos.is_empty()) {
                draft.repos = vec![only.path.clone()];
            }
        }
        drafts
    }

    fn replace_tasks(&self, run_id: &str, drafts: &[TaskDraft]) -> Result<()> {
        self.db().write(|c| {
            repo::task_delete_for_run(c, run_id)?;
            for (index, draft) in drafts.iter().enumerate() {
                repo::task_insert(
                    c,
                    &Task {
                        id: uuid::Uuid::new_v4().to_string(),
                        run_id: run_id.to_string(),
                        key: draft.key.trim().to_string(),
                        title: draft.title.trim().to_string(),
                        role: draft.role.trim().to_lowercase(),
                        cli: draft.cli.clone(),
                        description: draft.description.clone(),
                        acceptance: draft.acceptance.clone(),
                        scope: draft.scope.clone(),
                        depends_on: draft.depends_on.clone(),
                        repos: draft.repos.clone(),
                        status: task_status::WAITING.into(),
                        status_note: None,
                        round: 0,
                        worker_session: None,
                        branch: None,
                        worktree: None,
                        checks: None,
                        last_review: None,
                        added: 0,
                        removed: 0,
                        out_of_scope: Vec::new(),
                        sort_order: index as i64,
                    },
                )?;
            }
            Ok(())
        })?;
        self.env.changed(run_id);
        Ok(())
    }

    /// The integration branch checked out in its own worktree.
    /// The integration branch of every repo, checked out under one folder.
    async fn ensure_integration(&self, run: &Run, project: &Project) -> Result<PathBuf> {
        let dir = self.integration_dir(&run.id);
        for repo in &run.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            git::ensure_branch(&root, &run.integration_branch, &repo.base_commit).await?;
            git::ensure_worktree(
                &root,
                &workspace::repo_dir(&dir, &repo.path),
                &run.integration_branch,
            )
            .await?;
        }
        Ok(dir)
    }

    /// Commits what an agent left in every repo of a workspace. True when
    /// anything was committed.
    async fn commit_workspace(&self, run: &Run, base: &Path, message: &str) -> Result<bool> {
        let mut committed = false;
        for repo in &run.repos {
            let dir = workspace::repo_dir(base, &repo.path);
            if dir.join(".git").exists() {
                committed |= git::commit_all(&dir, message).await?;
            }
        }
        Ok(committed)
    }

    /// Files changed in a workspace since it left the integration branch
    /// (paths as seen from the project folder), and lines added / removed.
    async fn workspace_changes(&self, run: &Run, base: &Path) -> Result<(Vec<String>, u64, u64)> {
        let (mut files, mut added, mut removed) = (Vec::new(), 0, 0);
        for repo in &run.repos {
            let dir = workspace::repo_dir(base, &repo.path);
            if !dir.join(".git").exists() {
                continue;
            }
            for file in git::changed_files(&dir, &run.integration_branch).await? {
                files.push(workspace::project_path(&repo.path, &file));
            }
            let (a, r) = git::numstat(&dir, &run.integration_branch).await?;
            added += a;
            removed += r;
        }
        Ok((files, added, removed))
    }

    /// Repos of a workspace with changes since the integration branch.
    async fn changed_repos(&self, run: &Run, base: &Path) -> Result<Vec<String>> {
        let mut changed = Vec::new();
        for repo in &run.repos {
            let dir = workspace::repo_dir(base, &repo.path);
            if dir.join(".git").exists()
                && !git::changed_files(&dir, &run.integration_branch)
                    .await?
                    .is_empty()
            {
                changed.push(repo.path.clone());
            }
        }
        Ok(changed)
    }

    /// The diff of a workspace against the integration branch, all repos.
    async fn workspace_diff(&self, run: &Run, base: &Path) -> Result<String> {
        let mut out = String::new();
        for repo in &run.repos {
            let dir = workspace::repo_dir(base, &repo.path);
            if !dir.join(".git").exists() {
                continue;
            }
            let prefix = workspace::project_path(&repo.path, "");
            out.push_str(
                &git::diff_range(&dir, &format!("{}...HEAD", run.integration_branch), &prefix)
                    .await?,
            );
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        Ok(out)
    }

    async fn implement(&self, run: &Run, control: &Control) -> Step<Flow> {
        let project = self.project(run)?;
        let integration = self.ensure_integration(run, &project).await?;
        let tasks = self.db().read(|c| repo::task_list(c, &run.id))?;
        let settled = |key: &str| {
            tasks
                .iter()
                .any(|t| t.key == key && task_status::is_settled(&t.status))
        };
        let in_progress = [
            task_status::RUNNING,
            task_status::CHECKING,
            task_status::REVIEW,
            task_status::FIXING,
        ];
        let next = tasks
            .iter()
            .find(|t| in_progress.contains(&t.status.as_str()))
            .or_else(|| {
                tasks.iter().find(|t| {
                    (t.status == task_status::WAITING || t.status == task_status::READY)
                        && t.depends_on.iter().all(|dep| settled(dep))
                })
            });
        if let Some(task) = next {
            self.task_turn(run, &project, &integration, task, control)
                .await?;
            return Ok(Flow::Continue);
        }
        if tasks.iter().all(|t| task_status::is_settled(&t.status)) {
            self.update_run(&run.id, |run| run.stage = stage::INTEGRATE.into())?;
            return Ok(Flow::Continue);
        }
        let blocked = tasks
            .iter()
            .filter(|t| {
                [
                    task_status::FAILED,
                    task_status::CONFLICT,
                    task_status::NEEDS_HUMAN,
                ]
                .contains(&t.status.as_str())
            })
            .count();
        self.wait(
            run,
            &format!("{blocked} task cần người xem trước khi tiếp tục"),
        )
    }

    /// One step of a task: a worker turn, the checks, or a review.
    async fn task_turn(
        &self,
        run: &Run,
        project: &Project,
        integration: &Path,
        task: &Task,
        control: &Control,
    ) -> Step<()> {
        match task.status.as_str() {
            task_status::CHECKING => self.check_task(run, project, task, control).await,
            task_status::REVIEW => self.review_task(run, integration, task, control).await,
            _ => self.worker_turn(run, project, task, control).await,
        }
    }

    async fn ensure_task_worktree(
        &self,
        run: &Run,
        project: &Project,
        task: &Task,
        stop: watch::Receiver<bool>,
    ) -> Step<Task> {
        let branch = task
            .branch
            .clone()
            .unwrap_or_else(|| format!("cm/{}/{}", short(&run.id), task.key));
        let base = task
            .worktree
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.worktree_dir(&run.id, &task.key));
        let fresh = task.worktree.is_none();
        // Every repo is there, so the agent can read the others too.
        for repo in &run.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            let start = git::commit_of(&root, &run.integration_branch).await?;
            git::ensure_branch(&root, &branch, &start).await?;
            git::ensure_worktree(&root, &workspace::repo_dir(&base, &repo.path), &branch).await?;
        }
        let task = if fresh {
            self.update_task(&task.id, |task| {
                task.branch = Some(branch);
                task.worktree = Some(base.to_string_lossy().into_owned());
            })?
        } else {
            task.clone()
        };
        if !fresh {
            return Ok(task);
        }
        let setups: Vec<(String, String)> = task
            .repos
            .iter()
            .filter_map(|repo| {
                project
                    .repo(repo)
                    .and_then(|config| config.setup_cmd.clone())
                    .map(|cmd| (repo.clone(), cmd))
            })
            .collect();
        if setups.is_empty() {
            return Ok(task);
        }
        let id = format!("agent:setup-{}", uuid::Uuid::new_v4());
        let log = self.env.open_log(&id, &format!("chuẩn bị · {}", task.key));
        for (repo, setup) in setups {
            log.heading(&format!("{}$ {setup}", workspace::label(&repo)));
            let dir = workspace::repo_dir(&base, &repo);
            let result = process::run_shell(&setup, &dir, CHECK_TIMEOUT, &log, stop.clone()).await;
            let ok = matches!(result, Ok((true, _, _)));
            log.result(
                ok,
                if ok {
                    "Đã chuẩn bị worktree"
                } else {
                    "Lệnh chuẩn bị thất bại"
                },
            );
            match result {
                Ok((true, _, _)) => {}
                Err(RunError::Cancelled) => {
                    self.env.close_log(&id, false);
                    return Err(StepError::Cancelled);
                }
                Ok((false, code, _)) => {
                    self.env.close_log(&id, false);
                    return Err(StepError::Failed(format!(
                        "{}Lệnh chuẩn bị worktree \"{setup}\" thất bại (mã {})",
                        workspace::label(&repo),
                        code.map_or("?".into(), |c| c.to_string())
                    )));
                }
                Err(error) => {
                    self.env.close_log(&id, false);
                    return Err(StepError::Failed(error.to_string()));
                }
            }
        }
        self.env.close_log(&id, true);
        Ok(task)
    }

    async fn worker_turn(
        &self,
        run: &Run,
        project: &Project,
        task: &Task,
        control: &Control,
    ) -> Step<()> {
        let task = match self
            .ensure_task_worktree(run, project, task, control.stop.subscribe())
            .await
        {
            Ok(task) => task,
            Err(StepError::Failed(message)) => {
                self.update_task(&task.id, |task| {
                    task.status = task_status::FAILED.into();
                    task.status_note = Some(message);
                })?;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        let worktree = PathBuf::from(task.worktree.clone().unwrap_or_default());
        let fixing = task.status == task_status::FIXING;
        let before = task.status.clone();
        let earlier_work = !self.workspace_changes(run, &worktree).await?.0.is_empty();
        let review = task.last_review.clone();
        let (prompt, resume, step) = match (&review, &task.worker_session) {
            (Some(review), Some(session)) if fixing => (
                prompts::fix(review, task.checks.as_deref().unwrap_or(&[])),
                Some(session.clone()),
                "fix",
            ),
            // A new agent (reassigned) gets the whole task plus the review.
            (Some(review), None) if fixing => (
                format!(
                    "{}\n\n{}",
                    prompts::implement(run, project, &task, earlier_work),
                    prompts::fix(review, task.checks.as_deref().unwrap_or(&[]))
                ),
                None,
                "fix",
            ),
            _ => (
                prompts::implement(run, project, &task, earlier_work),
                task.worker_session.clone().filter(|_| earlier_work),
                "implement",
            ),
        };
        self.update_task(&task.id, |task| {
            task.status = task_status::RUNNING.into();
            task.status_note = None;
        })?;
        let title = format!("{} · {} · vòng {}", task.cli, task.key, task.round + 1);
        let outcome = match self
            .invoke(
                run,
                Some(&task),
                &task.cli,
                step,
                &title,
                Access::Write,
                &worktree,
                resume.as_deref(),
                prompt,
                None,
                control,
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(StepError::Cancelled) => {
                self.update_task(&task.id, |task| task.status = before)?;
                return Err(StepError::Cancelled);
            }
            Err(StepError::Failed(message)) => {
                self.update_task(&task.id, |task| {
                    task.status = task_status::FAILED.into();
                    task.status_note = Some(message);
                })?;
                return Ok(());
            }
        };
        if outcome.session_id.is_some() {
            self.update_task(&task.id, |task| {
                task.worker_session = outcome.session_id.clone()
            })?;
        }
        if let Some(error) = outcome.error {
            self.update_task(&task.id, |task| {
                task.status = task_status::FAILED.into();
                task.status_note = Some(error);
            })?;
            return Ok(());
        }
        let message = format!(
            "cm({}): {}{}",
            task.key,
            task.title,
            if task.round > 0 {
                format!(" (vòng {})", task.round + 1)
            } else {
                String::new()
            }
        );
        let committed = self.commit_workspace(run, &worktree, &message).await?;
        let (files, added, removed) = self.workspace_changes(run, &worktree).await?;
        if !committed && files.is_empty() {
            let hint = if outcome.denials > 0 {
                format!(
                    " ({} thao tác bị từ chối vì thiếu quyền — xem log hoặc cấp quyền trong Đội & CLI)",
                    outcome.denials
                )
            } else {
                String::new()
            };
            self.update_task(&task.id, |task| {
                task.status = task_status::FAILED.into();
                task.status_note = Some(format!("Agent không thay đổi tệp nào{hint}"));
            })?;
            return Ok(());
        }
        let outside = git::outside_scope(&task.scope, &files);
        self.update_task(&task.id, |task| {
            task.added = added;
            task.removed = removed;
            task.out_of_scope = outside;
            task.status = task_status::CHECKING.into();
        })?;
        Ok(())
    }

    /// Runs `(repo, command, folder)` checks in order, in one log tab.
    async fn run_checks(
        &self,
        jobs: &[(String, String, PathBuf)],
        title: &str,
        control: &Control,
    ) -> Step<Vec<CheckResult>> {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }
        let id = format!("agent:check-{}", uuid::Uuid::new_v4());
        let log = self.env.open_log(&id, title);
        let mut results = Vec::new();
        for (repo, command, cwd) in jobs {
            let shown = format!("{}{command}", workspace::label(repo));
            log.heading(&format!("$ {shown}"));
            let started = Instant::now();
            let result =
                process::run_shell(command, cwd, CHECK_TIMEOUT, &log, control.stop.subscribe())
                    .await;
            let (ok, exit_code, output) = match result {
                Ok(done) => done,
                Err(RunError::Cancelled) => {
                    self.env.close_log(&id, false);
                    return Err(StepError::Cancelled);
                }
                Err(error) => (false, None, error.to_string()),
            };
            log.result(ok, &shown);
            results.push(CheckResult {
                command: shown,
                ok,
                exit_code,
                output,
                duration_ms: started.elapsed().as_millis() as u64,
            });
        }
        self.env
            .close_log(&id, results.iter().all(|check| check.ok));
        Ok(results)
    }

    /// The checks of `repos`, to run in `base`.
    fn check_jobs(
        project: &Project,
        repos: &[String],
        base: &Path,
    ) -> Vec<(String, String, PathBuf)> {
        repos
            .iter()
            .flat_map(|repo| {
                project
                    .repo(repo)
                    .map(|config| config.check_cmds.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |cmd| (repo.clone(), cmd, workspace::repo_dir(base, repo)))
            })
            .collect()
    }

    async fn check_task(
        &self,
        run: &Run,
        project: &Project,
        task: &Task,
        control: &Control,
    ) -> Step<()> {
        let base = PathBuf::from(task.worktree.clone().unwrap_or_default());
        // The task's repos, and any other it changed after all.
        let mut repos = task.repos.clone();
        for repo in self.changed_repos(run, &base).await? {
            if !repos.contains(&repo) {
                repos.push(repo);
            }
        }
        let jobs = Self::check_jobs(project, &repos, &base);
        let checks = self
            .run_checks(&jobs, &format!("kiểm tra · {}", task.key), control)
            .await?;
        self.update_task(&task.id, |task| {
            task.checks = Some(checks);
            task.status = task_status::REVIEW.into();
        })?;
        Ok(())
    }

    async fn review_task(
        &self,
        run: &Run,
        integration: &Path,
        task: &Task,
        control: &Control,
    ) -> Step<()> {
        let worktree = PathBuf::from(task.worktree.clone().unwrap_or_default());
        let (diff, truncated) = git::cut(
            self.workspace_diff(run, &worktree).await?,
            prompts::REVIEW_DIFF_BYTES,
        );
        let checks = task.checks.clone().unwrap_or_default();
        let prompt = prompts::review(
            run,
            task,
            &diff,
            truncated,
            &run.integration_branch,
            &checks,
            &task.out_of_scope,
        );
        let parse = |value: Value| -> Result<Review> {
            let review: Review = serde_json::from_value(value).map_err(|error| {
                Error::msg(format!("review does not match the schema: {error}"))
            })?;
            if review.verdict != "approved" && review.verdict != "changes_requested" {
                return Err(Error::msg("verdict must be approved or changes_requested"));
            }
            Ok(review)
        };
        let (mut review, _) = self
            .ask_json(
                run,
                Some(task),
                "review",
                &format!(
                    "{} · review {} · vòng {}",
                    run.leader_cli,
                    task.key,
                    task.round + 1
                ),
                &worktree,
                None,
                prompt,
                &prompts::review_schema(),
                control,
                parse,
            )
            .await?;
        review.round = task.round + 1;
        // Failing checks are never approved, whatever the reviewer says.
        if review.verdict == "approved" && checks.iter().any(|check| !check.ok) {
            review.verdict = "changes_requested".into();
            review.summary = format!("Lệnh kiểm tra thất bại. {}", review.summary);
        }
        if review.verdict == "approved" {
            self.update_task(&task.id, |task| task.last_review = Some(review))?;
            return self.merge_task(run, integration, task).await;
        }
        let max_rounds = run.limits.max_rounds;
        self.update_task(&task.id, |task| {
            task.last_review = Some(review);
            if task.round + 1 >= max_rounds {
                task.status = task_status::NEEDS_HUMAN.into();
                task.status_note = Some(format!("Hết {max_rounds} vòng review mà chưa đạt"));
            } else {
                task.round += 1;
                task.status = task_status::FIXING.into();
            }
        })?;
        Ok(())
    }

    /// Merges the task branch into the integration branch, repo by repo
    /// (only repos where the task has commits).
    async fn merge_task(&self, run: &Run, integration: &Path, task: &Task) -> Step<()> {
        let branch = task.branch.clone().unwrap_or_default();
        let message = format!("cm: merge {} — {}", task.key, task.title);
        for repo in &run.repos {
            let dir = workspace::repo_dir(integration, &repo.path);
            if git::ahead(&dir, &run.integration_branch, &branch).await? == 0 {
                continue;
            }
            let note = match git::merge(&dir, &branch, &message).await {
                Ok(()) => continue,
                Err(MergeError::Conflict(files)) => format!(
                    "Xung đột merge: {}",
                    files
                        .iter()
                        .map(|file| workspace::project_path(&repo.path, file))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Err(MergeError::Failed(message)) => {
                    format!("{}{message}", workspace::label(&repo.path))
                }
            };
            self.update_task(&task.id, |task| {
                task.status = task_status::CONFLICT.into();
                task.status_note = Some(note);
            })?;
            return Ok(());
        }
        let project = self.project(run)?;
        if let Some(base) = &task.worktree {
            self.remove_workspace(run, &project, Path::new(base)).await;
        }
        self.update_task(&task.id, |task| {
            task.status = task_status::MERGED.into();
            task.status_note = None;
            task.worktree = None;
        })?;
        Ok(())
    }

    /// Removes the worktrees of a workspace and the folder holding them.
    async fn remove_workspace(&self, run: &Run, project: &Project, base: &Path) {
        for repo in &run.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            git::remove_worktree(&root, &workspace::repo_dir(base, &repo.path)).await;
        }
        if base.exists() {
            let _ = std::fs::remove_dir_all(base);
        }
    }

    async fn integrate(&self, run: &Run, control: &Control) -> Step<Flow> {
        let project = self.project(run)?;
        let integration = self.ensure_integration(run, &project).await?;
        // Repos the run changed.
        let mut repos = Vec::new();
        for repo in &run.repos {
            let dir = workspace::repo_dir(&integration, &repo.path);
            if git::ahead(&dir, &repo.base_commit, &run.integration_branch).await? > 0 {
                repos.push(repo.path.clone());
            }
        }
        let jobs = Self::check_jobs(&project, &repos, &integration);
        let checks = self
            .run_checks(&jobs, "kiểm tra · tích hợp", control)
            .await?;
        self.update_run(&run.id, |run| {
            run.integration_checks = Some(checks);
            run.stage = stage::MERGE_GATE.into();
        })?;
        if !run.merge_gate {
            if let Err(error) = self.merge_into_base(&run.id).await {
                return self.wait(run, &error.to_string());
            }
            return Ok(Flow::Stop);
        }
        Ok(Flow::Continue)
    }

    // ---- gates and actions ------------------------------------------------

    fn at_stage(&self, run_id: &str, wanted: &str) -> Result<Run> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.stage != wanted {
            return Err(Error::msg("Lượt chạy không ở bước này nữa"));
        }
        Ok(run)
    }

    /// Gate 1: the (edited) design and plan are approved.
    pub fn approve_plan(
        self: &Arc<Self>,
        run_id: &str,
        design: String,
        contract: String,
        drafts: Vec<TaskDraft>,
    ) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.at_stage(run_id, stage::PLAN_GATE)?;
        let repos: Vec<String> = run.repos.iter().map(|r| r.path.clone()).collect();
        let drafts = repo::check_plan(&Self::fill_repos(&run, drafts), &repos)?;
        self.replace_tasks(run_id, &drafts)?;
        self.update_run(run_id, |run| {
            run.design_md = Some(design);
            run.contract = Some(contract);
            run.stage = stage::IMPLEMENT.into();
        })?;
        self.drive(run_id);
        Ok(())
    }

    /// Gate 1: back to the leader with feedback.
    pub fn revise_plan(
        self: &Arc<Self>,
        run_id: &str,
        design: String,
        feedback: String,
    ) -> Result<()> {
        self.require_idle(run_id)?;
        self.at_stage(run_id, stage::PLAN_GATE)?;
        let feedback = feedback.trim().to_string();
        if feedback.is_empty() {
            return Err(Error::msg("Hãy nhập góp ý cho leader"));
        }
        self.update_run(run_id, |run| {
            run.design_md = Some(design);
            run.pending_feedback = Some(feedback);
            run.stage = stage::DESIGN.into();
        })?;
        self.drive(run_id);
        Ok(())
    }

    async fn cleanup_worktrees(&self, run: &Run, delete_branches: bool) -> Result<()> {
        let project = self.project(run)?;
        let tasks = self.db().read(|c| repo::task_list(c, &run.id))?;
        for task in &tasks {
            if let Some(base) = &task.worktree {
                self.remove_workspace(run, &project, Path::new(base)).await;
            }
            if let Some(branch) = &task.branch {
                // Merged work lives on in the integration branch.
                if delete_branches || task.status == task_status::MERGED {
                    for repo in &run.repos {
                        let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
                        git::delete_branch(&root, branch).await;
                    }
                }
            }
        }
        self.remove_workspace(run, &project, &self.integration_dir(&run.id))
            .await;
        let _ = std::fs::remove_dir(self.worktree_dir(&run.id, ""));
        self.db().write(|c| {
            for mut task in tasks {
                task.worktree = None;
                repo::task_save(c, &task)?;
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Gate 2: merge the integration branch into the base branch, in the
    /// user's checkout. Never pushes.
    pub async fn merge_into_base(&self, run_id: &str) -> Result<()> {
        let run = self.at_stage(run_id, stage::MERGE_GATE)?;
        let project = self.project(&run)?;
        // Every repo is checked before any is merged.
        let mut targets = Vec::new();
        for repo in &run.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            if git::ahead(&root, &repo.base_branch, &run.integration_branch).await? == 0 {
                continue;
            }
            let label = workspace::label(&repo.path);
            let current = git::current_branch(&root).await?;
            if current.as_deref() != Some(repo.base_branch.as_str()) {
                return Err(Error::msg(format!(
                    "{label}Repo đang ở nhánh {}; hãy chuyển về {} rồi merge",
                    current.as_deref().unwrap_or("(detached)"),
                    repo.base_branch
                )));
            }
            let dirty = git::dirty_files(&root).await?;
            if !dirty.is_empty() {
                return Err(Error::msg(format!(
                    "{label}Repo có thay đổi chưa commit ({}); hãy commit hoặc stash trước",
                    dirty.len()
                )));
            }
            targets.push((repo.path.clone(), root));
        }
        let mut merged: Vec<String> = Vec::new();
        for (repo, root) in &targets {
            if let Err(error) =
                git::merge(root, &run.integration_branch, &format!("cm: {}", run.title)).await
            {
                let done = if merged.is_empty() {
                    String::new()
                } else {
                    format!(" (đã merge: {})", merged.join(", "))
                };
                return Err(Error::msg(format!(
                    "{}{error}{done}",
                    workspace::label(repo)
                )));
            }
            merged.push(repo.clone());
        }
        self.cleanup_worktrees(&run, false).await?;
        self.update_run(run_id, |run| {
            run.stage = stage::DONE.into();
            run.status = run_status::MERGED.into();
            run.status_note = None;
            run.finished_at = Some(now_unix());
        })?;
        Ok(())
    }

    /// Gate 2: keep the integration branch, drop the worktrees.
    pub async fn keep_branch(&self, run_id: &str) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.at_stage(run_id, stage::MERGE_GATE)?;
        self.cleanup_worktrees(&run, false).await?;
        self.update_run(run_id, |run| {
            run.stage = stage::DONE.into();
            run.status = run_status::KEPT.into();
            run.status_note = Some(format!("Giữ nhánh {}", run.integration_branch));
            run.finished_at = Some(now_unix());
        })?;
        Ok(())
    }

    /// Ends the run and removes everything it created in the repo.
    pub async fn discard(&self, run_id: &str) -> Result<()> {
        self.stop(run_id);
        self.wait_stopped(run_id).await;
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.status == run_status::MERGED {
            return Err(Error::msg("Lượt chạy đã merge"));
        }
        self.cleanup_worktrees(&run, true).await?;
        let project = self.project(&run)?;
        if run.status != run_status::KEPT {
            for repo in &run.repos {
                let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
                git::delete_branch(&root, &run.integration_branch).await;
            }
        }
        self.update_run(run_id, |run| {
            run.stage = stage::DONE.into();
            run.status = run_status::DISCARDED.into();
            run.status_note = None;
            run.finished_at = Some(now_unix());
        })?;
        Ok(())
    }

    /// Forgets a finished run (its logs too).
    pub fn delete(&self, run_id: &str) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.stage != stage::DONE {
            return Err(Error::msg("Hãy bỏ lượt chạy trước khi xóa"));
        }
        self.db().write(|c| repo::run_delete(c, run_id))?;
        let _ = std::fs::remove_dir_all(self.run_dir(run_id));
        Ok(())
    }

    pub fn set_limits(&self, run_id: &str, limits: Limits) -> Result<()> {
        if limits.max_rounds == 0 || limits.max_invocations == 0 || limits.timeout_minutes == 0 {
            return Err(Error::msg("Giới hạn phải lớn hơn 0"));
        }
        self.update_run(run_id, |run| run.limits = limits)?;
        Ok(())
    }

    /// After a task action: a run that stopped on its tasks goes on.
    fn continue_after_task_action(self: &Arc<Self>, run_id: &str) -> Result<()> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.stage == stage::IMPLEMENT
            && (run.status == run_status::WAITING || run.status == run_status::FAILED)
        {
            self.drive(run_id);
        }
        Ok(())
    }

    fn task_for_action(&self, task_id: &str) -> Result<Task> {
        let task = self.db().read(|c| repo::task_get(c, task_id))?;
        self.require_idle(&task.run_id)?;
        let run = self.db().read(|c| repo::run_get(c, &task.run_id))?;
        if run.stage != stage::IMPLEMENT {
            return Err(Error::msg("Chỉ sửa được task trong bước triển khai"));
        }
        Ok(task)
    }

    /// "Thử lại": run the failed step again. A merge conflict first brings
    /// the integration branch into the task branch, then the worker
    /// resolves the conflict markers.
    pub async fn retry_task(self: &Arc<Self>, task_id: &str) -> Result<()> {
        let task = self.task_for_action(task_id)?;
        if task.status == task_status::CONFLICT {
            let run = self.db().read(|c| repo::run_get(c, &task.run_id))?;
            let project = self.project(&run)?;
            // Kept alive so the setup command is not taken as cancelled.
            let (_stop, stop) = watch::channel(false);
            let task = self
                .ensure_task_worktree(&run, &project, &task, stop)
                .await
                .map_err(|error| match error {
                    StepError::Failed(message) => Error::msg(message),
                    StepError::Cancelled => Error::msg("Đã dừng"),
                })?;
            let base = PathBuf::from(task.worktree.clone().unwrap_or_default());
            // Bring the integration branch into the task branch, every repo.
            let mut conflicts: Vec<String> = Vec::new();
            let mut failed = false;
            for repo in &run.repos {
                let dir = workspace::repo_dir(&base, &repo.path);
                let merged = git::git(
                    &dir,
                    &["merge", "--no-ff", "--no-commit", &run.integration_branch],
                )
                .await;
                let files = git::git(&dir, &["diff", "--name-only", "--diff-filter=U"])
                    .await
                    .unwrap_or_default();
                failed |= merged.is_err() && files.trim().is_empty();
                conflicts.extend(
                    files
                        .lines()
                        .map(|file| workspace::project_path(&repo.path, file)),
                );
            }
            if failed && conflicts.is_empty() {
                return Err(Error::msg("Không merge được nhánh tích hợp vào nhánh task"));
            }
            if conflicts.is_empty() {
                self.commit_workspace(&run, &base, &format!("cm({}): merge integration", task.key))
                    .await?;
                self.update_task(task_id, |task| {
                    task.status = task_status::REVIEW.into();
                    task.status_note = None;
                })?;
            } else {
                let review = Review {
                    verdict: "changes_requested".into(),
                    summary: format!(
                        "The integration branch was merged into your branch and has conflicts. \
                         Resolve every conflict marker, keeping both sides' intent: {}",
                        conflicts.join(", ")
                    ),
                    comments: Vec::new(),
                    round: task.round + 1,
                };
                self.update_task(task_id, |task| {
                    task.last_review = Some(review);
                    task.status = task_status::FIXING.into();
                    task.status_note = None;
                })?;
            }
        } else {
            let fixing = task
                .last_review
                .as_ref()
                .is_some_and(|review| review.verdict == "changes_requested");
            self.update_task(task_id, |task| {
                task.status = if task.status == task_status::NEEDS_HUMAN {
                    // One more round than the limit allowed.
                    task.round += 1;
                    task_status::FIXING.into()
                } else if fixing {
                    task_status::FIXING.into()
                } else {
                    task_status::READY.into()
                };
                task.status_note = None;
            })?;
        }
        self.continue_after_task_action(&task.run_id)
    }

    pub fn skip_task(self: &Arc<Self>, task_id: &str) -> Result<()> {
        let task = self.task_for_action(task_id)?;
        if task.status == task_status::MERGED {
            return Err(Error::msg("Task đã merge"));
        }
        self.update_task(task_id, |task| {
            task.status = task_status::SKIPPED.into();
            task.status_note = Some("Bỏ qua theo yêu cầu".into());
        })?;
        self.continue_after_task_action(&task.run_id)
    }

    /// Another CLI takes over, starting a new session in the same worktree.
    pub fn reassign_task(self: &Arc<Self>, task_id: &str, cli: &str) -> Result<()> {
        Cli::parse(cli)?;
        let task = self.task_for_action(task_id)?;
        if task.status == task_status::MERGED {
            return Err(Error::msg("Task đã merge"));
        }
        let cli = cli.to_string();
        self.update_task(task_id, |task| {
            task.cli = cli;
            task.worker_session = None;
            let fixing = task
                .last_review
                .as_ref()
                .is_some_and(|review| review.verdict == "changes_requested");
            task.status = if fixing {
                task_status::FIXING.into()
            } else {
                task_status::READY.into()
            };
            task.status_note = None;
        })?;
        self.continue_after_task_action(&task.run_id)
    }

    /// "Yêu cầu agent sửa lại": the user's own review.
    pub fn request_fix(self: &Arc<Self>, task_id: &str, feedback: &str) -> Result<()> {
        let feedback = feedback.trim().to_string();
        if feedback.is_empty() {
            return Err(Error::msg("Hãy nhập nội dung cần sửa"));
        }
        let task = self.task_for_action(task_id)?;
        if task.worktree.is_none() {
            return Err(Error::msg("Task chưa có code để sửa"));
        }
        self.update_task(task_id, |task| {
            task.round += 1;
            task.last_review = Some(Review {
                verdict: "changes_requested".into(),
                summary: feedback,
                comments: Vec::new(),
                round: task.round,
            });
            task.status = task_status::FIXING.into();
            task.status_note = None;
        })?;
        self.continue_after_task_action(&task.run_id)
    }

    /// "Bỏ qua review": merge the task as it is.
    pub async fn approve_task(self: &Arc<Self>, task_id: &str) -> Result<()> {
        let task = self.task_for_action(task_id)?;
        if task.worktree.is_none() || task.branch.is_none() {
            return Err(Error::msg("Task chưa có code để merge"));
        }
        let run = self.db().read(|c| repo::run_get(c, &task.run_id))?;
        let project = self.project(&run)?;
        let integration = self.ensure_integration(&run, &project).await?;
        let base = PathBuf::from(task.worktree.clone().unwrap_or_default());
        // Work the agent left uncommitted counts too.
        self.commit_workspace(&run, &base, &format!("cm({}): {}", task.key, task.title))
            .await?;
        self.merge_task(&run, &integration, &task)
            .await
            .map_err(|error| match error {
                StepError::Failed(message) => Error::msg(message),
                StepError::Cancelled => Error::msg("Đã dừng"),
            })?;
        self.continue_after_task_action(&task.run_id)
    }

    /// The task's diff against the integration branch.
    pub async fn task_diff(&self, task_id: &str) -> Result<String> {
        let task = self.db().read(|c| repo::task_get(c, task_id))?;
        let run = self.db().read(|c| repo::run_get(c, &task.run_id))?;
        let project = self.project(&run)?;
        if let Some(base) = &task.worktree {
            let (text, _) = git::cut(self.workspace_diff(&run, Path::new(base)).await?, 2 << 20);
            return Ok(text);
        }
        let branch = task
            .branch
            .clone()
            .ok_or_else(|| Error::msg("Task chưa có code"))?;
        // Merged: what the task branch brought, repo by repo.
        let mut out = String::new();
        for repo in &run.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            if !git::branch_exists(&root, &branch).await {
                continue;
            }
            let prefix = workspace::project_path(&repo.path, "");
            out.push_str(
                &git::diff_range(&root, &format!("{}...{branch}", repo.base_commit), &prefix)
                    .await?,
            );
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        Ok(out)
    }

    /// Everything the run would merge.
    pub async fn run_diff(&self, run_id: &str) -> Result<String> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let project = self.project(&run)?;
        let mut out = String::new();
        for repo in &run.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            let range = format!("{}...{}", repo.base_commit, run.integration_branch);
            let prefix = workspace::project_path(&repo.path, "");
            out.push_str(&git::diff_range(&root, &range, &prefix).await?);
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        Ok(out)
    }

    /// Summary for gate 2: files and lines changed by the run.
    pub async fn run_diffstat(&self, run_id: &str) -> Result<(u64, u64, u64)> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let project = self.project(&run)?;
        let mut total = (0, 0, 0);
        for repo in &run.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            let range = format!("{}...{}", repo.base_commit, run.integration_branch);
            let (files, added, removed) = git::numstat_range(&root, &range).await?;
            total = (total.0 + files, total.1 + added, total.2 + removed);
        }
        Ok(total)
    }
}
