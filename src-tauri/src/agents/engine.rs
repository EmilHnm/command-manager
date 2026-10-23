//! The run state machine (SCR-09 §3). One driver task per active run walks
//! the stages; it stops at a gate, on something only a person can decide,
//! when paused, or when the run is done. Everything it decides is in the
//! database first, so a driver can be started again at any point (after a
//! pause, an error or an app restart) and pick up where the last one left.
//!
//! Phase 1: tasks run one at a time, in dependency order.

use super::adapter::{self, Access, Cli, Outcome, Quota};
use super::context;
use super::git::{self, MergeError};
use super::guides::{self, Guides, Use};
use super::log::AgentLog;
use super::process::{self, unix_now, RunError, Spawn, Stopped};
use super::prompts;
use super::workspace;
use crate::clock::now_unix;
use crate::db::repos::agents::{
    self as repo, run_status, stage, task_status, CheckProposal, CheckResult, Limits, Phase,
    Project, ProjectSettings, Review, ReviewComment, RoleCli, Run, Task, TaskDraft,
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

/// What the engine needs from the app.
pub trait AgentEnv: Send + Sync + 'static {
    fn db(&self) -> &Db;
    /// Worktrees, schema files and raw logs live under `<data_dir>/agents`.
    fn data_dir(&self) -> PathBuf;
    fn find_cli(&self, cli: Cli) -> Option<PathBuf> {
        adapter::find(cli)
    }
    /// GitHub's CLI, for pull requests.
    fn find_gh(&self) -> Option<PathBuf> {
        context::find_gh()
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
    #[serde(default)]
    pub leader_model: Option<String>,
    #[serde(default)]
    pub leader_effort: Option<String>,
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

/// Name of the git "remote helper" every push of a worker is sent to; it
/// does not exist, so the push fails whatever remote or `-C` it names.
const PUSH_BLOCKED: &str = "command-manager-blocked-push";

/// The environment of a step that writes code: pushing and opening pull
/// requests are the orchestrator's job, and rules in the CLIs are easy to
/// get around (`git -C x push`), so they are made impossible here.
/// - git: `url.<x>.pushInsteadOf` with an empty prefix rewrites every push
///   URL; fetching is untouched.
/// - gh: an empty config folder and no token, so it has no account.
fn write_env(run_dir: &Path) -> Vec<(String, String)> {
    let gh = run_dir.join("gh-no-auth");
    let _ = std::fs::create_dir_all(&gh);
    vec![
        ("GIT_CONFIG_COUNT".into(), "1".into()),
        (
            "GIT_CONFIG_KEY_0".into(),
            format!("url.{PUSH_BLOCKED}://.pushInsteadOf"),
        ),
        ("GIT_CONFIG_VALUE_0".into(), String::new()),
        ("GH_CONFIG_DIR".into(), gh.to_string_lossy().into_owned()),
        ("GH_TOKEN".into(), String::new()),
        ("GITHUB_TOKEN".into(), String::new()),
        ("GH_PROMPT_DISABLED".into(), "1".into()),
    ]
}

/// A trimmed, non-empty choice.
fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Gate 2's "Push & tạo PR": what to push where, and the PR's text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrRequest {
    pub repos: Vec<PrRepo>,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrRepo {
    pub repo: String,
    /// The branch the integration branch is pushed as.
    pub branch: String,
    /// What the PR merges into.
    pub base: String,
}

/// "Gắn PR": a pull request opened by hand for the run's work in a repo.
/// Branch and base are read from GitHub when not given.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachPr {
    pub repo: String,
    pub url: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub base: Option<String>,
}

const FETCH_TIMEOUT: Duration = Duration::from_secs(120);

/// What "Verify PR" found, kept in `<run dir>/verification.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Verification {
    pub checked_at: String,
    pub prs: Vec<PrCheck>,
    /// The leader's review of the pull requests' code.
    pub review: Option<Review>,
    /// A follow-up was started with the open comments.
    pub followed_up: bool,
    /// Why no follow-up was started although something is open.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrCheck {
    pub repo: String,
    pub url: String,
    pub branch: String,
    pub base: String,
    /// open, merged, closed.
    pub state: String,
    pub review_decision: String,
    /// Files the run changed in this repo, and how many the PR has too.
    pub run_files: usize,
    pub matched: usize,
    pub pr_files: usize,
    /// Run files missing from the PR (a few, to show).
    pub missing: Vec<String>,
    /// Fewer than half of the run's files are in the PR: likely the wrong
    /// pull request.
    pub mismatch: bool,
    pub open_comments: Vec<context::PrComment>,
    pub error: Option<String>,
}

const VERIFICATION_FILE: &str = "verification.json";

/// How long a push may take: pre-push hooks run the project's checks.
const PUSH_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const GH_TIMEOUT: Duration = Duration::from_secs(3 * 60);

/// A branch name safe to pass to git and gh as it is.
fn valid_branch(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.starts_with('/')
        && !name.ends_with('/')
        && !name.ends_with(".lock")
        && !name.contains("..")
        && !name.contains("//")
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
}

/// The URL `gh pr create` / `gh pr view` printed.
fn pr_url(output: &str) -> Option<String> {
    output
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| line.starts_with("https://") && line.contains("/pull/"))
        .map(str::to_string)
}

/// The leader's plan, as its schema has it.
#[derive(Debug, Deserialize)]
struct PlanAnswer {
    contract: String,
    tasks: Vec<PlanTask>,
    #[serde(default)]
    checks: Vec<CheckProposal>,
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
    /// Usage limits of each CLI's account, as last reported.
    pub quotas: HashMap<String, Quota>,
    /// The last "Verify PR".
    pub verification: Option<Verification>,
    /// Paused for a usage limit: when the run goes on by itself.
    pub resume_at: Option<i64>,
}

enum Flow {
    Continue,
    Stop,
}

/// One check command of a repo, where and how long it runs.
struct CheckJob {
    repo: String,
    command: String,
    cwd: PathBuf,
    timeout: Duration,
}

enum StepError {
    Cancelled,
    Failed(String),
    /// The account of a CLI is (nearly) out of usage: pause, maybe until
    /// `resume_at`.
    Quota {
        note: String,
        resume_at: Option<i64>,
    },
}

impl From<Error> for StepError {
    fn from(error: Error) -> Self {
        Self::Failed(error.to_string())
    }
}

impl From<StepError> for Error {
    fn from(error: StepError) -> Self {
        match error {
            StepError::Failed(message) => Error::msg(message),
            StepError::Cancelled => Error::msg("Đã dừng"),
            StepError::Quota { note, .. } => Error::msg(note),
        }
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
    /// Last usage limits seen per CLI (accounts are shared by all runs).
    quotas: Mutex<HashMap<String, Quota>>,
    /// Runs paused for a usage limit, and when they go on.
    quota_waits: Mutex<HashMap<String, i64>>,
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
}

impl Engine {
    pub fn new(env: Arc<dyn AgentEnv>) -> Arc<Self> {
        Arc::new(Self {
            env,
            drivers: Mutex::new(HashMap::new()),
            quotas: Mutex::new(HashMap::new()),
            quota_waits: Mutex::new(HashMap::new()),
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
            quotas: self.quotas.lock().expect("agent quotas").clone(),
            verification: std::fs::read_to_string(self.run_dir(run_id).join(VERIFICATION_FILE))
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok()),
            resume_at: self
                .quota_waits
                .lock()
                .expect("agent quota waits")
                .get(run_id)
                .copied(),
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
                model: clean(role.model),
                effort: clean(role.effort),
            });
        }
        if roles.is_empty() {
            return Err(Error::msg("Hãy khai báo ít nhất một vai cho worker"));
        }
        let limits = new.limits.unwrap_or_default();
        limits.validate()?;

        let project = self.db().read(|c| repo::project_get(c, &new.project_id))?;
        let keep = workspace::distinct(
            Path::new(&project.path),
            project.repos.iter().map(|repo| repo.path.clone()).collect(),
        )
        .await;
        let mut repos = Vec::new();
        for config in project
            .repos
            .iter()
            .filter(|repo| keep.contains(&repo.path))
        {
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
            followups: Vec::new(),
            leader_model: clean(new.leader_model),
            leader_effort: clean(new.leader_effort),
            suggested_checks: Vec::new(),
            pull_requests: Vec::new(),
        };
        self.db().write(|c| {
            repo::run_insert(c, &run)?;
            // The next run here starts from these.
            repo::project_save_limits(c, &run.project_id, &run.limits)
        })?;
        self.drive(&run.id);
        Ok(run)
    }

    /// Starts the driver of a run unless one is already working on it.
    pub fn drive(self: &Arc<Self>, run_id: &str) {
        self.quota_waits
            .lock()
            .expect("agent quota waits")
            .remove(run_id);
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
                Err(StepError::Quota { note, resume_at }) => {
                    let resume_at = resume_at.filter(|_| run.limits.auto_resume);
                    let _ = self.update_run(run_id, |run| {
                        run.status = run_status::PAUSED.into();
                        run.status_note = Some(match resume_at {
                            Some(_) => format!("{note}; tự chạy tiếp khi hạn mức reset"),
                            None => format!("{note}; bấm Tiếp tục khi còn hạn mức"),
                        });
                    });
                    if let Some(at) = resume_at {
                        self.resume_later(run_id, at);
                    }
                    return;
                }
            }
        }
    }

    /// Drives a run paused for a usage limit again once the limit resets,
    /// unless someone has acted on it in between.
    fn resume_later(self: &Arc<Self>, run_id: &str, at: i64) {
        self.quota_waits
            .lock()
            .expect("agent quota waits")
            .insert(run_id.to_string(), at);
        self.env.changed(run_id);
        let engine = Arc::clone(self);
        let run_id = run_id.to_string();
        tokio::spawn(async move {
            // A minute of slack: limits reset on the server's clock.
            let wait = (at + 60 - unix_now()).max(5) as u64;
            tokio::time::sleep(Duration::from_secs(wait)).await;
            let mine = engine
                .quota_waits
                .lock()
                .expect("agent quota waits")
                .get(&run_id)
                == Some(&at);
            if !mine {
                return;
            }
            let paused = engine
                .db()
                .read(|c| repo::run_get(c, &run_id))
                .is_ok_and(|run| run.status == run_status::PAUSED);
            if paused {
                engine.drive(&run_id);
            } else {
                engine
                    .quota_waits
                    .lock()
                    .expect("agent quota waits")
                    .remove(&run_id);
            }
        });
    }

    /// Remembers a CLI's usage limits; the error to stop with if they block
    /// work under `limits`.
    fn note_quota(&self, cli: &str, quota: Option<&Quota>, limits: &Limits) -> Option<StepError> {
        let mut quotas = self.quotas.lock().expect("agent quotas");
        if let Some(quota) = quota {
            quotas.insert(cli.to_string(), quota.clone());
        }
        let quota = quotas.get(cli)?;
        let (window, state) = quota.blocking(limits.quota_limit(), unix_now())?;
        Some(StepError::Quota {
            note: format!(
                "Hạn mức {window} của {cli} đã dùng {:.0}%{}",
                state.utilization * 100.0,
                if quota.rejected {
                    String::new()
                } else {
                    format!(" (ngưỡng {}%)", limits.quota_pause_percent)
                }
            ),
            resume_at: (state.resets_at > 0).then_some(state.resets_at),
        })
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
        let budget = run.limits.max_cost_usd - usage.cost_usd;
        if budget <= 0.0 {
            return Err(StepError::Failed(format!(
                "Đã tiêu ~${:.2}, chạm ngân sách ${:.2} của lượt chạy; tăng ngân sách để tiếp tục",
                usage.cost_usd, run.limits.max_cost_usd
            )));
        }
        if let Some(error) = self.note_quota(cli_name, None, &run.limits) {
            return Err(error);
        }
        let cli = Cli::parse(cli_name)?;
        let program = self.env.find_cli(cli).ok_or_else(|| {
            StepError::Failed(format!(
                "Chưa cài {} (không tìm thấy {})",
                cli.as_str(),
                cli.binaries().join(" / ")
            ))
        })?;
        let phase = match step_name {
            "design" | "plan" => Phase::Lead,
            "review" | "verify" => Phase::Review,
            _ => Phase::Work,
        };
        let mut profile = self
            .db()
            .read(|c| repo::profile_get(c, cli_name))?
            .for_phase(phase);
        // Picked for this run (leader, role) or task beat the profile's.
        let (model, effort) = match (phase, task) {
            (Phase::Lead, _) => (run.leader_model.clone(), run.leader_effort.clone()),
            (Phase::Work, Some(task)) => {
                // A role's model is for its CLI; a reassigned task has its own.
                let role = run
                    .roles
                    .iter()
                    .find(|role| role.role == task.role && role.cli == task.cli);
                (
                    task.model
                        .clone()
                        .or_else(|| role.and_then(|r| r.model.clone())),
                    task.effort
                        .clone()
                        .or_else(|| role.and_then(|r| r.effort.clone())),
                )
            }
            _ => (None, None),
        };
        if model.is_some() {
            profile.model = model;
        }
        if effort.is_some() {
            profile.effort = effort;
        }

        let dir = self.run_dir(&run.id);
        std::fs::create_dir_all(&dir).map_err(Error::from)?;
        let mut extra_dirs: Vec<PathBuf> = Some(dir.join(context::DIR))
            .filter(|dir| dir.is_dir())
            .into_iter()
            .collect();
        // Rules and skills that are not in the agent's folder.
        if let Ok(project) = self.project(run) {
            let (_, outside) = guides::block(
                &Self::guides(&project),
                Path::new(&project.path),
                cwd,
                None,
                Use::Work,
            );
            extra_dirs.extend(outside);
        }
        let id = uuid::Uuid::new_v4().to_string();
        let schema_file = match schema {
            Some(schema) if cli.has_schema_flag() && cli != Cli::Claude => {
                let path = dir.join(format!("schema-{step_name}.json"));
                std::fs::write(&path, schema.to_string()).map_err(Error::from)?;
                Some(path)
            }
            _ => None,
        };
        // A resumed Claude session reports the cost and tokens of the whole
        // session; what this step adds is what it reports beyond its earlier
        // steps (each stored as its own share).
        let earlier = match (cli, resume) {
            (Cli::Claude, Some(session)) => self
                .db()
                .read(|c| repo::invocation_list(c, &run.id))?
                .iter()
                .filter(|inv| inv.cli == cli_name && inv.session_id.as_deref() == Some(session))
                .fold((0.0, 0, 0), |(cost, input, output), inv| {
                    (
                        cost + inv.cost_usd.unwrap_or(0.0),
                        input + inv.input_tokens,
                        output + inv.output_tokens,
                    )
                }),
            _ => (0.0, 0, 0),
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
                // Claude holds the whole session to the budget.
                budget_usd: Some(budget + earlier.0),
                extra_dirs: &extra_dirs,
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
            resumable: false,
        };
        self.db()
            .write(|c| repo::invocation_insert(c, &invocation))?;
        // The tab first: the UI opens it as soon as it hears of the change.
        let log = self.env.open_log(&terminal_id, title);
        self.env.changed(&run.id);
        log.heading(title);
        log.info(&format!(
            "{} · {}{}{}",
            cwd.display(),
            match (&profile.model, &profile.effort) {
                (None, None) => String::new(),
                (model, effort) => format!(
                    "model {} · effort {} · ",
                    model.as_deref().unwrap_or("mặc định"),
                    effort.as_deref().unwrap_or("mặc định")
                ),
            },
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
        // A worker's task may have its own; review and leader steps use the run's.
        let timeout_minutes = task
            .filter(|_| phase == Phase::Work)
            .and_then(|task| task.timeout_minutes)
            .unwrap_or(run.limits.timeout_minutes);
        let started = Instant::now();
        let result = process::run(
            Spawn {
                cli,
                program: &program,
                args,
                cwd,
                timeout: Duration::from_secs(timeout_minutes as u64 * 60),
                raw_log: Some(dir.join(format!("{id}.jsonl"))),
                quota_limit: run.limits.quota_limit(),
                env: if access == Access::Write {
                    write_env(&dir)
                } else {
                    Vec::new()
                },
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
                invocation.input_tokens = outcome.input_tokens.saturating_sub(earlier.1);
                invocation.output_tokens = outcome.output_tokens.saturating_sub(earlier.2);
                invocation.cost_usd = outcome.cost_usd.map(|cost| (cost - earlier.0).max(0.0));
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
                let blocked = self.note_quota(cli_name, outcome.quota.as_ref(), &run.limits);
                if outcome.hit_limit() {
                    // Out of usage mid-step: pause and continue it later.
                    invocation.resumable = true;
                    Err(blocked.unwrap_or_else(|| StepError::Quota {
                        note: format!(
                            "{cli_name} báo hết hạn mức: {}",
                            outcome.error.as_deref().unwrap_or("?")
                        ),
                        resume_at: outcome
                            .limit_resets_in()
                            .map(|secs| unix_now() + secs as i64),
                    }))
                } else {
                    Ok(outcome)
                }
            }
            Err(Stopped { error, partial }) => {
                invocation.session_id = partial.session_id.clone().or(invocation.session_id);
                invocation.input_tokens = partial.input_tokens;
                invocation.output_tokens = partial.output_tokens;
                invocation.cost_usd = partial.cost_usd;
                invocation.ok = Some(false);
                invocation.error = Some(error.to_string());
                invocation.resumable = matches!(
                    error,
                    RunError::Cancelled | RunError::TimedOut | RunError::Quota(_)
                );
                log.result(false, &error.to_string());
                let blocked = self.note_quota(cli_name, partial.quota.as_ref(), &run.limits);
                Err(match error {
                    RunError::Cancelled => StepError::Cancelled,
                    RunError::TimedOut => StepError::Failed(format!(
                        "{} chạy quá {timeout_minutes} phút; tăng timeout rồi Thử lại để làm \
                         tiếp trong phiên cũ",
                        cli.as_str(),
                    )),
                    RunError::Failed(message) => StepError::Failed(message),
                    RunError::Quota(_) => blocked.unwrap_or_else(|| StepError::Quota {
                        note: error.to_string(),
                        resume_at: None,
                    }),
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

    /// The session of the step's last invocation when that one was cut short
    /// (stopped, timed out, out of usage, app closed): going on with it keeps
    /// what the agent already read and did instead of starting over.
    fn interrupted_session(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        cli: &str,
        steps: &[&str],
    ) -> Result<Option<String>> {
        let invocations = self.db().read(|c| repo::invocation_list(c, run_id))?;
        Ok(invocations
            .into_iter()
            .rev()
            .find(|inv| inv.task_id.as_deref() == task_id && steps.contains(&inv.step.as_str()))
            .filter(|inv| inv.resumable && inv.cli == cli)
            .and_then(|inv| inv.session_id))
    }

    /// The project's rules and skills, as they are now.
    fn guides(project: &Project) -> Guides {
        let repos: Vec<String> = project.repos.iter().map(|repo| repo.path.clone()).collect();
        guides::scan(Path::new(&project.path), &repos)
    }

    fn context_dir(&self, run_id: &str) -> PathBuf {
        self.run_dir(run_id).join(context::DIR)
    }

    /// Pull requests `text` links to, fetched for every agent (`refresh`:
    /// again, for comments made since).
    async fn prepare_context(&self, run: &Run, text: &str, refresh: bool) {
        if context::pr_links(text).is_empty() {
            return;
        }
        let id = format!("agent:context-{}", uuid::Uuid::new_v4());
        let log = self.env.open_log(&id, "chuẩn bị ngữ cảnh");
        context::fetch_prs(&self.context_dir(&run.id), text, &log, refresh).await;
        self.env.close_log(&id, true);
    }

    async fn design(&self, run: &Run, control: &Control) -> Step<Flow> {
        let project = self.project(run)?;
        let feedback = run
            .pending_feedback
            .clone()
            .filter(|_| run.leader_session.is_some());
        let interrupted = self.interrupted_session(&run.id, None, &run.leader_cli, &["design"])?;
        let (prompt, resume, title) = match (&interrupted, &feedback) {
            (Some(session), _) => (
                prompts::resume_step("Reply with the complete design document only."),
                Some(session.as_str()),
                format!("{} · thiết kế (tiếp)", run.leader_cli),
            ),
            (None, Some(feedback)) => (
                prompts::redesign(run, feedback),
                run.leader_session.as_deref(),
                format!("{} · thiết kế lại", run.leader_cli),
            ),
            (None, None) => {
                self.prepare_context(run, &run.request, false).await;
                let overview = context::overview(Path::new(&project.path), &run.repos).await;
                let files = context::files(&self.context_dir(&run.id));
                let here = Path::new(&project.path);
                let (guides, _) =
                    guides::block(&Self::guides(&project), here, here, None, Use::Lead);
                (
                    prompts::design(run, &project, &overview, &files, &guides),
                    None,
                    format!("{} · thiết kế", run.leader_cli),
                )
            }
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
        let earlier = self.db().read(|c| repo::task_list(c, &run.id))?;
        let parse = |value: Value| -> Result<(String, Vec<TaskDraft>, Vec<CheckProposal>)> {
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
            Ok((
                answer.contract,
                Self::check_drafts(run, &earlier, drafts)?,
                answer.checks,
            ))
        };
        let interrupted = self.interrupted_session(&run.id, None, &run.leader_cli, &["plan"])?;
        let (resume, prompt) = match &interrupted {
            Some(session) => (
                Some(session.as_str()),
                prompts::resume_step(
                    "Reply with ONLY the JSON object of the plan, matching the required schema.",
                ),
            ),
            None => match run.followups.last() {
                Some(followup) => {
                    // Fresh review comments of the pull requests involved:
                    // the ones linked in the request and the run's own.
                    let links = run
                        .pull_requests
                        .iter()
                        .map(|pr| pr.url.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let text = format!("{}\n{links}", followup.request);
                    self.prepare_context(run, &text, true).await;
                    let done: Vec<&Task> = earlier
                        .iter()
                        .filter(|task| task.batch < run.batch())
                        .collect();
                    let files = context::files(&self.context_dir(&run.id));
                    (
                        run.leader_session.as_deref(),
                        prompts::follow_up(run, &project, &followup.request, &done, &files),
                    )
                }
                None => (run.leader_session.as_deref(), prompts::plan(run, &project)),
            },
        };
        let ((contract, drafts, suggested), outcome) = self
            .ask_json(
                run,
                None,
                "plan",
                &format!("{} · kế hoạch", run.leader_cli),
                Path::new(&project.path),
                resume,
                prompt,
                &schema,
                control,
                parse,
            )
            .await?;
        self.replace_tasks(run, &drafts)?;
        // A follow-up adds to the contract of the delivered work.
        let contract = match (run.batch(), run.contract.as_deref()) {
            (0, _) | (_, None) => contract,
            (_, Some(old)) if contract.trim().is_empty() => old.to_string(),
            (batch, Some(old)) => format!("{old}\n\n## Follow-up {batch}\n{}", contract.trim()),
        };
        let next = if run.plan_gate {
            stage::PLAN_GATE
        } else {
            stage::IMPLEMENT
        };
        let suggested = Self::proposals(&project, suggested);
        self.update_run(&run.id, |run| {
            run.contract = Some(contract);
            run.suggested_checks = suggested;
            run.pending_feedback = None;
            if outcome.session_id.is_some() {
                run.leader_session = outcome.session_id;
            }
            run.stage = next.into();
        })?;
        Ok(Flow::Continue)
    }

    /// Checks the drafts of the current batch against the tasks delivered
    /// before it: new keys, dependencies on delivered work dropped (it is
    /// merged already).
    fn check_drafts(run: &Run, earlier: &[Task], drafts: Vec<TaskDraft>) -> Result<Vec<TaskDraft>> {
        let old: Vec<&str> = earlier
            .iter()
            .filter(|task| task.batch < run.batch())
            .map(|task| task.key.as_str())
            .collect();
        let mut drafts = Self::fill_repos(run, drafts);
        for draft in &mut drafts {
            if old.contains(&draft.key.trim()) {
                return Err(Error::msg(format!(
                    "Key `{}` đã dùng ở lượt trước; hãy đặt key mới",
                    draft.key.trim()
                )));
            }
            draft.depends_on.retain(|dep| !old.contains(&dep.as_str()));
        }
        let repos: Vec<String> = run.repos.iter().map(|r| r.path.clone()).collect();
        repo::check_plan(&drafts, &repos)
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

    /// Plans the tasks of the run's current batch again; earlier batches
    /// (delivered work) stay.
    fn replace_tasks(&self, run: &Run, drafts: &[TaskDraft]) -> Result<()> {
        let run_id = run.id.as_str();
        let batch = run.batch();
        self.db().write(|c| {
            repo::task_delete_batch(c, run_id, batch)?;
            let first = repo::task_list(c, run_id)?
                .iter()
                .map(|task| task.sort_order + 1)
                .max()
                .unwrap_or(0);
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
                        sort_order: first + index as i64,
                        batch,
                        handoff: None,
                        model: None,
                        effort: None,
                        timeout_minutes: None,
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
        if self.drop_duplicate_repos(run, &project).await? {
            return Ok(Flow::Continue);
        }
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

    /// A run started before linked worktrees were told apart may list one git
    /// repository twice (`fe/` and a `wt-x/` made from it); its branches can
    /// only be checked out once. The copy no task works in is dropped.
    async fn drop_duplicate_repos(&self, run: &Run, project: &Project) -> Result<bool> {
        let paths: Vec<String> = run.repos.iter().map(|repo| repo.path.clone()).collect();
        let keep = workspace::distinct(Path::new(&project.path), paths.clone()).await;
        if keep.len() == paths.len() {
            return Ok(false);
        }
        let tasks = self.db().read(|c| repo::task_list(c, &run.id))?;
        let dropped: Vec<String> = paths
            .into_iter()
            .filter(|path| !keep.contains(path) && !tasks.iter().any(|t| t.repos.contains(path)))
            .collect();
        if dropped.is_empty() {
            return Ok(false);
        }
        self.update_run(&run.id, |run| {
            run.repos.retain(|repo| !dropped.contains(&repo.path));
        })?;
        // Go round again only for a change that stuck, never in a loop.
        let saved = self.db().read(|c| repo::run_get(c, &run.id))?;
        Ok(saved.repos.len() < run.repos.len())
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
        let setups: Vec<(String, String, Duration)> = task
            .repos
            .iter()
            .filter_map(|repo| {
                let config = project.repo(repo)?;
                config
                    .setup_cmd
                    .clone()
                    .map(|cmd| (repo.clone(), cmd, config.check_timeout()))
            })
            .collect();
        if setups.is_empty() {
            return Ok(task);
        }
        let id = format!("agent:setup-{}", uuid::Uuid::new_v4());
        let log = self.env.open_log(&id, &format!("chuẩn bị · {}", task.key));
        for (repo, setup, timeout) in setups {
            log.heading(&format!("{}$ {setup}", workspace::label(&repo)));
            let dir = workspace::repo_dir(&base, &repo);
            let result = process::run_shell(&setup, &dir, timeout, &log, stop.clone()).await;
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
        let context_dir = self.context_dir(&run.id);
        // Claude (given the folder) and codex read outside their worktree.
        let design_file = match (Cli::parse(&task.cli), &run.design_md) {
            (Ok(Cli::Claude | Cli::Codex), Some(design)) => {
                context::write_design(&context_dir, design).ok()
            }
            _ => None,
        };
        let files = context::files(&context_dir);
        // What the tasks merged before this one left for it.
        let all_tasks = self.db().read(|c| repo::task_list(c, &run.id))?;
        let done: Vec<&Task> = all_tasks
            .iter()
            .filter(|other| {
                other.id != task.id
                    && other.status == task_status::MERGED
                    && other.repos.iter().any(|repo| task.repos.contains(repo))
            })
            .collect();
        let (guides, _) = guides::block(
            &Self::guides(project),
            Path::new(&project.path),
            &worktree,
            Some(&task.repos),
            Use::Work,
        );
        let shared = prompts::Shared {
            design_file: design_file.as_deref(),
            files: &files,
            done: &done,
            guides: &guides,
        };
        let interrupted =
            self.interrupted_session(&run.id, Some(&task.id), &task.cli, &["implement", "fix"])?;
        let (prompt, resume, step) = match (&interrupted, &review, &task.worker_session) {
            (Some(session), _, _) => (
                prompts::resume_step(prompts::HANDOFF_ASK),
                Some(session.clone()),
                if fixing { "fix" } else { "implement" },
            ),
            (None, Some(review), Some(session)) if fixing => (
                prompts::fix(review, task.checks.as_deref().unwrap_or(&[])),
                Some(session.clone()),
                "fix",
            ),
            // A new agent (reassigned) gets the whole task plus the review.
            (None, Some(review), None) if fixing => (
                format!(
                    "{}\n\n{}",
                    prompts::implement(run, project, &task, earlier_work, &shared),
                    prompts::fix(review, task.checks.as_deref().unwrap_or(&[]))
                ),
                None,
                "fix",
            ),
            _ => (
                prompts::implement(run, project, &task, earlier_work, &shared),
                task.worker_session.clone().filter(|_| earlier_work),
                "implement",
            ),
        };
        // Where each repo stood: a fix round that leaves them all as they
        // were has not answered its review.
        let heads_before = self.workspace_heads(run, &worktree).await;
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
            Err(error @ (StepError::Cancelled | StepError::Quota { .. })) => {
                self.update_task(&task.id, |task| task.status = before)?;
                return Err(error);
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
        if step == "fix" && self.workspace_heads(run, &worktree).await == heads_before {
            // The worker disagrees with the review (or cannot act on it):
            // reviewing the same code again would only loop.
            let reply = adapter::shorten(outcome.text.trim(), 400);
            self.update_task(&task.id, |task| {
                task.status = task_status::NEEDS_HUMAN.into();
                task.status_note = Some(format!(
                    "Vòng sửa không thay đổi gì nên không review lại. Agent trả lời: {reply}"
                ));
            })?;
            return Ok(());
        }
        let outside = git::outside_scope(&task.scope, &files);
        let handoff = Self::handoff(&outcome.text, &files);
        self.update_task(&task.id, |task| {
            task.added = added;
            task.removed = removed;
            task.out_of_scope = outside;
            task.handoff = Some(handoff);
            task.status = task_status::CHECKING.into();
        })?;
        Ok(())
    }

    /// The commit each repo of a workspace is at.
    async fn workspace_heads(&self, run: &Run, base: &Path) -> Vec<Option<String>> {
        let mut heads = Vec::new();
        for repo in &run.repos {
            let dir = workspace::repo_dir(base, &repo.path);
            heads.push(git::commit_of(&dir, "HEAD").await.ok());
        }
        heads
    }

    /// The note a worker leaves for the tasks after it: its final summary and
    /// the files its task changed so far.
    fn handoff(summary: &str, files: &[String]) -> String {
        const SHOWN: usize = 30;
        let mut note = summary.trim().to_string();
        if !files.is_empty() {
            let mut list = files
                .iter()
                .take(SHOWN)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            if files.len() > SHOWN {
                list.push_str(&format!(" (+{} tệp)", files.len() - SHOWN));
            }
            note.push_str(&format!("\nFiles changed: {list}"));
        }
        note
    }

    /// Runs `(repo, command, folder)` checks in order, in one log tab.
    async fn run_checks(
        &self,
        jobs: &[CheckJob],
        title: &str,
        control: &Control,
    ) -> Step<Vec<CheckResult>> {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }
        let id = format!("agent:check-{}", uuid::Uuid::new_v4());
        let log = self.env.open_log(&id, title);
        let mut results = Vec::new();
        for job in jobs {
            let shown = format!("{}{}", workspace::label(&job.repo), job.command);
            log.heading(&format!("$ {shown}"));
            let started = Instant::now();
            let result = process::run_shell(
                &job.command,
                &job.cwd,
                job.timeout,
                &log,
                control.stop.subscribe(),
            )
            .await;
            let (ok, exit_code, output) = match result {
                Ok(done) => done,
                Err(RunError::Cancelled) => {
                    self.env.close_log(&id, false);
                    return Err(StepError::Cancelled);
                }
                Err(RunError::TimedOut) => (
                    false,
                    None,
                    format!(
                        "Hết thời gian ({} phút). Tăng \"Timeout lệnh kiểm tra\" của repo trong \
                         Đội & CLI → Dự án nếu lệnh này vốn chạy lâu.",
                        job.timeout.as_secs() / 60
                    ),
                ),
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
    fn check_jobs(project: &Project, repos: &[String], base: &Path) -> Vec<CheckJob> {
        repos
            .iter()
            .filter_map(|repo| project.repo(repo))
            .flat_map(|config| {
                config.check_cmds.iter().map(move |command| CheckJob {
                    repo: config.path.clone(),
                    command: command.clone(),
                    cwd: workspace::repo_dir(base, &config.path),
                    timeout: config.check_timeout(),
                })
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
        let failed = checks.iter().any(|check| !check.ok);
        self.update_task(&task.id, |task| {
            task.checks = Some(checks);
            task.status = task_status::REVIEW.into();
        })?;
        if failed {
            // The verdict is known: back to the worker without asking the
            // reviewer (a whole leader call per round saved).
            let review = Review {
                verdict: "changes_requested".into(),
                summary: "Lệnh kiểm tra thất bại; hãy sửa cho các lệnh kiểm tra chạy qua.".into(),
                comments: Vec::new(),
                round: task.round + 1,
            };
            self.request_changes(run, task, review)?;
        }
        Ok(())
    }

    /// Back to the worker with a review, or to a person after the last round.
    fn request_changes(&self, run: &Run, task: &Task, review: Review) -> Step<()> {
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
        let files = context::files(&self.context_dir(&run.id));
        let project = self.project(run)?;
        let (guides, _) = guides::block(
            &Self::guides(&project),
            Path::new(&project.path),
            &worktree,
            Some(&task.repos),
            Use::Review,
        );
        let interrupted =
            self.interrupted_session(&run.id, Some(&task.id), &run.leader_cli, &["review"])?;
        let prompt = match &interrupted {
            Some(_) => prompts::resume_step(
                "Reply with ONLY the JSON review object, matching the required schema.",
            ),
            None => prompts::review(
                run,
                task,
                &diff,
                truncated,
                &run.integration_branch,
                &checks,
                &task.out_of_scope,
                &files,
                &guides,
            ),
        };
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
                interrupted.as_deref(),
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
        self.request_changes(run, task, review)
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
        checks: Vec<CheckProposal>,
    ) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.at_stage(run_id, stage::PLAN_GATE)?;
        let earlier = self.db().read(|c| repo::task_list(c, run_id))?;
        let drafts = Self::check_drafts(&run, &earlier, drafts)?;
        self.apply_checks(&run, checks)?;
        self.replace_tasks(&run, &drafts)?;
        self.update_run(run_id, |run| {
            run.design_md = Some(design);
            run.contract = Some(contract);
            run.suggested_checks = Vec::new();
            run.stage = stage::IMPLEMENT.into();
        })?;
        self.drive(run_id);
        Ok(())
    }

    /// The leader's check proposals worth showing: for repos of the run that
    /// have none configured, with at least one command.
    fn proposals(project: &Project, proposals: Vec<CheckProposal>) -> Vec<CheckProposal> {
        let mut kept: Vec<CheckProposal> = Vec::new();
        for proposal in proposals {
            let commands: Vec<String> = proposal
                .commands
                .iter()
                .map(|cmd| cmd.trim().to_string())
                .filter(|cmd| !cmd.is_empty())
                .collect();
            let configured = project
                .repo(&proposal.repo)
                .is_none_or(|config| !config.check_cmds.is_empty());
            if commands.is_empty() || configured || kept.iter().any(|p| p.repo == proposal.repo) {
                continue;
            }
            kept.push(CheckProposal {
                repo: proposal.repo,
                setup: proposal.setup.trim().to_string(),
                commands,
            });
        }
        kept
    }

    /// Gate 1: the check commands the user approved become the project's
    /// settings (a repo's own settings are never overwritten by an empty
    /// proposal).
    fn apply_checks(&self, run: &Run, checks: Vec<CheckProposal>) -> Result<()> {
        if checks.is_empty() {
            return Ok(());
        }
        let mut project = self.project(run)?;
        for proposal in checks {
            let Some(config) = project.repos.iter_mut().find(|r| r.path == proposal.repo) else {
                continue;
            };
            let commands: Vec<String> = proposal
                .commands
                .iter()
                .map(|cmd| cmd.trim().to_string())
                .filter(|cmd| !cmd.is_empty())
                .collect();
            if !commands.is_empty() {
                config.check_cmds = commands;
            }
            let setup = proposal.setup.trim();
            if !setup.is_empty() {
                config.setup_cmd = Some(setup.to_string());
            }
        }
        let settings = repo::normalize_settings(ProjectSettings {
            repos: project.repos.clone(),
            notes: project.notes.clone(),
        })?;
        self.db()
            .write(|c| repo::project_save(c, &project.id, &settings))
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

    /// A delivered run takes a follow-up request (comments of a human code
    /// review...): the leader plans it with what it knows of the run, and
    /// the pipeline goes again from the plan. Merged work: the integration
    /// branch restarts from the base branch as it is now; kept work: it goes
    /// on on the kept branch.
    pub async fn follow_up(self: &Arc<Self>, run_id: &str, request: String) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.stage != stage::DONE
            || (run.status != run_status::MERGED && run.status != run_status::KEPT)
        {
            return Err(Error::msg(
                "Chỉ lượt chạy đã merge hoặc đã giữ nhánh mới làm tiếp được",
            ));
        }
        let request = request.trim().to_string();
        if request.is_empty() {
            return Err(Error::msg("Hãy nhập yêu cầu làm tiếp"));
        }
        let project = self.project(&run)?;
        let mut repos = run.repos.clone();
        for repo in &mut repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            let label = workspace::label(&repo.path);
            // A repo with a pull request goes on from the PR's branch as it
            // is now (rebased, reviewers' commits...), so the next push
            // updates that PR; merged work goes on from the base branch.
            let pr_head = match run.pull_requests.iter().find(|pr| pr.repo == repo.path) {
                Some(pr) => Self::branch_head(&root, &pr.branch).await,
                None => None,
            };
            let head = match pr_head {
                Some(head) => head,
                None if run.status == run_status::MERGED => {
                    git::commit_of(&root, &repo.base_branch)
                        .await
                        .map_err(|error| Error::msg(format!("{label}{error}")))?
                }
                None => continue,
            };
            git::reset_branch(&root, &run.integration_branch, &head)
                .await
                .map_err(|error| Error::msg(format!("{label}{error}")))?;
            repo.base_commit = head;
        }
        self.update_run(run_id, |run| {
            run.followups.push(repo::FollowUp {
                request,
                created_at: now_unix(),
            });
            if let Some(first) = repos.first() {
                run.base_commit = first.base_commit.clone();
            }
            run.repos = repos;
            run.stage = stage::PLAN.into();
            run.status = run_status::RUNNING.into();
            run.status_note = None;
            run.pending_feedback = None;
            run.integration_checks = None;
            run.finished_at = None;
        })?;
        self.drive(run_id);
        Ok(())
    }

    /// The newest commit of `branch`: origin's after fetching it, else the
    /// local branch's.
    async fn branch_head(root: &Path, branch: &str) -> Option<String> {
        let args = ["fetch", "origin", branch];
        let fetch = git::git(root, &args);
        let fetched = matches!(tokio::time::timeout(FETCH_TIMEOUT, fetch).await, Ok(Ok(_)));
        if fetched {
            if let Ok(head) = git::commit_of(root, &format!("refs/remotes/origin/{branch}")).await {
                return Some(head);
            }
        }
        git::commit_of(root, &format!("refs/heads/{branch}"))
            .await
            .ok()
    }

    /// "Gắn PR": records a pull request the user opened by hand. At gate 2
    /// it ends the run as kept; a follow-up then works on the PR's branch.
    pub async fn attach_pull_request(&self, run_id: &str, attach: AttachPr) -> Result<Run> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let at_gate = run.stage == stage::MERGE_GATE;
        let done = run.stage == stage::DONE
            && (run.status == run_status::MERGED || run.status == run_status::KEPT);
        if !at_gate && !done {
            return Err(Error::msg(
                "Gắn PR khi lượt chạy ở Cổng 2 hoặc đã kết thúc (merge / giữ nhánh)",
            ));
        }
        if !run.repos.iter().any(|repo| repo.path == attach.repo) {
            return Err(Error::msg(format!(
                "Lượt chạy không có repo {}",
                attach.repo
            )));
        }
        let Some(link) = context::pr_links(&attach.url).into_iter().next() else {
            return Err(Error::msg(
                "Link PR phải có dạng https://github.com/<org>/<repo>/pull/<số>",
            ));
        };
        let given = |value: &Option<String>| {
            value
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        };
        let (mut branch, mut base) = (given(&attach.branch), given(&attach.base));
        if branch.is_none() || base.is_none() {
            let gh = self.env.find_gh().ok_or_else(|| {
                Error::msg("Không tìm thấy gh để đọc PR; hãy nhập nhánh và nhánh đích")
            })?;
            let out = context::gh_output(
                &gh,
                &["pr", "view", &link.url, "--json", "headRefName,baseRefName"],
            )
            .await
            .map_err(|error| Error::msg(format!("Không đọc được PR: {error}")))?;
            let info: Value = serde_json::from_str(&out)
                .map_err(|error| Error::msg(format!("gh trả về dữ liệu lạ: {error}")))?;
            let field = |key: &str| info.get(key).and_then(Value::as_str).map(str::to_string);
            branch = branch.or_else(|| field("headRefName"));
            base = base.or_else(|| field("baseRefName"));
        }
        let (Some(branch), Some(base)) = (branch, base) else {
            return Err(Error::msg("Thiếu nhánh hoặc nhánh đích của PR"));
        };
        if !valid_branch(&branch) || !valid_branch(&base) {
            return Err(Error::msg("Tên nhánh không hợp lệ: chỉ chữ, số và - _ . /"));
        }
        if at_gate {
            self.cleanup_worktrees(&run, false).await?;
        }
        let pr = repo::PullRequest {
            repo: attach.repo.clone(),
            branch,
            base,
            url: link.url,
        };
        self.update_run(run_id, |run| {
            // One PR per repo: a new link replaces the old one.
            run.pull_requests.retain(|known| known.repo != pr.repo);
            run.pull_requests.push(pr);
            if at_gate {
                run.stage = stage::DONE.into();
                run.status = run_status::KEPT.into();
                run.finished_at = Some(now_unix());
            }
            let urls: Vec<String> = run.pull_requests.iter().map(|p| p.url.clone()).collect();
            run.status_note = Some(format!("PR: {}", urls.join(" · ")));
        })
    }

    /// Files the run changed in a repo, relative to it: what its integration
    /// branch adds to where the run (or its last follow-up) started, and the
    /// files its workers reported.
    async fn run_files(
        &self,
        run: &Run,
        tasks: &[Task],
        repo: &repo::RunRepo,
        root: &Path,
    ) -> Vec<String> {
        let mut files: Vec<String> = git::git(
            root,
            &[
                "diff",
                "--name-only",
                &format!("{}...{}", repo.base_commit, run.integration_branch),
            ],
        )
        .await
        .map(|out| out.lines().map(str::to_string).collect())
        .unwrap_or_default();
        let prefix = workspace::project_path(&repo.path, "");
        for task in tasks.iter().filter(|t| t.status == task_status::MERGED) {
            let Some(line) = task
                .handoff
                .as_deref()
                .and_then(|note| note.lines().find(|l| l.starts_with("Files changed: ")))
            else {
                continue;
            };
            for file in line["Files changed: ".len()..].split(", ") {
                let file = file.split(" (+").next().unwrap_or(file).trim();
                if let Some(file) = file.strip_prefix(&prefix) {
                    if !file.is_empty() && !files.iter().any(|known| known == file) {
                        files.push(file.to_string());
                    }
                }
            }
        }
        files
    }

    /// "Verify PR": reads each pull request of the run on GitHub (its files,
    /// state and the review comments nobody answered), checks it is the
    /// run's (files in common), and has the leader review its code as on
    /// origin. Open comments, or `must` findings, start a follow-up so the
    /// team fixes them, unless a PR looks like the wrong one.
    pub async fn verify_pull_requests(self: &Arc<Self>, run_id: &str) -> Result<Verification> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.pull_requests.is_empty() {
            return Err(Error::msg("Lượt chạy chưa có PR; hãy Gắn PR trước"));
        }
        if run.stage != stage::DONE
            || (run.status != run_status::MERGED && run.status != run_status::KEPT)
        {
            return Err(Error::msg(
                "Verify PR khi lượt chạy đã kết thúc (giữ nhánh / merge)",
            ));
        }
        let gh = self.env.find_gh().ok_or_else(|| {
            Error::msg("Không tìm thấy gh (GitHub CLI); hãy cài và `gh auth login`")
        })?;
        let project = self.project(&run)?;
        let tasks = self.db().read(|c| repo::task_list(c, run_id))?;
        let verify_dir = self.worktree_dir(run_id, "_verify");
        let mut checks = Vec::new();
        let mut diff = String::new();
        let mut reviewable: Vec<String> = Vec::new();
        for pr in &run.pull_requests {
            let Some(config) = run.repos.iter().find(|repo| repo.path == pr.repo) else {
                continue;
            };
            let root = workspace::repo_dir(Path::new(&project.path), &pr.repo);
            let mut check = PrCheck {
                repo: pr.repo.clone(),
                url: pr.url.clone(),
                branch: pr.branch.clone(),
                base: pr.base.clone(),
                ..Default::default()
            };
            let link = context::pr_links(&pr.url).into_iter().next();
            match link {
                None => check.error = Some("Link PR không hợp lệ".into()),
                Some(link) => match context::pr_state(&gh, &link).await {
                    Err(error) => check.error = Some(format!("gh: {error}")),
                    Ok(state) => {
                        let run_files = self.run_files(&run, &tasks, config, &root).await;
                        let matched = run_files
                            .iter()
                            .filter(|file| state.files.contains(file))
                            .count();
                        check.missing = run_files
                            .iter()
                            .filter(|file| !state.files.contains(file))
                            .take(10)
                            .cloned()
                            .collect();
                        check.mismatch = !run_files.is_empty() && matched * 2 < run_files.len();
                        check.run_files = run_files.len();
                        check.matched = matched;
                        check.pr_files = state.files.len();
                        check.state = state.state;
                        check.review_decision = state.review_decision;
                        check.open_comments = state.open_comments;
                        if !state.head.is_empty() {
                            check.branch = state.head;
                        }
                        if !state.base.is_empty() {
                            check.base = state.base;
                        }
                    }
                },
            }
            // The PR's code as on origin, for the leader to read.
            if check.error.is_none() {
                let fetched = git::git(
                    &root,
                    &[
                        "fetch",
                        "origin",
                        check.branch.as_str(),
                        check.base.as_str(),
                    ],
                )
                .await;
                let head = format!("refs/remotes/origin/{}", check.branch);
                let base = format!("refs/remotes/origin/{}", check.base);
                let place = workspace::repo_dir(&verify_dir, &pr.repo);
                match fetched {
                    Ok(_) => match git::detached_worktree(&root, &place, &head).await {
                        Ok(()) => {
                            let prefix = workspace::project_path(&pr.repo, "");
                            diff.push_str(
                                &git::diff_range(&root, &format!("{base}...{head}"), &prefix)
                                    .await
                                    .unwrap_or_default(),
                            );
                            reviewable.push(format!(
                                "- {} ({} → {}){}",
                                check.url,
                                check.branch,
                                check.base,
                                if pr.repo == repo::ROOT_REPO {
                                    String::new()
                                } else {
                                    format!(", repository `{}/`", pr.repo)
                                }
                            ));
                        }
                        Err(error) => check.error = Some(error.to_string()),
                    },
                    Err(error) => check.error = Some(format!("git fetch: {error}")),
                }
            }
            checks.push(check);
        }

        let open: Vec<(&PrCheck, &context::PrComment)> = checks
            .iter()
            .flat_map(|check| check.open_comments.iter().map(move |c| (check, c)))
            .collect();
        let open_text = open
            .iter()
            .map(|(check, comment)| {
                format!(
                    "- [{}] {}{}: {}",
                    comment.author,
                    workspace::project_path(&check.repo, comment.path.as_deref().unwrap_or("")),
                    comment
                        .line
                        .map(|line| format!(":{line}"))
                        .unwrap_or_default(),
                    comment.body.replace('\n', "\n  ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut review = None;
        if !reviewable.is_empty() {
            let (diff, truncated) = git::cut(diff, prompts::REVIEW_DIFF_BYTES);
            let (guides, _) = guides::block(
                &Self::guides(&project),
                Path::new(&project.path),
                &verify_dir,
                None,
                Use::Review,
            );
            let prompt = prompts::verify_pr(
                &run,
                &reviewable.join("\n"),
                &open_text,
                &diff,
                truncated,
                &guides,
            );
            let control = Control {
                pause: AtomicBool::new(false),
                stop: watch::channel(false).0,
            };
            let parse = |value: Value| -> Result<Review> {
                serde_json::from_value(value).map_err(|error| {
                    Error::msg(format!("review does not match the schema: {error}"))
                })
            };
            let answer = self
                .ask_json(
                    &run,
                    None,
                    "verify",
                    &format!("{} · verify PR", run.leader_cli),
                    &verify_dir,
                    None,
                    prompt,
                    &prompts::review_schema(),
                    &control,
                    parse,
                )
                .await;
            match answer {
                Ok((answer, _)) => review = Some(answer),
                Err(error) => {
                    self.remove_workspace(&run, &project, &verify_dir).await;
                    return Err(Error::from(error));
                }
            }
        }
        self.remove_workspace(&run, &project, &verify_dir).await;

        let musts: Vec<&ReviewComment> = review
            .iter()
            .flat_map(|review| review.comments.iter())
            .filter(|comment| comment.severity == "must")
            .collect();
        let mut verification = Verification {
            checked_at: now_unix(),
            prs: checks.clone(),
            review: review.clone(),
            followed_up: false,
            note: None,
        };
        if !open.is_empty() || !musts.is_empty() {
            if let Some(wrong) = checks.iter().find(|check| check.mismatch) {
                verification.note = Some(format!(
                    "Không tự làm tiếp: PR {} chỉ có {}/{} tệp của lượt chạy, có thể gắn nhầm",
                    wrong.url, wrong.matched, wrong.run_files
                ));
            } else {
                let mut request =
                    String::from("Verify PR: xử lý các điểm còn mở trên pull request.\n");
                if !open_text.is_empty() {
                    request.push_str("\nComment review chưa giải quyết trên GitHub:\n");
                    request.push_str(&open_text);
                    request.push('\n');
                }
                if !musts.is_empty() {
                    request.push_str("\nLeader verify (bắt buộc):\n");
                    for comment in &musts {
                        request.push_str(&format!(
                            "- {}{}: {}\n",
                            comment.file,
                            comment
                                .line
                                .map(|line| format!(":{line}"))
                                .unwrap_or_default(),
                            comment.issue
                        ));
                    }
                }
                self.follow_up(run_id, request).await?;
                verification.followed_up = true;
            }
        }
        let dir = self.run_dir(run_id);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join(VERIFICATION_FILE),
            serde_json::to_string_pretty(&verification)
                .map_err(|error| Error::msg(error.to_string()))?,
        )?;
        self.env.changed(run_id);
        Ok(verification)
    }

    /// What gate 2 proposes to push: the repos the run changed, under the
    /// branch of an earlier PR (a follow-up updates it) or the integration
    /// branch's name, into each repo's base; a title and a body from the
    /// request, the tasks and their notes.
    pub async fn pr_draft(&self, run_id: &str) -> Result<PrRequest> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let project = self.project(&run)?;
        let tasks = self.db().read(|c| repo::task_list(c, run_id))?;
        let mut repos = Vec::new();
        for repo in &run.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            if git::ahead(&root, &repo.base_commit, &run.integration_branch).await? == 0 {
                continue;
            }
            let earlier = run.pull_requests.iter().find(|pr| pr.repo == repo.path);
            repos.push(PrRepo {
                repo: repo.path.clone(),
                branch: earlier
                    .map(|pr| pr.branch.clone())
                    .unwrap_or_else(|| run.integration_branch.clone()),
                base: earlier
                    .map(|pr| pr.base.clone())
                    .unwrap_or_else(|| repo.base_branch.clone()),
            });
        }
        let mut body = String::new();
        let request: String = run.request.chars().take(1500).collect();
        body.push_str(request.trim());
        body.push_str("\n\n## Thay đổi\n");
        for task in tasks.iter().filter(|t| t.status == task_status::MERGED) {
            body.push_str(&format!("- `{}` {}\n", task.key, task.title));
            if let Some(note) = &task.handoff {
                for line in note.lines().filter(|l| !l.trim().is_empty()).take(4) {
                    body.push_str(&format!("  > {}\n", line.trim()));
                }
            }
        }
        if let Some(checks) = run.integration_checks.as_ref().filter(|c| !c.is_empty()) {
            body.push_str("\n## Kiểm tra\n");
            for check in checks {
                body.push_str(&format!(
                    "- {} `{}`\n",
                    if check.ok { "✅" } else { "❌" },
                    check.command
                ));
            }
        }
        body.push_str("\n---\nTạo bởi Command Manager · Agent Studio\n");
        Ok(PrRequest {
            repos,
            title: run.title.clone(),
            body,
            draft: false,
        })
    }

    /// Gate 2, instead of merging locally: pushes the integration branch of
    /// each chosen repo (pre-push hooks run, never skipped) and opens its
    /// pull request with gh, or updates the PR an earlier push opened. The
    /// run ends as "kept", its PRs remembered for follow-ups.
    pub async fn open_pull_requests(&self, run_id: &str, request: PrRequest) -> Result<Run> {
        self.require_idle(run_id)?;
        let run = self.at_stage(run_id, stage::MERGE_GATE)?;
        let title = request.title.trim().to_string();
        if title.is_empty() {
            return Err(Error::msg("Hãy nhập tiêu đề PR"));
        }
        if request.repos.is_empty() {
            return Err(Error::msg("Hãy chọn ít nhất một repo"));
        }
        for pr in &request.repos {
            if !run.repos.iter().any(|repo| repo.path == pr.repo) {
                return Err(Error::msg(format!("Lượt chạy không có repo {}", pr.repo)));
            }
            if !valid_branch(pr.branch.trim()) || !valid_branch(pr.base.trim()) {
                return Err(Error::msg(format!(
                    "{}Tên nhánh không hợp lệ: chỉ chữ, số và - _ . /",
                    workspace::label(&pr.repo)
                )));
            }
        }
        let gh = self.env.find_gh().ok_or_else(|| {
            Error::msg("Không tìm thấy gh (GitHub CLI); hãy cài và đăng nhập `gh auth login`")
        })?;
        let project = self.project(&run)?;
        let dir = self.run_dir(run_id);
        std::fs::create_dir_all(&dir)?;
        let body_file = dir.join("pr-body.md");
        std::fs::write(&body_file, &request.body)?;

        let id = format!("agent:pr-{}", uuid::Uuid::new_v4());
        let log = self.env.open_log(&id, "push & tạo PR");
        // Kept alive: the tab's own stop button still cancels.
        let (_stop, stop) = watch::channel(false);
        let mut opened = run.pull_requests.clone();
        let mut outcome: Result<()> = Ok(());
        for pr in &request.repos {
            let (branch, base) = (pr.branch.trim(), pr.base.trim());
            let label = workspace::label(&pr.repo);
            let root = workspace::repo_dir(Path::new(&project.path), &pr.repo);
            let refspec = format!("{}:refs/heads/{branch}", run.integration_branch);
            log.heading(&format!("{label}git push origin {refspec}"));
            let pushed = process::run_program(
                Path::new("git"),
                &["push".into(), "origin".into(), refspec],
                &root,
                PUSH_TIMEOUT,
                &log,
                stop.clone(),
            )
            .await;
            match pushed {
                Ok((true, _, _)) => log.result(true, "Đã push"),
                Ok((false, code, tail)) => {
                    let last = tail.lines().last().unwrap_or_default().to_string();
                    log.result(false, "git push thất bại");
                    outcome = Err(Error::msg(format!(
                        "{label}git push thất bại (mã {}): {last}",
                        code.map_or("?".into(), |c| c.to_string())
                    )));
                    break;
                }
                Err(error) => {
                    log.result(false, &error.to_string());
                    outcome = Err(Error::msg(format!("{label}git push: {error}")));
                    break;
                }
            }
            if let Some(earlier) = opened
                .iter()
                .find(|known| known.repo == pr.repo && known.branch == branch)
            {
                log.result(true, &format!("Đã cập nhật PR {}", earlier.url));
                continue;
            }
            let mut args: Vec<String> = [
                "pr", "create", "--base", base, "--head", branch, "--title", &title,
            ]
            .iter()
            .map(|arg| arg.to_string())
            .collect();
            args.push("--body-file".into());
            args.push(body_file.to_string_lossy().into_owned());
            if request.draft {
                args.push("--draft".into());
            }
            log.heading(&format!(
                "{label}gh pr create --base {base} --head {branch}"
            ));
            let created =
                process::run_program(&gh, &args, &root, GH_TIMEOUT, &log, stop.clone()).await;
            let url = match created {
                Ok((true, _, tail)) => pr_url(&tail),
                // The branch already has an open PR (opened by hand).
                Ok((false, _, tail)) if tail.contains("already exists") => pr_url(&tail),
                Ok((false, _, tail)) => {
                    log.result(false, "gh pr create thất bại");
                    outcome = Err(Error::msg(format!(
                        "{label}gh pr create: {}",
                        tail.lines().last().unwrap_or_default()
                    )));
                    break;
                }
                Err(error) => {
                    outcome = Err(Error::msg(format!("{label}gh pr create: {error}")));
                    break;
                }
            };
            let url = url.unwrap_or_default();
            log.result(true, &format!("PR: {url}"));
            opened.push(repo::PullRequest {
                repo: pr.repo.clone(),
                branch: branch.to_string(),
                base: base.to_string(),
                url,
            });
        }
        self.env.close_log(&id, outcome.is_ok());
        // What was opened is kept even when a later repo failed.
        let saved = opened.clone();
        if let Err(error) = outcome {
            self.update_run(run_id, |run| run.pull_requests = saved)?;
            return Err(error);
        }
        self.cleanup_worktrees(&run, false).await?;
        let note = opened
            .iter()
            .map(|pr| pr.url.clone())
            .filter(|url| !url.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        self.update_run(run_id, |run| {
            run.pull_requests = opened;
            run.stage = stage::DONE.into();
            run.status = run_status::KEPT.into();
            run.status_note = Some(format!("PR: {note}"));
            run.finished_at = Some(now_unix());
        })
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

    /// Changes the models of a run that is not running: the leader's and each
    /// role's. Roles and their CLIs stay; the next steps use the new models.
    pub fn set_models(
        &self,
        run_id: &str,
        leader_model: Option<String>,
        leader_effort: Option<String>,
        roles: Vec<RoleCli>,
    ) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        for role in &roles {
            if !run.roles.iter().any(|r| r.role == role.role) {
                return Err(Error::msg(format!("Không có vai {}", role.role)));
            }
        }
        self.update_run(run_id, |run| {
            run.leader_model = clean(leader_model);
            run.leader_effort = clean(leader_effort);
            for role in &mut run.roles {
                if let Some(new) = roles.iter().find(|r| r.role == role.role) {
                    role.model = clean(new.model.clone());
                    role.effort = clean(new.effort.clone());
                }
            }
        })?;
        Ok(())
    }

    /// Takes effect from the next step: a running agent keeps the timeout it
    /// started with. Remembered as the project's defaults too.
    pub fn set_limits(&self, run_id: &str, limits: Limits) -> Result<()> {
        limits.validate()?;
        let run = self.update_run(run_id, |run| run.limits = limits)?;
        self.db()
            .write(|c| repo::project_save_limits(c, &run.project_id, &limits))
    }

    /// A task's own agent timeout (None: the run's), from its next turn.
    pub fn set_task_timeout(&self, task_id: &str, minutes: Option<u32>) -> Result<()> {
        if minutes.is_some_and(|m| m == 0 || m > repo::MAX_TIMEOUT_MINUTES) {
            return Err(Error::msg(format!(
                "Timeout phải từ 1 đến {} phút",
                repo::MAX_TIMEOUT_MINUTES
            )));
        }
        self.update_task(task_id, |task| task.timeout_minutes = minutes)?;
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
                .map_err(Error::from)?;
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

    /// "Review lại": the leader reviews the task's worktree as it is now
    /// (changes made by hand included), after the checks run again. Approved:
    /// merged; otherwise its review comes back, and a fix round is allowed
    /// while rounds are left.
    pub async fn rereview_task(self: &Arc<Self>, task_id: &str) -> Result<()> {
        let task = self.task_for_action(task_id)?;
        if [task_status::MERGED, task_status::SKIPPED].contains(&task.status.as_str()) {
            return Err(Error::msg("Task đã kết thúc"));
        }
        let Some(base) = task.worktree.clone() else {
            return Err(Error::msg("Task chưa có code để review"));
        };
        let run = self.db().read(|c| repo::run_get(c, &task.run_id))?;
        self.commit_workspace(
            &run,
            Path::new(&base),
            &format!("cm({}): sửa tay trước khi review lại", task.key),
        )
        .await?;
        let (files, added, removed) = self.workspace_changes(&run, Path::new(&base)).await?;
        if files.is_empty() {
            return Err(Error::msg(
                "Worktree của task không có thay đổi nào để review",
            ));
        }
        let outside = git::outside_scope(&task.scope, &files);
        self.update_task(task_id, |task| {
            task.added = added;
            task.removed = removed;
            task.out_of_scope = outside;
            // The review of the work as it is; its round counts like any.
            if task.round + 1 >= run.limits.max_rounds {
                task.round = run.limits.max_rounds.saturating_sub(1);
            }
            task.status = task_status::CHECKING.into();
            task.status_note = None;
        })?;
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
    pub fn reassign_task(
        self: &Arc<Self>,
        task_id: &str,
        cli: &str,
        model: Option<String>,
        effort: Option<String>,
    ) -> Result<()> {
        let task = self.task_for_action(task_id)?;
        self.hand_over(&task, cli, &model, &effort)?;
        self.continue_after_task_action(&task.run_id)
    }

    /// "Giao lại các task lỗi": several tasks of a run to one agent at once;
    /// the run then takes them one after the other.
    pub fn reassign_tasks(
        self: &Arc<Self>,
        run_id: &str,
        task_ids: &[String],
        cli: &str,
        model: Option<String>,
        effort: Option<String>,
        timeout_minutes: Option<u32>,
    ) -> Result<()> {
        if timeout_minutes.is_some_and(|m| m == 0 || m > repo::MAX_TIMEOUT_MINUTES) {
            return Err(Error::msg(format!(
                "Timeout phải từ 1 đến {} phút",
                repo::MAX_TIMEOUT_MINUTES
            )));
        }
        if task_ids.is_empty() {
            return Err(Error::msg("Hãy chọn ít nhất một task"));
        }
        let mut tasks = Vec::new();
        for id in task_ids {
            let task = self.task_for_action(id)?;
            if task.run_id != run_id {
                return Err(Error::msg("Task không thuộc lượt chạy này"));
            }
            tasks.push(task);
        }
        // Checked first: either every task changes or none.
        Cli::parse(cli)?;
        if let Some(task) = tasks.iter().find(|t| t.status == task_status::MERGED) {
            return Err(Error::msg(format!("Task {} đã merge", task.key)));
        }
        for task in &tasks {
            self.hand_over(task, cli, &model, &effort)?;
            if timeout_minutes.is_some() {
                self.update_task(&task.id, |task| task.timeout_minutes = timeout_minutes)?;
            }
        }
        self.continue_after_task_action(run_id)
    }

    /// Gives a task to another agent: a new session in the same worktree.
    fn hand_over(
        &self,
        task: &Task,
        cli: &str,
        model: &Option<String>,
        effort: &Option<String>,
    ) -> Result<()> {
        Cli::parse(cli)?;
        if task.status == task_status::MERGED {
            return Err(Error::msg("Task đã merge"));
        }
        let cli = cli.to_string();
        self.update_task(&task.id, |task| {
            task.cli = cli;
            task.model = clean(model.clone());
            task.effort = clean(effort.clone());
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
        Ok(())
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
            .map_err(Error::from)?;
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
