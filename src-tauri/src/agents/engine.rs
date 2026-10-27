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
use crate::db::repos::settings;
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
    #[serde(alias = "desc", alias = "description")]
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
    /// The leader looks for blockers and raises QA before designing.
    #[serde(default)]
    pub research: bool,
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
    /// Rebase onto the base before pushing (and push with a lease); None:
    /// when the branch needs it (behind its base, or merge commits).
    #[serde(default)]
    pub rebase: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrRepo {
    pub repo: String,
    /// The branch the integration branch is pushed as.
    pub branch: String,
    /// What the PR merges into.
    pub base: String,
}

/// The title and description "Viết bằng AI" fills the PR form with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrText {
    pub title: String,
    pub body: String,
}

/// Where repositories keep their pull request template.
const PR_TEMPLATES: &[&str] = &[
    ".github/pull_request_template.md",
    ".github/PULL_REQUEST_TEMPLATE.md",
    "pull_request_template.md",
    "PULL_REQUEST_TEMPLATE.md",
    "docs/pull_request_template.md",
    "docs/PULL_REQUEST_TEMPLATE.md",
    ".gitlab/merge_request_templates/default.md",
];

/// A person's answer to one QA of the research.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaAnswer {
    pub id: String,
    pub answer: String,
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

/// "Push bằng AI": which agent ships the work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiPush {
    pub cli: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    /// Anything the agent should know (how to resolve a conflict...).
    #[serde(default)]
    pub note: Option<String>,
}

/// What a finished run has not pushed to one of its pull requests yet.
#[derive(Debug, Clone, Serialize)]
pub struct PendingPush {
    pub repo: String,
    pub branch: String,
    pub url: String,
    /// None: origin's copy of the branch is not known here.
    pub commits: Option<u64>,
    /// Behind the PR's base, or merge commits: pushing rebases first and
    /// rewrites the PR branch (with a lease).
    pub needs_rebase: bool,
    pub base: String,
}

/// What a failed push says that matters: a hook's own lines, else the end.
fn push_error(tail: &str) -> String {
    let lines: Vec<&str> = tail
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let hook: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.starts_with("pre-push:") || line.contains("rejected"))
        .take(3)
        .collect();
    if hook.is_empty() {
        lines[lines.len().saturating_sub(4)..].join(" · ")
    } else {
        hook.join(" · ")
    }
}

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

/// The last push that failed, kept in `<run dir>/push-failure.json` until a
/// push goes through.
const PUSH_FAILURE_FILE: &str = "push-failure.json";
/// How much of a failed push's output the leader reads.
const PUSH_OUTPUT_BYTES: usize = 24_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushFailure {
    pub at: String,
    /// The repo whose push failed.
    pub repo: String,
    pub error: String,
    /// The end of the push's log (the pre-push hook's checks), plain text.
    pub output: String,
    /// What was being pushed: pushed again after the fix.
    pub request: PrRequest,
    /// The leader's reading of it.
    #[serde(default)]
    pub diagnosis: Option<String>,
    /// What the leader chose: retry, self, tasks, none.
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub fix_tasks: Vec<String>,
    /// Fix tasks are on it: the run pushes again once they are merged.
    #[serde(default)]
    pub retry: bool,
}

/// The end of a terminal log as plain text: no colours or cursor moves, a
/// line redrawn with `\r` kept as last drawn.
fn plain_tail(bytes: &[u8], max_bytes: usize) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' => {
                let start = out.rfind('\n').map_or(0, |at| at + 1);
                out.truncate(start);
            }
            c => out.push(c),
        }
    }
    let lines: Vec<&str> = out.lines().filter(|line| !line.trim().is_empty()).collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut size = 0;
    for line in lines.iter().rev() {
        size += line.len() + 1;
        if size > max_bytes {
            break;
        }
        kept.push(line);
    }
    kept.reverse();
    kept.join("\n")
}

/// App setting: the folder new runs keep their worktrees in (empty: app data).
pub const WORKTREE_DIR_SETTING: &str = "agents_worktree_dir";

/// How long a push may take: pre-push hooks run the project's checks.
const PUSH_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const GH_TIMEOUT: Duration = Duration::from_secs(3 * 60);

/// Files that pin dependencies: when none changed since a worktree's setup
/// ran, what it installed still holds.
const MANIFESTS: &[&str] = &[
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "package-lock.json",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
    ".npmrc",
    ".nvmrc",
    ".node-version",
    "composer.json",
    "composer.lock",
    "Gemfile",
    "Gemfile.lock",
    "requirements.txt",
    "requirements-dev.txt",
    "pyproject.toml",
    "poetry.lock",
    "uv.lock",
    "Pipfile.lock",
    "go.mod",
    "go.sum",
    "Cargo.toml",
    "Cargo.lock",
];

/// The note a worktree keeps (in its private git folder) of the setup that
/// ran in it.
const SETUP_MARK: &str = "cm-setup.json";

/// Task worktrees are slots: a merged task's slot goes to the next task,
/// installed dependencies included.
const SLOT_PREFIX: &str = "slot-";

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

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
    /// The last push that failed, until one goes through.
    pub push_failure: Option<PushFailure>,
    /// Checks, setup, context and push logs of this session.
    pub side_logs: Vec<SideLog>,
    /// Paused for a usage limit: when the run goes on by itself.
    pub resume_at: Option<i64>,
    /// Worktrees of the run are still on disk.
    pub worktrees: bool,
}

enum Flow {
    Continue,
    Stop,
}

/// A log tab that is not an agent: a task's setup or checks, the run's
/// integration checks, context fetching or pushing.
#[derive(Debug, Clone, Serialize)]
pub struct SideLog {
    /// The terminal id (`agent:check-...`).
    pub id: String,
    pub task_id: Option<String>,
    /// setup, check, context, pr.
    pub kind: String,
    pub title: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub ok: Option<bool>,
}

/// Side logs kept per run.
const SIDE_LOGS_KEPT: usize = 60;

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

impl StepError {
    fn message(self) -> String {
        match self {
            StepError::Cancelled => "đã dừng".into(),
            StepError::Failed(message) => message,
            StepError::Quota { note, .. } => note,
        }
    }
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
    /// Logs of a run that are not agent invocations (checks, setup, context,
    /// pushes), newest last, for the run's terminals. Not kept across
    /// restarts.
    side_logs: Mutex<HashMap<String, Vec<SideLog>>>,
    /// Per run: the leader's session takes one turn at a time.
    leader_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
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
            side_logs: Mutex::new(HashMap::new()),
            leader_locks: Mutex::new(HashMap::new()),
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
            worktrees: self.has_worktrees(run_id),
            run,
            tasks,
            invocations,
            usage,
            driving: self.is_driving(run_id),
            quotas: self.quotas.lock().expect("agent quotas").clone(),
            side_logs: self
                .side_logs
                .lock()
                .expect("agent side logs")
                .get(run_id)
                .cloned()
                .unwrap_or_default(),
            verification: std::fs::read_to_string(self.run_dir(run_id).join(VERIFICATION_FILE))
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok()),
            push_failure: self.push_failure(run_id),
            resume_at: self
                .quota_waits
                .lock()
                .expect("agent quota waits")
                .get(run_id)
                .copied(),
        })
    }

    /// Opens a log tab of `kind` and lists it with the run's terminals.
    fn open_side_log(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        kind: &str,
        title: &str,
    ) -> (String, Arc<AgentLog>) {
        let id = format!("agent:{kind}-{}", uuid::Uuid::new_v4());
        let log = self.env.open_log(&id, title);
        {
            let mut logs = self.side_logs.lock().expect("agent side logs");
            let list = logs.entry(run_id.to_string()).or_default();
            list.push(SideLog {
                id: id.clone(),
                task_id: task_id.map(str::to_string),
                kind: kind.to_string(),
                title: title.to_string(),
                started_at: now_unix(),
                finished_at: None,
                ok: None,
            });
            if list.len() > SIDE_LOGS_KEPT {
                list.remove(0);
            }
        }
        self.env.changed(run_id);
        (id, log)
    }

    fn close_side_log(&self, run_id: &str, id: &str, ok: bool) {
        self.env.close_log(id, ok);
        if let Some(entry) = self
            .side_logs
            .lock()
            .expect("agent side logs")
            .get_mut(run_id)
            .and_then(|list| list.iter_mut().find(|entry| entry.id == id))
        {
            entry.finished_at = Some(now_unix());
            entry.ok = Some(ok);
        }
        self.env.changed(run_id);
    }

    fn run_dir(&self, run_id: &str) -> PathBuf {
        self.env.data_dir().join("agents").join("runs").join(run_id)
    }

    fn worktree_dir(&self, run: &Run, name: &str) -> PathBuf {
        run.worktree_root
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.env.data_dir().join("agents").join("wt"))
            .join(short(&run.id))
            .join(name)
    }

    fn integration_dir(&self, run: &Run) -> PathBuf {
        self.worktree_dir(run, "_integration")
    }

    /// The "agents_worktree_dir" setting for a new run: worktrees and their
    /// builds can take many GB, more than the app data drive may have.
    fn chosen_worktree_root(&self) -> Result<Option<String>> {
        let Some(dir) = self
            .db()
            .read(|c| settings::get(c, WORKTREE_DIR_SETTING))?
            .map(|dir| dir.trim().to_string())
            .filter(|dir| !dir.is_empty())
        else {
            return Ok(None);
        };
        if !Path::new(&dir).is_absolute() {
            return Err(Error::msg(format!(
                "Thư mục worktree \"{dir}\" phải là đường dẫn tuyệt đối"
            )));
        }
        std::fs::create_dir_all(&dir).map_err(|error| {
            Error::msg(format!(
                "Không tạo được thư mục worktree \"{dir}\": {error}"
            ))
        })?;
        Ok(Some(dir))
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

    pub fn set_info(
        &self,
        run_id: &str,
        title: Option<String>,
        request: Option<String>,
    ) -> Result<Run> {
        self.update_run(run_id, |run| {
            if let Some(t) = title {
                let trimmed = t.trim().to_string();
                if !trimmed.is_empty() {
                    run.title = trimmed;
                }
            }
            if let Some(r) = request {
                let trimmed = r.trim().to_string();
                if !trimmed.is_empty() {
                    run.request = trimmed;
                }
            }
        })
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
            stage: if new.research {
                stage::RESEARCH.into()
            } else {
                stage::DESIGN.into()
            },
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
            worktree_root: self.chosen_worktree_root()?,
            research: new.research.then(repo::Research::default),
            leader_chat: Vec::new(),
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
            stage::QA_GATE => return Err(Error::msg("Lượt chạy đang chờ trả lời QA (Cổng 0)")),
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
                stage::RESEARCH => self.research(&run, control).await,
                stage::QA_GATE => self.wait(&run, "Chờ trả lời QA (Cổng 0)"),
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
        self.invoke_in(
            run,
            task,
            cli_name,
            step_name,
            title,
            access,
            cwd,
            resume,
            prompt,
            schema,
            control,
            &[],
        )
        .await
    }

    /// `invoke`, with folders outside `cwd` the agent may read (or write,
    /// by its access) too.
    #[allow(clippy::too_many_arguments)]
    async fn invoke_in(
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
        extra: &[PathBuf],
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
            // Pushing (rebase, conflicts, fixing checks) takes the lead's model.
            "research" | "design" | "plan" | "push" => Phase::Lead,
            "review" | "verify" | "prepare" | "chat" | "describe" => Phase::Review,
            "triage" => Phase::Lead,
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
        extra_dirs.extend(extra.iter().cloned());
        // Pushing writes to the repository's own git folder too.
        if access == Access::Push {
            if let Ok(common) = git::common_dir(cwd).await {
                extra_dirs.push(common);
            }
        }
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
                Access::Push => "ghi + rebase + push lên PR",
            },
            if profile.bypass_permissions && access != Access::ReadOnly {
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
        self.ask_json_in(
            run,
            task,
            step_name,
            title,
            cwd,
            resume,
            prompt,
            schema,
            control,
            parse,
            &[],
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn ask_json_in<T>(
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
        extra: &[PathBuf],
    ) -> Step<(T, Outcome)> {
        let leader = run.leader_cli.clone();
        let mut outcome = self
            .invoke_in(
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
                extra,
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
                .invoke_in(
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
                    extra,
                )
                .await?;
        }
        unreachable!("the loop returns")
    }

    // ---- the leader's one session -------------------------------------------

    /// One leader turn at a time per run: they all go on in the same
    /// session.
    fn leader_lock(&self, run_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        Arc::clone(
            self.leader_locks
                .lock()
                .expect("agent leader locks")
                .entry(run_id.to_string())
                .or_default(),
        )
    }

    /// The code a leader turn is about lives in `place` (a worktree), not in
    /// the project folder the turn runs in (Claude keeps a session per
    /// folder, so every turn of the one session runs there).
    fn relocate(prompt: String, place: Option<&Path>) -> String {
        let Some(place) = place else {
            return prompt;
        };
        let at = format!("`{}`", place.display());
        let body = prompt
            .replace("The current directory", &format!("The folder {at}"))
            .replace("the current directory", &format!("the folder {at}"))
            .replace("this worktree", &format!("the worktree {at}"));
        format!(
            "For this step the code to work on is in {at}, not in your working directory \
             (the user's own checkout: leave it alone). Read the files there and run commands \
             there (`cd {} && ...`, `git -C ...`).\n\n{body}",
            place.display()
        )
    }

    /// The session is gone (another machine, cleared CLI data): the turn
    /// starts a new one.
    fn lost_session(message: &str) -> bool {
        let message = message.to_lowercase();
        message.contains("no conversation found")
            || (message.contains("session") && message.contains("not found"))
    }

    fn keep_leader_session(&self, run_id: &str, outcome: &Outcome) -> Result<()> {
        if let Some(session) = outcome.session_id.clone() {
            self.update_run(run_id, |run| run.leader_session = Some(session))?;
        }
        Ok(())
    }

    /// A leader turn with a JSON answer, in the run's one leader session.
    #[allow(clippy::too_many_arguments)]
    async fn leader_json<T>(
        &self,
        run_id: &str,
        task: Option<&Task>,
        step_name: &str,
        title: &str,
        place: Option<&Path>,
        prompt: String,
        schema: &Value,
        control: &Control,
        parse: impl Fn(Value) -> Result<T>,
    ) -> Step<(T, Outcome)> {
        let lock = self.leader_lock(run_id);
        let _turn = lock.lock().await;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let project = self.project(&run)?;
        let cwd = PathBuf::from(&project.path);
        let extra: Vec<PathBuf> = place.into_iter().map(Path::to_path_buf).collect();
        let prompt = Self::relocate(prompt, place);
        let first = self
            .ask_json_in(
                &run,
                task,
                step_name,
                title,
                &cwd,
                run.leader_session.as_deref(),
                prompt.clone(),
                schema,
                control,
                &parse,
                &extra,
            )
            .await;
        let (answer, outcome) = match first {
            Err(StepError::Failed(message))
                if run.leader_session.is_some() && Self::lost_session(&message) =>
            {
                self.ask_json_in(
                    &run, task, step_name, title, &cwd, None, prompt, schema, control, &parse,
                    &extra,
                )
                .await?
            }
            other => other?,
        };
        self.keep_leader_session(run_id, &outcome)?;
        Ok((answer, outcome))
    }

    /// A read-only leader turn with a text answer, in the run's one leader
    /// session.
    #[allow(clippy::too_many_arguments)]
    async fn leader_text(
        &self,
        run_id: &str,
        task: Option<&Task>,
        step_name: &str,
        title: &str,
        place: Option<&Path>,
        prompt: String,
        control: &Control,
    ) -> Step<Outcome> {
        let lock = self.leader_lock(run_id);
        let _turn = lock.lock().await;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let project = self.project(&run)?;
        let cwd = PathBuf::from(&project.path);
        let extra: Vec<PathBuf> = place.into_iter().map(Path::to_path_buf).collect();
        let prompt = Self::relocate(prompt, place);
        let mut resume = run.leader_session.clone();
        let mut outcome = loop {
            let outcome = self
                .invoke_in(
                    &run,
                    task,
                    &run.leader_cli,
                    step_name,
                    title,
                    Access::ReadOnly,
                    &cwd,
                    resume.as_deref(),
                    prompt.clone(),
                    None,
                    control,
                    &extra,
                )
                .await?;
            let lost = outcome.error.as_deref().is_some_and(Self::lost_session);
            if lost && resume.is_some() {
                resume = None;
                continue;
            }
            break outcome;
        };
        if let Some(error) = outcome.error.take() {
            return Err(StepError::Failed(format!(
                "Leader ({}): {error}",
                run.leader_cli
            )));
        }
        self.keep_leader_session(run_id, &outcome)?;
        Ok(outcome)
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
        let (id, log) = self.open_side_log(&run.id, None, "context", "chuẩn bị ngữ cảnh");
        context::fetch_prs(&self.context_dir(&run.id), text, &log, refresh).await;
        self.close_side_log(&run.id, &id, true);
    }

    /// Before the design: the leader looks for what blocks the work. QA
    /// marked as blocking stops the run at gate 0; otherwise the design
    /// goes on in the same session.
    async fn research(&self, run: &Run, control: &Control) -> Step<Flow> {
        // The leader's one session: a chat turn may have moved it on.
        let lock = self.leader_lock(&run.id);
        let _turn = lock.lock().await;
        let run = &self.db().read(|c| repo::run_get(c, &run.id))?;
        let project = self.project(run)?;
        let feedback = run
            .pending_feedback
            .clone()
            .filter(|_| run.leader_session.is_some());
        let earlier = run.research.as_ref().and_then(|r| r.report.clone());
        let (prompt, resume, title) = match &feedback {
            Some(feedback) => (
                prompts::research_again(earlier.as_ref(), feedback),
                run.leader_session.as_deref(),
                format!("{} · tìm hiểu lại", run.leader_cli),
            ),
            None => {
                self.prepare_context(run, &run.request, false).await;
                let overview = context::overview(Path::new(&project.path), &run.repos).await;
                let files = context::files(&self.context_dir(&run.id));
                let here = Path::new(&project.path);
                let (guides, _) =
                    guides::block(&Self::guides(&project), here, here, None, Use::Lead);
                (
                    prompts::research(run, &project, &overview, &files, &guides),
                    None,
                    format!("{} · tìm hiểu", run.leader_cli),
                )
            }
        };
        let parse = |value: Value| -> Result<repo::ResearchReport> {
            let mut report: repo::ResearchReport =
                serde_json::from_value(value).map_err(|error| {
                    Error::msg(format!("report does not match the schema: {error}"))
                })?;
            let mut seen: Vec<String> = Vec::new();
            for (index, qa) in report.qa.iter_mut().enumerate() {
                qa.id = qa.id.trim().to_string();
                if qa.id.is_empty() || seen.contains(&qa.id) {
                    qa.id = format!("Q{}", index + 1);
                }
                seen.push(qa.id.clone());
                if qa.severity != "blocker" {
                    qa.severity = "question".into();
                }
                qa.answer = None;
            }
            Ok(report)
        };
        let (mut report, outcome) = self
            .ask_json(
                run,
                None,
                "research",
                &title,
                Path::new(&project.path),
                resume,
                prompt,
                &prompts::research_schema(),
                control,
                parse,
            )
            .await?;
        report.at = now_unix();
        let blocked = report.blocked();
        self.update_run(&run.id, |run| {
            run.research = Some(repo::Research {
                report: Some(report),
                approved: !blocked,
            });
            if outcome.session_id.is_some() {
                run.leader_session = outcome.session_id;
            }
            run.pending_feedback = None;
            run.stage = if blocked {
                stage::QA_GATE.into()
            } else {
                stage::DESIGN.into()
            };
        })?;
        Ok(Flow::Continue)
    }

    /// Gate 0: the person's answers to the QA (by id); the design goes on
    /// with them. Every blocking question needs one.
    pub fn answer_research(self: &Arc<Self>, run_id: &str, answers: Vec<QaAnswer>) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.at_stage(run_id, stage::QA_GATE)?;
        let mut research = run.research.clone().unwrap_or_default();
        let Some(report) = research.report.as_mut() else {
            return Err(Error::msg("Chưa có báo cáo tìm hiểu"));
        };
        for qa in &mut report.qa {
            qa.answer = answers
                .iter()
                .find(|a| a.id == qa.id)
                .map(|a| a.answer.trim().to_string())
                .filter(|a| !a.is_empty());
        }
        let missing: Vec<String> = report
            .qa
            .iter()
            .filter(|qa| qa.severity == "blocker" && qa.answer.is_none())
            .map(|qa| qa.id.clone())
            .collect();
        if !missing.is_empty() {
            return Err(Error::msg(format!(
                "Hãy trả lời (hoặc dùng giả định của leader cho) các câu chặn: {}",
                missing.join(", ")
            )));
        }
        research.approved = true;
        self.update_run(run_id, |run| {
            run.research = Some(research);
            run.stage = stage::DESIGN.into();
            run.status = run_status::RUNNING.into();
            run.status_note = None;
        })?;
        self.drive(run_id);
        Ok(())
    }

    /// Gate 0: back to the leader to look again, with what the person says
    /// (and the answers given so far).
    pub fn research_again(
        self: &Arc<Self>,
        run_id: &str,
        feedback: String,
        answers: Vec<QaAnswer>,
    ) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.at_stage(run_id, stage::QA_GATE)?;
        let feedback = feedback.trim().to_string();
        if feedback.is_empty() {
            return Err(Error::msg("Hãy nhập điều leader cần tìm hiểu lại"));
        }
        let mut research = run.research.clone().unwrap_or_default();
        if let Some(report) = research.report.as_mut() {
            for qa in &mut report.qa {
                qa.answer = answers
                    .iter()
                    .find(|a| a.id == qa.id)
                    .map(|a| a.answer.trim().to_string())
                    .filter(|a| !a.is_empty());
            }
        }
        self.update_run(run_id, |run| {
            run.research = Some(research);
            run.pending_feedback = Some(feedback);
            run.stage = stage::RESEARCH.into();
            run.status = run_status::RUNNING.into();
            run.status_note = None;
        })?;
        self.drive(run_id);
        Ok(())
    }

    async fn design(&self, run: &Run, control: &Control) -> Step<Flow> {
        // The leader's one session: a chat turn may have moved it on.
        let lock = self.leader_lock(&run.id);
        let _turn = lock.lock().await;
        let run = &self.db().read(|c| repo::run_get(c, &run.id))?;
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
            (None, None) if run.leader_session.is_some() && run.research.is_some() => {
                let report = run
                    .research
                    .as_ref()
                    .and_then(|research| research.report.clone())
                    .unwrap_or_default();
                (
                    prompts::design_after_research(run, &project, &report),
                    run.leader_session.as_deref(),
                    format!("{} · thiết kế", run.leader_cli),
                )
            }
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
        // The leader's one session: a chat turn may have moved it on.
        let lock = self.leader_lock(&run.id);
        let _turn = lock.lock().await;
        let run = &self.db().read(|c| repo::run_get(c, &run.id))?;
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
                    kind: repo::task_kind::AGENT.into(),
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
                        kind: draft.kind.clone(),
                        human: (draft.kind == repo::task_kind::HUMAN)
                            .then(repo::HumanTest::default),
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
        let dir = self.integration_dir(run);
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
            task_status::TRIAGE,
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
        let testing: Vec<&str> = tasks
            .iter()
            .filter(|t| t.status == task_status::AWAITING_HUMAN)
            .map(|t| t.key.as_str())
            .collect();
        let note = match (blocked, testing.is_empty()) {
            (0, false) => format!("Chờ người test: {}", testing.join(", ")),
            (_, false) => format!(
                "{blocked} task cần người xem; chờ người test: {}",
                testing.join(", ")
            ),
            _ => format!("{blocked} task cần người xem trước khi tiếp tục"),
        };
        self.wait(run, &note)
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
        if task.is_human() {
            return match task.status.as_str() {
                task_status::TRIAGE => self.triage_human(run, integration, task, control).await,
                _ => {
                    self.prepare_human(run, project, integration, task, control)
                        .await
                }
            };
        }
        match task.status.as_str() {
            task_status::CHECKING => self.check_task(run, project, task, control).await,
            task_status::REVIEW => self.review_task(run, integration, task, control).await,
            _ => self.worker_turn(run, project, task, control).await,
        }
    }

    /// The tasks a person's task is about: what it depends on (the work to
    /// test and the fixes made since), merged.
    fn tested_tasks<'a>(tasks: &'a [Task], task: &Task) -> Vec<&'a Task> {
        tasks
            .iter()
            .filter(|t| task.depends_on.contains(&t.key) && t.status == task_status::MERGED)
            .collect()
    }

    fn human_dir(&self, run_id: &str, task: &Task) -> PathBuf {
        self.context_dir(run_id).join("human").join(&task.key)
    }

    /// A person's task comes up: the AI prepares what its description asks
    /// for (a test guide), then it waits for the person.
    async fn prepare_human(
        &self,
        run: &Run,
        project: &Project,
        integration: &Path,
        task: &Task,
        control: &Control,
    ) -> Step<()> {
        let tasks = self.db().read(|c| repo::task_list(c, &run.id))?;
        let done = Self::tested_tasks(&tasks, task);
        let human = task.human.clone().unwrap_or_default();
        let (guides, _) = guides::block(
            &Self::guides(project),
            Path::new(&project.path),
            integration,
            Some(&task.repos),
            Use::Review,
        );
        let files = context::files(&self.context_dir(&run.id));
        let prompt = prompts::prepare_human(run, task, &done, human.rounds.last(), &guides, &files);
        let parse = |value: Value| -> Result<repo::TestGuide> {
            serde_json::from_value(value)
                .map_err(|error| Error::msg(format!("guide does not match the schema: {error}")))
        };
        let (guide, _) = self
            .leader_json(
                &run.id,
                Some(task),
                "prepare",
                &format!("{} · chuẩn bị {}", run.leader_cli, task.key),
                Some(integration),
                prompt,
                &prompts::test_guide_schema(),
                control,
                parse,
            )
            .await?;
        self.update_task(&task.id, |task| {
            let human = task.human.get_or_insert_with(Default::default);
            human.guide = Some(guide);
            human.leader_note = None;
            task.status = task_status::AWAITING_HUMAN.into();
            task.status_note = Some("Chờ người test".into());
        })?;
        Ok(())
    }

    /// A person reported failures: the leader confirms them and makes fix
    /// tasks; the person's task waits for them, then comes up again. No bug
    /// found: the leader says why and the person decides.
    async fn triage_human(
        &self,
        run: &Run,
        integration: &Path,
        task: &Task,
        control: &Control,
    ) -> Step<()> {
        let tasks = self.db().read(|c| repo::task_list(c, &run.id))?;
        let done = Self::tested_tasks(&tasks, task);
        let human = task.human.clone().unwrap_or_default();
        let Some(round) = human.rounds.last().cloned() else {
            self.update_task(&task.id, |task| {
                task.status = task_status::AWAITING_HUMAN.into()
            })?;
            return Ok(());
        };
        let dir = self.human_dir(&run.id, task);
        let shots: Vec<String> = round
            .attachments
            .iter()
            .map(|name| dir.join(name).to_string_lossy().into_owned())
            .collect();
        let chat = human
            .chat
            .iter()
            .map(|m| {
                format!(
                    "{}: {}",
                    if m.role == "user" { "Person" } else { "You" },
                    m.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let keys: Vec<String> = tasks.iter().map(|t| t.key.clone()).collect();
        let roles: Vec<String> = run.roles.iter().map(|r| r.role.clone()).collect();
        let repos: Vec<String> = run.repos.iter().map(|r| r.path.clone()).collect();
        let prompt = prompts::triage_human(run, task, &round, &done, &chat, &shots, &keys);
        let parse = |value: Value| -> Result<(String, Vec<PlanTask>)> {
            #[derive(Deserialize)]
            struct Answer {
                summary: String,
                tasks: Vec<PlanTask>,
            }
            let answer: Answer = serde_json::from_value(value).map_err(|error| {
                Error::msg(format!("answer does not match the schema: {error}"))
            })?;
            Ok((answer.summary, answer.tasks))
        };
        let ((summary, planned), _) = self
            .leader_json(
                &run.id,
                Some(task),
                "triage",
                &format!("{} · xác nhận lỗi {}", run.leader_cli, task.key),
                Some(integration),
                prompt,
                &prompts::triage_schema(&roles, &repos),
                control,
                parse,
            )
            .await?;
        let leader_says = repo::ChatMessage {
            role: "leader".into(),
            text: summary.clone(),
            at: now_unix(),
        };
        if planned.is_empty() {
            self.update_task(&task.id, |task| {
                let human = task.human.get_or_insert_with(Default::default);
                human.leader_note = Some(summary);
                human.chat.push(leader_says);
                task.status = task_status::AWAITING_HUMAN.into();
                task.status_note = Some("Leader không thấy lỗi; xem ghi chú rồi quyết định".into());
            })?;
            return Ok(());
        }
        // New keys, the fixes first in order, the test after them.
        let mut used = keys;
        let mut drafts = Vec::new();
        for planned in planned {
            let mut key = planned.key.trim().to_string();
            let base = key.clone();
            let mut n = 2;
            while used.contains(&key) {
                key = format!("{base}-{n}");
                n += 1;
            }
            used.push(key.clone());
            let mut description = planned.description;
            if !shots.is_empty() && !description.contains("human/") {
                description.push_str(&format!(
                    "\n\nẢnh chụp người test gửi: {}",
                    shots.join(", ")
                ));
            }
            drafts.push(TaskDraft {
                cli: Self::role_cli(run, &planned.role),
                key,
                title: planned.title,
                role: planned.role,
                description,
                acceptance: planned.acceptance,
                scope: planned.scope,
                depends_on: Vec::new(),
                repos: planned.repos,
                kind: repo::task_kind::AGENT.into(),
            });
        }
        let first = tasks.iter().map(|t| t.sort_order + 1).max().unwrap_or(0);
        let fix_keys = self.insert_fix_tasks(run, drafts, &task.repos, task.batch, first)?;
        self.update_task(&task.id, |task| {
            let human = task.human.get_or_insert_with(Default::default);
            if let Some(round) = human.rounds.last_mut() {
                round.fix_tasks = fix_keys.clone();
            }
            human.chat.push(leader_says);
            for key in &fix_keys {
                if !task.depends_on.contains(key) {
                    task.depends_on.push(key.clone());
                }
            }
            // Tested again once the fixes are merged.
            task.sort_order = first + fix_keys.len() as i64;
            task.status = task_status::WAITING.into();
            task.status_note = Some(format!("Chờ sửa: {}", fix_keys.join(", ")));
        })?;
        Ok(())
    }

    /// Fix tasks the leader planned, added to the run (waiting) after its
    /// other tasks. Their keys.
    fn insert_fix_tasks(
        &self,
        run: &Run,
        drafts: Vec<TaskDraft>,
        default_repos: &[String],
        batch: u32,
        first: i64,
    ) -> Result<Vec<String>> {
        let drafts = Self::fill_repos(run, drafts);
        let keys: Vec<String> = drafts.iter().map(|d| d.key.clone()).collect();
        self.db().write(|c| {
            for (index, draft) in drafts.iter().enumerate() {
                repo::task_insert(
                    c,
                    &Task {
                        id: uuid::Uuid::new_v4().to_string(),
                        run_id: run.id.clone(),
                        key: draft.key.clone(),
                        title: draft.title.trim().to_string(),
                        role: draft.role.trim().to_lowercase(),
                        cli: draft.cli.clone(),
                        description: draft.description.clone(),
                        acceptance: draft.acceptance.clone(),
                        scope: draft.scope.clone(),
                        depends_on: Vec::new(),
                        repos: if draft.repos.is_empty() {
                            default_repos.to_vec()
                        } else {
                            draft.repos.clone()
                        },
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
                        kind: repo::task_kind::AGENT.into(),
                        human: None,
                    },
                )?;
            }
            Ok(())
        })?;
        Ok(keys)
    }

    /// Planned tasks as drafts with keys no task of the run has.
    fn fix_drafts(run: &Run, used: &[String], planned: Vec<PlanTask>) -> Vec<TaskDraft> {
        let mut used = used.to_vec();
        let mut drafts = Vec::new();
        for planned in planned {
            let mut key = planned.key.trim().to_string();
            let base = key.clone();
            let mut n = 2;
            while used.contains(&key) {
                key = format!("{base}-{n}");
                n += 1;
            }
            used.push(key.clone());
            drafts.push(TaskDraft {
                cli: Self::role_cli(run, &planned.role),
                key,
                title: planned.title,
                role: planned.role,
                description: planned.description,
                acceptance: planned.acceptance,
                scope: planned.scope,
                depends_on: Vec::new(),
                repos: planned.repos,
                kind: repo::task_kind::AGENT.into(),
            });
        }
        drafts
    }

    /// A person's result for the round. Passed: the task is done; failed:
    /// the leader goes through it next.
    pub fn submit_human_test(
        self: &Arc<Self>,
        task_id: &str,
        results: Vec<repo::CaseResult>,
        passed: bool,
        note: String,
        attachments: Vec<String>,
    ) -> Result<()> {
        let task = self.db().read(|c| repo::task_get(c, task_id))?;
        if !task.is_human() || task.status != task_status::AWAITING_HUMAN {
            return Err(Error::msg("Task này không chờ kết quả test"));
        }
        if !passed && note.trim().is_empty() && results.iter().all(|r| r.note.trim().is_empty()) {
            return Err(Error::msg(
                "Hãy ghi lại lỗi đã thấy (ở từng case hoặc nhận xét chung)",
            ));
        }
        self.update_task(task_id, |task| {
            let human = task.human.get_or_insert_with(Default::default);
            human.rounds.push(repo::HumanRound {
                at: now_unix(),
                guide: human.guide.clone(),
                results,
                passed,
                note: note.trim().to_string(),
                attachments,
                fix_tasks: Vec::new(),
            });
            human.leader_note = None;
            if passed {
                task.status = task_status::PASSED.into();
                task.status_note = Some("Người test: đạt".into());
            } else {
                task.status = task_status::TRIAGE.into();
                task.status_note = Some("Leader đang xác nhận lỗi".into());
            }
        })?;
        self.continue_after_task_action(&task.run_id)
    }

    /// "Tạo lại hướng dẫn": the AI prepares the person's task again.
    pub fn reprepare_human(self: &Arc<Self>, task_id: &str) -> Result<()> {
        let task = self.db().read(|c| repo::task_get(c, task_id))?;
        if !task.is_human() || task.status != task_status::AWAITING_HUMAN {
            return Err(Error::msg("Task này không chờ người test"));
        }
        self.update_task(task_id, |task| {
            task.status = task_status::WAITING.into();
            task.status_note = None;
        })?;
        self.continue_after_task_action(&task.run_id)
    }

    /// The person asks the leader while testing; the conversation goes on
    /// in one session per task.
    pub async fn human_chat(&self, task_id: &str, message: String) -> Result<repo::ChatMessage> {
        let message = message.trim().to_string();
        if message.is_empty() {
            return Err(Error::msg("Hãy nhập câu hỏi"));
        }
        let task = self.db().read(|c| repo::task_get(c, task_id))?;
        if !task.is_human() {
            return Err(Error::msg("Chỉ hỏi leader trong task người test"));
        }
        let run = self.db().read(|c| repo::run_get(c, &task.run_id))?;
        let project = self.project(&run)?;
        let integration = self.integration_dir(&run);
        let cwd = if integration.exists() {
            integration
        } else {
            PathBuf::from(&project.path)
        };
        let control = Control {
            pause: AtomicBool::new(false),
            stop: watch::channel(false).0,
        };
        // In the leader's one session: it knows the run, the plan and the
        // person's earlier questions.
        let outcome = self
            .leader_text(
                &run.id,
                Some(&task),
                "chat",
                &format!("{} · hỏi đáp {}", run.leader_cli, task.key),
                (cwd != Path::new(&project.path)).then_some(cwd.as_path()),
                prompts::human_chat(&run, &task, &message),
                &control,
            )
            .await
            .map_err(|error| Error::msg(error.message()))?;
        let answer = repo::ChatMessage {
            role: "leader".into(),
            text: outcome.text.trim().to_string(),
            at: now_unix(),
        };
        let asked = repo::ChatMessage {
            role: "user".into(),
            text: message,
            at: now_unix(),
        };
        let reply = answer.clone();
        self.update_task(task_id, |task| {
            let human = task.human.get_or_insert_with(Default::default);
            human.chat.push(asked);
            human.chat.push(answer);
        })?;
        Ok(reply)
    }

    /// "Chat với leader": a question about the run, answered in the
    /// leader's one session (read-only), kept with the run.
    pub async fn leader_chat(&self, run_id: &str, message: String) -> Result<repo::ChatMessage> {
        let message = message.trim().to_string();
        if message.is_empty() {
            return Err(Error::msg("Hãy nhập câu hỏi"));
        }
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let tasks = self.db().read(|c| repo::task_list(c, run_id))?;
        let integration = self.integration_dir(&run);
        let place = integration.exists().then_some(integration);
        let asked = repo::ChatMessage {
            role: "user".into(),
            text: message.clone(),
            at: now_unix(),
        };
        // Shown at once; the answer follows.
        self.update_run(run_id, |run| run.leader_chat.push(asked))?;
        let control = Control {
            pause: AtomicBool::new(false),
            stop: watch::channel(false).0,
        };
        let outcome = self
            .leader_text(
                run_id,
                None,
                "chat",
                &format!("{} · chat", run.leader_cli),
                place.as_deref(),
                prompts::leader_chat(&run, &tasks, place.is_some(), &message),
                &control,
            )
            .await;
        let answer = repo::ChatMessage {
            role: "leader".into(),
            text: match &outcome {
                Ok(outcome) => outcome.text.trim().to_string(),
                Err(_) => String::new(),
            },
            at: now_unix(),
        };
        match outcome {
            Ok(_) => {
                let reply = answer.clone();
                self.update_run(run_id, |run| run.leader_chat.push(answer))?;
                Ok(reply)
            }
            Err(error) => {
                let message = error.message();
                self.update_run(run_id, |run| {
                    run.leader_chat.push(repo::ChatMessage {
                        role: "error".into(),
                        text: message.clone(),
                        at: now_unix(),
                    })
                })?;
                Err(Error::msg(message))
            }
        }
    }

    /// A screenshot for a person's task, kept with the run's context so
    /// every agent can read it. Returns its file name.
    pub fn human_attach(&self, task_id: &str, name: &str, bytes: &[u8]) -> Result<String> {
        const MAX_BYTES: usize = 15 * 1024 * 1024;
        if bytes.len() > MAX_BYTES {
            return Err(Error::msg("Ảnh quá lớn (tối đa 15 MB)"));
        }
        let task = self.db().read(|c| repo::task_get(c, task_id))?;
        let dir = self.human_dir(&task.run_id, &task);
        std::fs::create_dir_all(&dir)?;
        let clean: String = name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let file = format!("{}-{}", now_unix(), clean.trim_start_matches('.'));
        std::fs::write(dir.join(&file), bytes)?;
        Ok(file)
    }

    /// A screenshot of a person's task, to show it.
    pub fn human_file(&self, task_id: &str, name: &str) -> Result<Vec<u8>> {
        if name.contains('/') || name.contains('\\') || name.starts_with('.') {
            return Err(Error::msg("Tên tệp không hợp lệ"));
        }
        let task = self.db().read(|c| repo::task_get(c, task_id))?;
        Ok(std::fs::read(
            self.human_dir(&task.run_id, &task).join(name),
        )?)
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
        let fresh = task.worktree.is_none();
        let base = match &task.worktree {
            Some(path) => PathBuf::from(path),
            None => self.free_slot(run)?,
        };
        // Every repo is there, so the agent can read the others too.
        for repo in &run.repos {
            let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
            let start = git::commit_of(&root, &run.integration_branch).await?;
            git::ensure_branch(&root, &branch, &start).await?;
            let dir = workspace::repo_dir(&base, &repo.path);
            if fresh && dir.join(".git").exists() {
                // A slot an earlier task left: same folder, new branch.
                git::switch_worktree(&dir, &branch).await?;
            } else {
                git::ensure_worktree(&root, &dir, &branch).await?;
            }
        }
        let task = if fresh {
            self.update_task(&task.id, |task| {
                task.branch = Some(branch);
                task.worktree = Some(base.to_string_lossy().into_owned());
            })?
        } else {
            task.clone()
        };
        self.ensure_setup(
            &run.id,
            Some(&task.id),
            project,
            &base,
            &task.repos,
            &format!("chuẩn bị · {}", task.key),
            stop,
        )
        .await?;
        Ok(task)
    }

    /// A task worktree folder no unfinished task holds: the first one free,
    /// else a new one.
    fn free_slot(&self, run: &Run) -> Result<PathBuf> {
        let tasks = self.db().read(|c| repo::task_list(c, &run.id))?;
        let held: Vec<PathBuf> = tasks
            .iter()
            .filter_map(|task| task.worktree.as_ref().map(PathBuf::from))
            .collect();
        Ok((1..)
            .map(|n| self.worktree_dir(run, &format!("{SLOT_PREFIX}{n}")))
            .find(|slot| !held.contains(slot))
            .expect("a free slot"))
    }

    /// Runs the setup commands of `repos` in a workspace, unless they already
    /// ran in that worktree and nothing pinning dependencies changed since:
    /// a reused worktree keeps what it installed.
    #[allow(clippy::too_many_arguments)]
    async fn ensure_setup(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        project: &Project,
        base: &Path,
        repos: &[String],
        title: &str,
        stop: watch::Receiver<bool>,
    ) -> Step<()> {
        let mut setups = Vec::new();
        for repo in repos {
            let Some(config) = project.repo(repo) else {
                continue;
            };
            let Some(setup) = config
                .setup_cmd
                .clone()
                .filter(|cmd| !cmd.trim().is_empty())
            else {
                continue;
            };
            let dir = workspace::repo_dir(base, repo);
            if Self::needs_setup(&dir, &setup).await {
                setups.push((repo.clone(), setup, config.check_timeout(), dir));
            }
        }
        if setups.is_empty() {
            return Ok(());
        }
        let (id, log) = self.open_side_log(run_id, task_id, "setup", title);
        for (repo, setup, timeout, dir) in setups {
            log.heading(&format!("{}$ {setup}", workspace::label(&repo)));
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
                Ok((true, _, _)) => Self::mark_setup(&dir, &setup).await,
                Err(RunError::Cancelled) => {
                    self.close_side_log(run_id, &id, false);
                    return Err(StepError::Cancelled);
                }
                Ok((false, code, _)) => {
                    self.close_side_log(run_id, &id, false);
                    return Err(StepError::Failed(format!(
                        "{}Lệnh chuẩn bị worktree \"{setup}\" thất bại (mã {})",
                        workspace::label(&repo),
                        code.map_or("?".into(), |c| c.to_string())
                    )));
                }
                Err(error) => {
                    self.close_side_log(run_id, &id, false);
                    return Err(StepError::Failed(error.to_string()));
                }
            }
        }
        self.close_side_log(run_id, &id, true);
        Ok(())
    }

    /// Whether `setup` has to run in a worktree: it never ran there, it
    /// changed, or a dependency manifest changed since.
    async fn needs_setup(dir: &Path, setup: &str) -> bool {
        let Ok(git_dir) = git::git_dir(dir).await else {
            return true;
        };
        let mark: Option<Value> = std::fs::read(git_dir.join(SETUP_MARK))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let field = |key: &str| {
            mark.as_ref()
                .and_then(|mark| mark.get(key))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        let (Some(cmd), Some(commit)) = (field("cmd"), field("commit")) else {
            return true;
        };
        if cmd != setup {
            return true;
        }
        let Ok(head) = git::commit_of(dir, "HEAD").await else {
            return true;
        };
        if head == commit {
            return false;
        }
        match git::changed_between(dir, &commit, &head).await {
            Ok(files) => files.iter().any(|file| {
                let name = file.rsplit('/').next().unwrap_or(file);
                MANIFESTS.contains(&name)
            }),
            Err(_) => true,
        }
    }

    async fn mark_setup(dir: &Path, setup: &str) {
        let (Ok(git_dir), Ok(head)) = (git::git_dir(dir).await, git::commit_of(dir, "HEAD").await)
        else {
            return;
        };
        let mark = serde_json::json!({ "cmd": setup, "commit": head });
        let _ = std::fs::write(git_dir.join(SETUP_MARK), mark.to_string());
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
        let commit_style = self.commit_style(run, &worktree, &task.repos).await;
        let shared = prompts::Shared {
            design_file: design_file.as_deref(),
            files: &files,
            done: &done,
            guides: &guides,
            commit_style: &commit_style,
        };
        let interrupted =
            self.interrupted_session(&run.id, Some(&task.id), &task.cli, &["implement", "fix"])?;
        let (prompt, resume, step) = match (&interrupted, &review, &task.worker_session) {
            (Some(session), _, _) => (
                prompts::resume_step(&format!(
                    "{}\n{}",
                    prompts::HANDOFF_ASK,
                    prompts::commit_ask(&commit_style)
                )),
                Some(session.clone()),
                if fixing { "fix" } else { "implement" },
            ),
            (None, Some(review), Some(session)) if fixing => (
                prompts::fix(review, task.checks.as_deref().unwrap_or(&[]), &commit_style),
                Some(session.clone()),
                "fix",
            ),
            // A new agent (reassigned) gets the whole task plus the review.
            (None, Some(review), None) if fixing => (
                format!(
                    "{}\n\n{}",
                    prompts::implement(run, project, &task, earlier_work, &shared),
                    prompts::fix(review, task.checks.as_deref().unwrap_or(&[]), &commit_style)
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
        let message = match prompts::commit_message(&outcome.text) {
            Some(message) => {
                self.checked_commit_message(
                    run,
                    &task,
                    &worktree,
                    message,
                    outcome.session_id.as_deref(),
                    control,
                )
                .await
            }
            // No message given: the app's own.
            None => format!(
                "cm({}): {}{}",
                task.key,
                task.title,
                if task.round > 0 {
                    format!(" (round {})", task.round + 1)
                } else {
                    String::new()
                }
            ),
        };
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
        let handoff = Self::handoff(&prompts::without_commit(&outcome.text), &files);
        self.update_task(&task.id, |task| {
            task.added = added;
            task.removed = removed;
            task.out_of_scope = outside;
            task.handoff = Some(handoff);
            task.status = task_status::CHECKING.into();
        })?;
        Ok(())
    }

    /// How the task's repositories write commit messages, for its worker.
    async fn commit_style(&self, run: &Run, base: &Path, repos: &[String]) -> String {
        let mut parts = Vec::new();
        for repo in run.repos.iter().filter(|repo| repos.contains(&repo.path)) {
            let dir = workspace::repo_dir(base, &repo.path);
            let remote = format!("refs/remotes/origin/{}", repo.base_branch);
            let base_ref = if git::commit_of(&dir, &remote).await.is_ok() {
                format!("origin/{}", repo.base_branch)
            } else {
                repo.base_branch.clone()
            };
            let style = git::commit_style(&dir, &base_ref).await;
            if !style.is_empty() {
                parts.push(if repos.len() > 1 || repo.path != repo::ROOT_REPO {
                    format!("Repository `{}`:\n{style}", repo.path)
                } else {
                    style
                });
            }
        }
        parts.join("\n\n")
    }

    /// The worker's commit message, checked with the repositories' own
    /// commit-msg hooks (commitlint...); refused, the worker gets one try to
    /// correct it in its session. A message still refused is used anyway:
    /// the app commits without hooks, and the PR's squash may rewrite it.
    async fn checked_commit_message(
        &self,
        run: &Run,
        task: &Task,
        base: &Path,
        message: String,
        session: Option<&str>,
        control: &Control,
    ) -> String {
        let check = |message: String| async move {
            for repo in run
                .repos
                .iter()
                .filter(|repo| task.repos.contains(&repo.path))
            {
                let dir = workspace::repo_dir(base, &repo.path);
                git::check_commit_msg(&dir, &message).await?;
            }
            Ok::<(), String>(())
        };
        let Err(refusal) = check(message.clone()).await else {
            return message;
        };
        let Some(session) = session else {
            return message;
        };
        let answer = self
            .invoke(
                run,
                Some(task),
                &task.cli,
                "commit",
                &format!("{} · {} · commit message", task.cli, task.key),
                Access::ReadOnly,
                base,
                Some(session),
                prompts::commit_retry(&message, &refusal),
                None,
                control,
            )
            .await;
        match answer
            .ok()
            .and_then(|outcome| prompts::commit_message(&outcome.text))
        {
            Some(corrected) => corrected,
            None => message,
        }
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
        run_id: &str,
        task_id: Option<&str>,
        jobs: &[CheckJob],
        title: &str,
        control: &Control,
    ) -> Step<Vec<CheckResult>> {
        if jobs.is_empty() {
            return Ok(Vec::new());
        }
        let (id, log) = self.open_side_log(run_id, task_id, "check", title);
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
                    self.close_side_log(run_id, &id, false);
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
        self.close_side_log(run_id, &id, results.iter().all(|check| check.ok));
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
            .run_checks(
                &run.id,
                Some(&task.id),
                &jobs,
                &format!("kiểm tra · {}", task.key),
                control,
            )
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
                Err(MergeError::Failed(message)) => format!(
                    "{}Merge không thành công (không phải xung đột code): {message}",
                    workspace::label(&repo.path)
                ),
            };
            self.update_task(&task.id, |task| {
                task.status = task_status::CONFLICT.into();
                task.status_note = Some(note);
            })?;
            return Ok(());
        }
        if let Some(base) = &task.worktree {
            self.release_workspace(run, Path::new(base)).await?;
        }
        self.update_task(&task.id, |task| {
            task.status = task_status::MERGED.into();
            task.status_note = None;
            task.worktree = None;
        })?;
        Ok(())
    }

    /// A task is done with its worktree: a slot stays for the next task (off
    /// the task's branch, so that can be deleted), anything else goes.
    async fn release_workspace(&self, run: &Run, base: &Path) -> Result<()> {
        let slot = base
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(SLOT_PREFIX));
        if !slot {
            let project = self.project(run)?;
            self.remove_workspace(run, &project, base).await;
            return Ok(());
        }
        for repo in &run.repos {
            let dir = workspace::repo_dir(base, &repo.path);
            if dir.join(".git").exists() {
                git::detach(&dir).await?;
            }
        }
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
        let setup = if jobs.is_empty() {
            Ok(())
        } else {
            self.ensure_setup(
                &run.id,
                None,
                &project,
                &integration,
                &repos,
                "chuẩn bị · tích hợp",
                control.stop.subscribe(),
            )
            .await
        };
        let checks = match setup {
            Ok(()) => {
                self.run_checks(&run.id, None, &jobs, "kiểm tra · tích hợp", control)
                    .await?
            }
            Err(StepError::Failed(message)) => vec![CheckResult {
                command: "setup".into(),
                ok: false,
                exit_code: None,
                output: message,
                duration_ms: 0,
            }],
            Err(error) => return Err(error),
        };
        let checks_ok = checks.iter().all(|check| check.ok);
        self.update_run(&run.id, |run| {
            run.integration_checks = Some(checks);
            run.stage = stage::MERGE_GATE.into();
        })?;
        // Fix tasks for a failed push are merged: push it again.
        if let Some(failure) = self.push_failure(&run.id).filter(|failure| failure.retry) {
            if !checks_ok {
                return self.wait(run, "Kiểm tra tích hợp lỗi nên chưa push lại; xem Cổng 2");
            }
            return match self
                .push_pull_requests(&run.id, failure.request.clone())
                .await
            {
                Ok(_) => Ok(Flow::Stop),
                Err(error) => self.wait(
                    run,
                    &format!("Push lại vẫn lỗi: {error} — bấm \"Leader xử lý lỗi push\""),
                ),
            };
        }
        // A follow-up asked to update its pull requests when done.
        let push = run
            .followups
            .last()
            .is_some_and(|followup| followup.push_pr)
            && !run.pull_requests.is_empty();
        if push {
            if !checks_ok {
                return self.wait(
                    run,
                    "Kiểm tra tích hợp lỗi nên chưa push lên PR; xem Cổng 2",
                );
            }
            let mut draft = self.pr_draft(&run.id).await?;
            // Only the PRs there are: no new one without the user.
            draft
                .repos
                .retain(|repo| run.pull_requests.iter().any(|pr| pr.repo == repo.repo));
            if draft.repos.is_empty() {
                return self.wait(run, "Không có thay đổi mới để push lên PR");
            }
            return match self.push_pull_requests(&run.id, draft).await {
                Ok(_) => Ok(Flow::Stop),
                Err(error) => self.wait(run, &format!("Push lên PR thất bại: {error}")),
            };
        }
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

    /// Removes every worktree of the run (task slots, the integration one,
    /// what verifying and pushing left) and the branches of merged tasks,
    /// or of every task.
    async fn cleanup_worktrees(&self, run: &Run, delete_branches: bool) -> Result<()> {
        let project = self.project(run)?;
        let tasks = self.db().read(|c| repo::task_list(c, &run.id))?;
        let mut places: Vec<PathBuf> = tasks
            .iter()
            .filter_map(|task| task.worktree.as_ref().map(PathBuf::from))
            .collect();
        if let Ok(entries) = std::fs::read_dir(self.worktree_dir(run, "")) {
            places.extend(entries.flatten().map(|entry| entry.path()));
        }
        for place in &places {
            self.remove_workspace(run, &project, place).await;
        }
        self.remove_workspace(run, &project, &self.integration_dir(run))
            .await;
        let _ = std::fs::remove_dir(self.worktree_dir(run, ""));
        self.tidy_branches(run, &tasks, delete_branches).await;
        self.db().write(|c| {
            for mut task in tasks {
                task.worktree = None;
                repo::task_save(c, &task)?;
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Deletes the branches of merged tasks (their work lives on in the
    /// integration branch), or of every task. A branch some worktree still
    /// has checked out stays.
    async fn tidy_branches(&self, run: &Run, tasks: &[Task], all: bool) {
        let Ok(project) = self.project(run) else {
            return;
        };
        for task in tasks {
            let Some(branch) = &task.branch else {
                continue;
            };
            if all || task.status == task_status::MERGED {
                for repo in &run.repos {
                    let root = workspace::repo_dir(Path::new(&project.path), &repo.path);
                    git::delete_branch(&root, branch).await;
                }
            }
        }
    }

    /// The run is delivered: worktrees stay (the leader's keeps its installed
    /// dependencies for pushes and follow-ups), merged task branches go.
    async fn delivered(&self, run: &Run) -> Result<()> {
        let tasks = self.db().read(|c| repo::task_list(c, &run.id))?;
        self.tidy_branches(run, &tasks, false).await;
        Ok(())
    }

    /// "Dọn worktree": frees the disk a finished run holds. A follow-up
    /// creates the worktrees again.
    pub async fn clean_worktrees(&self, run_id: &str) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.stage != stage::DONE {
            return Err(Error::msg("Chỉ dọn worktree khi lượt chạy đã kết thúc"));
        }
        self.cleanup_worktrees(&run, false).await
    }

    /// Whether a run still has worktrees on disk.
    pub fn has_worktrees(&self, run_id: &str) -> bool {
        let run = match self.db().read(|c| repo::run_get(c, run_id)) {
            Ok(run) => run,
            Err(_) => return false,
        };
        std::fs::read_dir(self.worktree_dir(&run, ""))
            .is_ok_and(|mut entries| entries.next().is_some())
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
        self.delivered(&run).await?;
        self.update_run(run_id, |run| {
            run.stage = stage::DONE.into();
            run.status = run_status::MERGED.into();
            run.status_note = None;
            run.finished_at = Some(now_unix());
        })?;
        Ok(())
    }

    /// Gate 2: keep the integration branch (and the worktrees, for pushes
    /// and follow-ups).
    pub async fn keep_branch(&self, run_id: &str) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.at_stage(run_id, stage::MERGE_GATE)?;
        self.delivered(&run).await?;
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
    pub async fn follow_up(
        self: &Arc<Self>,
        run_id: &str,
        request: String,
        push_pr: bool,
    ) -> Result<()> {
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
            self.move_integration(&run, &root, &repo.path, &head)
                .await
                .map_err(|error| Error::msg(format!("{label}{error}")))?;
            repo.base_commit = head;
        }
        self.update_run(run_id, |run| {
            run.followups.push(repo::FollowUp {
                request,
                created_at: now_unix(),
                // Only where there is a pull request to update.
                push_pr: push_pr && !run.pull_requests.is_empty(),
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

    /// Points the run's integration branch of a repo at `head`: in the
    /// leader's worktree when there is one (its branch is checked out
    /// there), else the branch alone.
    async fn move_integration(&self, run: &Run, root: &Path, repo: &str, head: &str) -> Result<()> {
        let dir = workspace::repo_dir(&self.integration_dir(run), repo);
        if dir.join(".git").exists() {
            git::reset_worktree(&dir, head).await
        } else {
            git::reset_branch(root, &run.integration_branch, head).await
        }
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
            self.delivered(&run).await?;
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
        let verify_dir = self.worktree_dir(&run, "_verify");
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
                .leader_json(
                    &run.id,
                    None,
                    "verify",
                    &format!("{} · verify PR", run.leader_cli),
                    Some(verify_dir.as_path()),
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
                // Gate 2 shows what was done before anything is pushed.
                self.follow_up(run_id, request, false).await?;
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

    /// Per pull request of a finished run: the commits of its integration
    /// branch that origin's copy of the PR branch (as last fetched) lacks;
    /// None when that copy is not known here.
    pub async fn pending_pushes(&self, run_id: &str) -> Result<Vec<PendingPush>> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let project = self.project(&run)?;
        let mut pending = Vec::new();
        for pr in &run.pull_requests {
            let root = workspace::repo_dir(Path::new(&project.path), &pr.repo);
            let remote = format!("refs/remotes/origin/{}", pr.branch);
            let commits = match git::commit_of(&root, &remote).await {
                Ok(_) => git::ahead(&root, &remote, &run.integration_branch)
                    .await
                    .ok(),
                Err(_) => None,
            };
            let needs_rebase = git::needs_rebase(
                &root,
                &format!("refs/remotes/origin/{}", pr.base),
                &run.integration_branch,
            )
            .await;
            pending.push(PendingPush {
                repo: pr.repo.clone(),
                branch: pr.branch.clone(),
                url: pr.url.clone(),
                commits,
                needs_rebase,
                base: pr.base.clone(),
            });
        }
        Ok(pending)
    }

    /// "Push bằng AI": an agent rebases the run's work onto each PR's base,
    /// resolves conflicts, fixes what the checks and hooks refuse, and pushes
    /// with a lease, in a worktree of its own. Runs in the background like a
    /// driver (it can be stopped); success is what origin shows afterwards.
    pub fn ai_push(self: &Arc<Self>, run_id: &str, request: AiPush) -> Result<()> {
        self.require_idle(run_id)?;
        Cli::parse(&request.cli)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.pull_requests.is_empty() {
            return Err(Error::msg("Lượt chạy chưa có PR; hãy Gắn PR trước"));
        }
        let at_gate = run.stage == stage::MERGE_GATE;
        let done = run.stage == stage::DONE
            && (run.status == run_status::KEPT || run.status == run_status::MERGED);
        if !at_gate && !done {
            return Err(Error::msg(
                "Push bằng AI ở Cổng 2 hoặc khi lượt chạy đã kết thúc",
            ));
        }
        let control = {
            let mut drivers = self.drivers.lock().expect("agent drivers");
            if drivers.contains_key(run_id) {
                return Err(Error::msg("Lượt chạy đang chạy"));
            }
            let control = Arc::new(Control {
                pause: AtomicBool::new(false),
                stop: watch::channel(false).0,
            });
            drivers.insert(run_id.to_string(), Arc::clone(&control));
            control
        };
        let _ = self.update_run(run_id, |run| {
            run.status_note = Some(format!("{} đang push lên PR…", request.cli));
        });
        let engine = Arc::clone(self);
        let run_id = run_id.to_string();
        tokio::spawn(async move {
            let result = engine.ai_push_inner(&run_id, &request, &control).await;
            // The outcome is saved before the run counts as idle again.
            let _ = engine.update_run(&run_id, |run| match &result {
                Ok(urls) => {
                    run.status_note = Some(format!("AI đã push lên PR: {}", urls.join(" · ")));
                    if run.stage == stage::MERGE_GATE {
                        run.stage = stage::DONE.into();
                        run.status = run_status::KEPT.into();
                        run.finished_at = Some(now_unix());
                    }
                }
                Err(error) => run.status_note = Some(format!("Push bằng AI chưa xong: {error}")),
            });
            engine
                .drivers
                .lock()
                .expect("agent drivers")
                .remove(&run_id);
        });
        Ok(())
    }

    async fn ai_push_inner(
        &self,
        run_id: &str,
        request: &AiPush,
        control: &Control,
    ) -> Result<Vec<String>> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let project = self.project(&run)?;
        let tasks = self.db().read(|c| repo::task_list(c, run_id))?;
        // The agent's model and effort for this step.
        let shipping = Run {
            leader_model: clean(request.model.clone()).or(run.leader_model.clone()),
            leader_effort: clean(request.effort.clone()).or(run.leader_effort.clone()),
            ..run.clone()
        };
        // The leader's worktree, on the integration branch: dependencies
        // are installed there once and stay for the next push.
        let dir = self.ensure_integration(&run, &project).await?;
        let mut pushed: Vec<String> = Vec::new();
        let mut outcome: Result<()> = Ok(());
        for pr in &run.pull_requests {
            let root = workspace::repo_dir(Path::new(&project.path), &pr.repo);
            let label = workspace::label(&pr.repo);
            let _ = git::git(&root, &["fetch", "origin", &pr.base]).await;
            let _ = git::git(&root, &["fetch", "origin", &pr.branch]).await;
            let place = workspace::repo_dir(&dir, &pr.repo);
            git::abort_in_progress(&place).await;
            if let Err(error) = self
                .ensure_setup(
                    &run.id,
                    None,
                    &project,
                    &dir,
                    std::slice::from_ref(&pr.repo),
                    "chuẩn bị · push",
                    control.stop.subscribe(),
                )
                .await
            {
                outcome = Err(Error::msg(error.message()));
                break;
            }
            let (guides, _) = guides::block(
                &Self::guides(&project),
                Path::new(&project.path),
                &dir,
                Some(std::slice::from_ref(&pr.repo)),
                Use::Work,
            );
            let done: Vec<&Task> = tasks
                .iter()
                .filter(|t| t.status == task_status::MERGED && t.repos.contains(&pr.repo))
                .collect();
            let prompt = prompts::ai_push(
                &run,
                &run.integration_branch,
                &pr.branch,
                &pr.base,
                &pr.url,
                &done,
                request.note.as_deref().unwrap_or(""),
                &guides,
            );
            let answer = self
                .invoke(
                    &shipping,
                    None,
                    &request.cli,
                    "push",
                    &format!("{} · push PR {}", request.cli, pr.branch),
                    Access::Push,
                    &place,
                    None,
                    prompt,
                    None,
                    control,
                )
                .await;
            // A rebase or merge the agent left half done.
            git::abort_in_progress(&place).await;
            let head = git::commit_of(&place, "HEAD").await.ok();
            let reply = match answer {
                Ok(outcome) => match outcome.error {
                    Some(error) => Err(error),
                    None => Ok(outcome.text),
                },
                Err(StepError::Cancelled) => Err("đã dừng".into()),
                Err(StepError::Failed(message)) => Err(message),
                Err(StepError::Quota { note, .. }) => Err(note),
            };
            // What counts is what origin has now.
            let _ = git::git(&root, &["fetch", "origin", &pr.branch]).await;
            let remote = git::commit_of(&root, &format!("refs/remotes/origin/{}", pr.branch))
                .await
                .ok();
            match (reply, head) {
                (Ok(_), Some(head)) if remote.as_deref() == Some(head.as_str()) => {
                    pushed.push(pr.url.clone());
                }
                (Ok(reply), _) => {
                    outcome = Err(Error::msg(format!(
                        "{label}nhánh {} trên origin chưa phải bản agent làm ra. Agent trả lời: {}",
                        pr.branch,
                        adapter::shorten(reply.trim(), 300)
                    )));
                }
                (Err(error), _) => outcome = Err(Error::msg(format!("{label}{error}"))),
            }
            if outcome.is_err() {
                break;
            }
        }
        if run.stage == stage::MERGE_GATE && outcome.is_ok() {
            self.delivered(&run).await?;
        }
        // The integration branch is what the agent pushed: the run goes on
        // from there.
        outcome?;
        Ok(pushed)
    }

    /// "Push lên PR" on a finished run: its integration branch onto the
    /// branch of each pull request it has (fast-forward, hooks run, no new
    /// PR). The run stays finished.
    pub async fn push_to_pull_requests(&self, run_id: &str) -> Result<Run> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.pull_requests.is_empty() {
            return Err(Error::msg("Lượt chạy chưa có PR; hãy Gắn PR trước"));
        }
        let mut draft = self.pr_draft(run_id).await?;
        draft
            .repos
            .retain(|repo| run.pull_requests.iter().any(|pr| pr.repo == repo.repo));
        if draft.repos.is_empty() {
            return Err(Error::msg("Không có thay đổi nào để push lên PR"));
        }
        self.push_pull_requests(run_id, draft).await
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
        let qa = run
            .research
            .as_ref()
            .and_then(|research| research.report.as_ref())
            .map(|report| report.qa.as_slice())
            .unwrap_or_default();
        if !qa.is_empty() {
            body.push_str("\n## QA\n");
            for item in qa {
                body.push_str(&format!("- {} {}\n", item.id, item.question.trim()));
                match item.answer.as_deref() {
                    Some(answer) => body.push_str(&format!("  → {}\n", answer.trim())),
                    None => body.push_str(&format!(
                        "  → (chưa có câu trả lời; làm theo giả định: {})\n",
                        item.assumption.trim()
                    )),
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
            rebase: None,
        })
    }

    fn push_failure(&self, run_id: &str) -> Option<PushFailure> {
        std::fs::read_to_string(self.run_dir(run_id).join(PUSH_FAILURE_FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
    }

    fn save_push_failure(&self, run_id: &str, failure: Option<&PushFailure>) -> Result<()> {
        let path = self.run_dir(run_id).join(PUSH_FAILURE_FILE);
        match failure {
            Some(failure) => {
                std::fs::create_dir_all(self.run_dir(run_id))?;
                std::fs::write(
                    path,
                    serde_json::to_vec_pretty(failure).map_err(|e| Error::msg(e.to_string()))?,
                )?;
            }
            None => {
                let _ = std::fs::remove_file(path);
            }
        }
        self.env.changed(run_id);
        Ok(())
    }

    /// "Bỏ qua" on a failed push: forgets it.
    pub fn dismiss_push_failure(&self, run_id: &str) -> Result<()> {
        self.require_idle(run_id)?;
        self.save_push_failure(run_id, None)
    }

    /// "Leader xử lý lỗi push": the leader reads why the push failed and
    /// pushes again as it is, fixes it itself and pushes, or hands fix tasks
    /// to the team (the run pushes again once they are merged). Runs in the
    /// background like a driver.
    pub fn fix_push(self: &Arc<Self>, run_id: &str, note: String) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if self.push_failure(run_id).is_none() {
            return Err(Error::msg("Không có lần push lỗi nào để xử lý"));
        }
        let delivered = run.stage == stage::DONE
            && (run.status == run_status::KEPT || run.status == run_status::MERGED);
        if run.stage != stage::MERGE_GATE && !delivered {
            return Err(Error::msg(
                "Xử lý lỗi push khi lượt chạy ở Cổng 2 hoặc đã kết thúc",
            ));
        }
        let control = {
            let mut drivers = self.drivers.lock().expect("agent drivers");
            if drivers.contains_key(run_id) {
                return Err(Error::msg("Lượt chạy đang chạy"));
            }
            let control = Arc::new(Control {
                pause: AtomicBool::new(false),
                stop: watch::channel(false).0,
            });
            drivers.insert(run_id.to_string(), Arc::clone(&control));
            control
        };
        let _ = self.update_run(run_id, |run| {
            run.status_note = Some("Leader đang xem vì sao push lỗi…".into());
        });
        let engine = Arc::clone(self);
        let run_id = run_id.to_string();
        tokio::spawn(async move {
            let result = engine.fix_push_inner(&run_id, &note, &control).await;
            // The outcome is saved before the run counts as idle again.
            match &result {
                Ok(None) => {}
                Ok(Some(note)) => {
                    let _ = engine.update_run(&run_id, |run| run.status_note = Some(note.clone()));
                }
                Err(error) => {
                    let _ = engine.update_run(&run_id, |run| {
                        run.status_note = Some(format!("Leader chưa xử lý xong lỗi push: {error}"))
                    });
                }
            }
            engine
                .drivers
                .lock()
                .expect("agent drivers")
                .remove(&run_id);
            if matches!(result, Ok(None)) {
                engine.drive(&run_id);
            }
        });
        Ok(())
    }

    /// None: fix tasks were added and the run drives on; else the note to
    /// show.
    async fn fix_push_inner(
        &self,
        run_id: &str,
        note: &str,
        control: &Control,
    ) -> Result<Option<String>> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let project = self.project(&run)?;
        let tasks = self.db().read(|c| repo::task_list(c, run_id))?;
        let Some(mut failure) = self.push_failure(run_id) else {
            return Err(Error::msg("Không có lần push lỗi nào để xử lý"));
        };
        let integration = self.ensure_integration(&run, &project).await?;
        let target = failure
            .request
            .repos
            .iter()
            .map(|pr| {
                format!(
                    "{}`{}` → `{}`",
                    workspace::label(&pr.repo),
                    pr.branch,
                    pr.base
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let repos: Vec<String> = failure
            .request
            .repos
            .iter()
            .map(|pr| pr.repo.clone())
            .collect();
        let (guides, _) = guides::block(
            &Self::guides(&project),
            Path::new(&project.path),
            &integration,
            Some(&repos),
            Use::Lead,
        );
        let done: Vec<&Task> = tasks
            .iter()
            .filter(|task| task.status == task_status::MERGED)
            .collect();
        let keys: Vec<String> = tasks.iter().map(|t| t.key.clone()).collect();
        let roles: Vec<String> = run.roles.iter().map(|r| r.role.clone()).collect();
        let run_repos: Vec<String> = run.repos.iter().map(|r| r.path.clone()).collect();
        let prompt = prompts::push_failure(
            &run,
            &target,
            &failure.error,
            &failure.output,
            &done,
            &keys,
            note,
            &guides,
        );
        let parse = |value: Value| -> Result<(String, String, Vec<PlanTask>)> {
            #[derive(Deserialize)]
            struct Answer {
                summary: String,
                action: String,
                tasks: Vec<PlanTask>,
            }
            let answer: Answer = serde_json::from_value(value).map_err(|error| {
                Error::msg(format!("answer does not match the schema: {error}"))
            })?;
            Ok((answer.summary, answer.action, answer.tasks))
        };
        let ((summary, action, planned), _) = self
            .leader_json(
                &run.id,
                None,
                "triage",
                &format!("{} · xem lỗi push", run.leader_cli),
                Some(integration.as_path()),
                prompt,
                &prompts::push_failure_schema(&roles, &run_repos),
                control,
                parse,
            )
            .await
            .map_err(|error| Error::msg(error.message()))?;
        let action = if action == "tasks" && planned.is_empty() {
            "none".to_string()
        } else {
            action
        };
        failure.diagnosis = Some(summary.clone());
        failure.action = Some(action.clone());
        self.save_push_failure(run_id, Some(&failure))?;

        match action.as_str() {
            "tasks" => {
                let drafts = Self::fix_drafts(&run, &keys, planned);
                let first = tasks.iter().map(|t| t.sort_order + 1).max().unwrap_or(0);
                let batch = tasks.iter().map(|t| t.batch).max().unwrap_or(0);
                let fix_keys = self.insert_fix_tasks(&run, drafts, &repos, batch, first)?;
                failure.fix_tasks = fix_keys.clone();
                failure.retry = true;
                self.save_push_failure(run_id, Some(&failure))?;
                self.update_run(run_id, |run| {
                    run.stage = stage::IMPLEMENT.into();
                    run.status = run_status::RUNNING.into();
                    run.status_note = Some(format!(
                        "Sửa lỗi push: {} — xong sẽ push lại",
                        fix_keys.join(", ")
                    ));
                    run.integration_checks = None;
                    run.finished_at = None;
                })?;
                Ok(None)
            }
            "retry" => Ok(Some(self.push_again(run_id, &failure, &summary).await)),
            "self" => {
                // Its own session, in the worktree it writes to (the leader's
                // session lives in the user's checkout, read-only).
                let leader = run.leader_cli.clone();
                let answer = self
                    .invoke(
                        &run,
                        None,
                        &leader,
                        "push",
                        &format!("{leader} · sửa lỗi push"),
                        Access::Write,
                        &integration,
                        None,
                        prompts::push_fix_self(
                            &run,
                            &target,
                            &summary,
                            &failure.output,
                            &guides,
                            &self.commit_style(&run, &integration, &repos).await,
                        ),
                        None,
                        control,
                    )
                    .await
                    .map_err(|error| Error::msg(error.message()))?;
                if let Some(error) = answer.error {
                    return Err(Error::msg(error));
                }
                for repo in &run.repos {
                    git::abort_in_progress(&workspace::repo_dir(&integration, &repo.path)).await;
                }
                let message = prompts::commit_message(&answer.text)
                    .unwrap_or_else(|| "cm(push-fix): fix what the push refused".into());
                let committed = self.commit_workspace(&run, &integration, &message).await?;
                if !committed {
                    return Ok(Some(format!(
                        "Leader không sửa gì: {}",
                        adapter::shorten(answer.text.trim(), 400)
                    )));
                }
                Ok(Some(self.push_again(run_id, &failure, &summary).await))
            }
            _ => Ok(Some(format!("Leader: {summary}"))),
        }
    }

    /// Pushes what failed again; the note says how it went.
    async fn push_again(&self, run_id: &str, failure: &PushFailure, summary: &str) -> String {
        match self
            .push_pull_requests(run_id, failure.request.clone())
            .await
        {
            Ok(run) => format!(
                "Đã push lại sau khi leader xử lý ({}). {}",
                adapter::shorten(summary.trim(), 200),
                run.status_note.unwrap_or_default()
            ),
            Err(error) => format!("Push lại vẫn lỗi: {error}"),
        }
    }

    /// Runs the repo's pre-push hook (core.hooksPath honoured) the way git
    /// would for pushing HEAD to `branch` on origin. No hook: nothing to do.
    async fn run_pre_push(
        &self,
        run_id: &str,
        place: &Path,
        branch: &str,
        remote_head: Option<&str>,
        log: &AgentLog,
        stop: watch::Receiver<bool>,
    ) -> std::result::Result<(), String> {
        let Ok(rel) = git::git(place, &["rev-parse", "--git-path", "hooks/pre-push"]).await else {
            return Ok(());
        };
        let hook = place.join(rel);
        if !is_executable(&hook) {
            return Ok(());
        }
        let head = git::commit_of(place, "HEAD")
            .await
            .map_err(|error| error.to_string())?;
        let url = git::git(place, &["remote", "get-url", "--push", "origin"])
            .await
            .map_err(|error| error.to_string())?;
        // What git feeds the hook on stdin.
        let input = self.run_dir(run_id).join("pre-push-stdin");
        std::fs::create_dir_all(self.run_dir(run_id)).map_err(|error| error.to_string())?;
        std::fs::write(
            &input,
            format!(
                "HEAD {head} refs/heads/{branch} {}\n",
                remote_head.unwrap_or("0000000000000000000000000000000000000000")
            ),
        )
        .map_err(|error| error.to_string())?;
        log.heading("pre-push hook (chạy trước, rồi git push --no-verify)");
        let args: Vec<String> = vec![
            "-c".into(),
            "\"$0\" \"$1\" \"$2\" < \"$3\"".into(),
            hook.to_string_lossy().into_owned(),
            "origin".into(),
            url,
            input.to_string_lossy().into_owned(),
        ];
        let result =
            process::run_program(Path::new("sh"), &args, place, PUSH_TIMEOUT, log, stop).await;
        let _ = std::fs::remove_file(&input);
        match result {
            Ok((true, _, _)) => {
                log.result(true, "pre-push hook qua");
                Ok(())
            }
            Ok((false, code, tail)) => {
                log.result(false, "pre-push hook thất bại");
                Err(format!(
                    "pre-push hook thất bại (mã {}): {}",
                    code.map_or("?".into(), |c| c.to_string()),
                    push_error(&tail)
                ))
            }
            Err(error) => {
                log.result(false, &error.to_string());
                Err(format!("pre-push hook: {error}"))
            }
        }
    }

    /// "Viết bằng AI" in the PR form: the leader writes the title and
    /// description from the run's work, following the project's rules and
    /// skills, the repository's PR template and its commit style.
    pub async fn pr_ai_draft(
        &self,
        run_id: &str,
        repos: Vec<PrRepo>,
        note: String,
    ) -> Result<PrText> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let project = self.project(&run)?;
        let tasks = self.db().read(|c| repo::task_list(c, run_id))?;
        let integration = self.ensure_integration(&run, &project).await?;
        let mut sections = Vec::new();
        for pr in &repos {
            let Some(config) = run.repos.iter().find(|repo| repo.path == pr.repo) else {
                continue;
            };
            let place = workspace::repo_dir(&integration, &pr.repo);
            let range = format!("{}..HEAD", config.base_commit);
            let commits = git::git(&place, &["log", "--no-merges", "--format=- %s", &range])
                .await
                .unwrap_or_default();
            let stat = git::git(
                &place,
                &[
                    "diff",
                    "--stat=120",
                    &format!("{}...HEAD", config.base_commit),
                ],
            )
            .await
            .unwrap_or_default();
            let remote = format!("refs/remotes/origin/{}", pr.base);
            let base_ref = if git::commit_of(&place, &remote).await.is_ok() {
                remote
            } else {
                pr.base.clone()
            };
            let style = git::git(
                &place,
                &["log", "-15", "--no-merges", "--format=- %s", &base_ref],
            )
            .await
            .unwrap_or_default();
            let template = PR_TEMPLATES
                .iter()
                .find_map(|rel| {
                    std::fs::read_to_string(place.join(rel))
                        .ok()
                        .map(|text| (rel, text))
                })
                .map(|(rel, text)| {
                    format!(
                        "PR template (`{rel}`):\n<template>\n{}\n</template>",
                        git::cut(text, 6000).0.trim()
                    )
                })
                .unwrap_or_else(|| "PR template: (none)".into());
            sections.push(format!(
                "## {repo}: `{branch}` → `{base}`\n{template}\n\nCommits of the run:\n{commits}\n\nFiles changed:\n{stat}\n\nRecent commit subjects of `{base}`:\n{style}",
                repo = if pr.repo == repo::ROOT_REPO {
                    "repository".to_string()
                } else {
                    format!("`{}/`", pr.repo)
                },
                branch = pr.branch,
                base = pr.base,
                commits = git::cut(commits, 4000).0,
                stat = git::cut(stat, 4000).0,
                style = if style.is_empty() { "(unknown)".into() } else { style },
            ));
        }
        if sections.is_empty() {
            return Err(Error::msg("Hãy chọn ít nhất một repo của lượt chạy"));
        }
        let checks = run
            .integration_checks
            .iter()
            .flatten()
            .map(|check| {
                format!(
                    "- {} `{}`",
                    if check.ok { "passed" } else { "FAILED" },
                    check.command
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let chosen: Vec<String> = repos.iter().map(|pr| pr.repo.clone()).collect();
        let (guides, _) = guides::block(
            &Self::guides(&project),
            Path::new(&project.path),
            &integration,
            Some(&chosen),
            Use::Lead,
        );
        let done: Vec<&Task> = tasks
            .iter()
            .filter(|task| task.status == task_status::MERGED)
            .collect();
        let prompt =
            prompts::describe_pr(&run, &done, &sections.join("\n\n"), &checks, &note, &guides);
        let control = Control {
            pause: AtomicBool::new(false),
            stop: watch::channel(false).0,
        };
        let parse = |value: Value| -> Result<PrText> {
            let text: PrText = serde_json::from_value(value).map_err(|error| {
                Error::msg(format!("answer does not match the schema: {error}"))
            })?;
            if text.title.trim().is_empty() || text.body.trim().is_empty() {
                return Err(Error::msg("title and body must not be empty"));
            }
            Ok(text)
        };
        let (text, _) = self
            .leader_json(
                &run.id,
                None,
                "describe",
                &format!("{} · viết PR", run.leader_cli),
                Some(integration.as_path()),
                prompt,
                &prompts::pr_text_schema(),
                &control,
                parse,
            )
            .await
            .map_err(|error| Error::msg(error.message()))?;
        Ok(PrText {
            title: text
                .title
                .trim()
                .lines()
                .next()
                .unwrap_or_default()
                .to_string(),
            body: text.body.trim().to_string(),
        })
    }

    /// Gate 2, instead of merging locally: pushes the integration branch of
    /// each chosen repo (pre-push hooks run, never skipped) and opens its
    /// pull request with gh, or updates the PR an earlier push opened. The
    /// run ends as "kept", its PRs remembered for follow-ups.
    pub async fn open_pull_requests(&self, run_id: &str, request: PrRequest) -> Result<Run> {
        self.require_idle(run_id)?;
        self.push_pull_requests(run_id, request).await
    }

    /// `open_pull_requests` for the user or for the driver itself.
    async fn push_pull_requests(&self, run_id: &str, request: PrRequest) -> Result<Run> {
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        let delivered = run.stage == stage::DONE
            && (run.status == run_status::KEPT || run.status == run_status::MERGED);
        if run.stage != stage::MERGE_GATE && !delivered {
            return Err(Error::msg("Lượt chạy không ở bước này nữa"));
        }
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

        let (id, log) = self.open_side_log(run_id, None, "pr", "push & tạo PR");
        // Kept alive: the tab's own stop button still cancels.
        let (_stop, stop) = watch::channel(false);
        let mut opened = run.pull_requests.clone();
        let mut outcome: Result<()> = Ok(());
        // Pushed from the leader's worktree: dependencies installed there,
        // the pre-push hook checks the very code that goes out, and a rebase
        // moves the integration branch along.
        let integration = match self.ensure_integration(&run, &project).await {
            Ok(dir) => dir,
            Err(error) => {
                self.close_side_log(run_id, &id, false);
                return Err(error);
            }
        };
        let mut failed_repo = String::new();
        for pr in &request.repos {
            failed_repo = pr.repo.clone();
            let (branch, base) = (pr.branch.trim(), pr.base.trim());
            let label = workspace::label(&pr.repo);
            let root = workspace::repo_dir(Path::new(&project.path), &pr.repo);
            // Where origin stands: the base to sit on, the branch to lease.
            let _ = git::git(&root, &["fetch", "origin", base]).await;
            let _ = git::git(&root, &["fetch", "origin", branch]).await;
            let base_ref = format!("refs/remotes/origin/{base}");
            let remote_head = git::commit_of(&root, &format!("refs/remotes/origin/{branch}"))
                .await
                .ok();
            let rebase = match request.rebase {
                Some(rebase) => rebase,
                None => git::needs_rebase(&root, &base_ref, &run.integration_branch).await,
            };
            let place = workspace::repo_dir(&integration, &pr.repo);
            if let Err(error) = self
                .ensure_setup(
                    run_id,
                    None,
                    &project,
                    &integration,
                    std::slice::from_ref(&pr.repo),
                    "chuẩn bị · push",
                    stop.clone(),
                )
                .await
            {
                let message = error.message();
                log.result(false, &message);
                outcome = Err(Error::msg(message));
                break;
            }
            let before = git::commit_of(&place, "HEAD").await.unwrap_or_default();
            if rebase {
                log.heading(&format!("{label}git rebase origin/{base}"));
                let rebased = match git::rebase(&place, &base_ref).await {
                    Ok(()) => Ok(()),
                    Err(MergeError::Conflict(files)) => Err(format!(
                        "Rebase lên origin/{base} bị xung đột, chưa push gì: {}. Hãy dùng \
                         \"Push bằng AI\" hoặc rebase tay rồi Gắn PR / push lại",
                        files.join(", ")
                    )),
                    Err(MergeError::Failed(message)) => Err(format!("Rebase lỗi: {message}")),
                };
                match rebased {
                    Ok(()) => log.result(true, &format!("Đã rebase lên origin/{base}")),
                    Err(message) => {
                        log.result(false, &message);
                        outcome = Err(Error::msg(format!("{label}{message}")));
                        break;
                    }
                }
            }
            // The pre-push hook runs first, on its own: git opens the
            // connection to origin before running it, and a hook that takes
            // many minutes (the project's tests and builds) outlives that
            // connection, so git died silently once the hook had passed.
            match self
                .run_pre_push(
                    run_id,
                    &place,
                    branch,
                    remote_head.as_deref(),
                    &log,
                    stop.clone(),
                )
                .await
            {
                Ok(()) => {}
                Err(message) => {
                    if rebase && !before.is_empty() {
                        let _ = git::reset_worktree(&place, &before).await;
                    }
                    outcome = Err(Error::msg(format!("{label}{message}")));
                    break;
                }
            }
            let refspec = format!("HEAD:refs/heads/{branch}");
            // The hook just passed for this very commit.
            let mut args: Vec<String> = vec!["push".into(), "--no-verify".into()];
            if rebase {
                // Rewrites the PR branch, but only the commit we saw there.
                args.push(format!(
                    "--force-with-lease=refs/heads/{branch}:{}",
                    remote_head.clone().unwrap_or_default()
                ));
            }
            args.push("origin".into());
            args.push(refspec);
            log.heading(&format!("{label}git {}", args.join(" ")));
            let pushed = process::run_program(
                Path::new("git"),
                &args,
                &place,
                PUSH_TIMEOUT,
                &log,
                stop.clone(),
            )
            .await;
            let failure = match pushed {
                Ok((true, _, _)) => {
                    log.result(true, "Đã push");
                    None
                }
                Ok((false, code, tail)) => {
                    log.result(false, "git push thất bại");
                    Some(match code {
                        Some(code) => format!(
                            "{label}git push thất bại (mã {code}): {}",
                            push_error(&tail)
                        ),
                        None => format!(
                            "{label}git push bị ngắt giữa chừng (git bị dừng bởi tín hiệu, \
                             thường do kết nối tới origin bị đóng): {}",
                            push_error(&tail)
                        ),
                    })
                }
                Err(error) => {
                    log.result(false, &error.to_string());
                    Some(format!("{label}git push: {error}"))
                }
            };
            if let Some(failure) = failure {
                // Nothing went out: the integration branch is as it was.
                if rebase && !before.is_empty() {
                    let _ = git::reset_worktree(&place, &before).await;
                }
                outcome = Err(Error::msg(failure));
                break;
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
        self.close_side_log(run_id, &id, outcome.is_ok());
        // What was opened is kept even when a later repo failed.
        let saved = opened.clone();
        if let Err(error) = outcome {
            // For the leader to look into ("Leader xử lý lỗi push").
            let failure = PushFailure {
                at: now_unix(),
                repo: failed_repo,
                error: error.to_string(),
                output: plain_tail(&log.snapshot_bytes(), PUSH_OUTPUT_BYTES),
                request: request.clone(),
                diagnosis: None,
                action: None,
                fix_tasks: Vec::new(),
                retry: false,
            };
            let _ = self.save_push_failure(run_id, Some(&failure));
            self.update_run(run_id, |run| run.pull_requests = saved)?;
            return Err(error);
        }
        let _ = self.save_push_failure(run_id, None);
        self.delivered(&run).await?;
        let note = opened
            .iter()
            .map(|pr| pr.url.clone())
            .filter(|url| !url.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let at_gate = run.stage == stage::MERGE_GATE;
        self.update_run(run_id, |run| {
            run.pull_requests = opened;
            run.status_note = Some(format!("PR: {note}"));
            // A finished run stays as it ended (merged or kept).
            if at_gate {
                run.stage = stage::DONE.into();
                run.status = run_status::KEPT.into();
                run.finished_at = Some(now_unix());
            }
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
    pub async fn delete(&self, run_id: &str) -> Result<()> {
        self.require_idle(run_id)?;
        let run = self.db().read(|c| repo::run_get(c, run_id))?;
        if run.stage != stage::DONE {
            return Err(Error::msg("Hãy bỏ lượt chạy trước khi xóa"));
        }
        // Worktrees a delivered run kept; the branches stay.
        let _ = self.cleanup_worktrees(&run, false).await;
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
                    &[
                        "merge",
                        "--no-ff",
                        "--no-commit",
                        "--no-verify",
                        &run.integration_branch,
                    ],
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
                let brought = self
                    .commit_workspace(&run, &base, &format!("cm({}): merge integration", task.key))
                    .await?;
                let approved = task
                    .last_review
                    .as_ref()
                    .is_some_and(|review| review.verdict == "approved");
                if approved && !brought {
                    // Approved and nothing new came in: the merge itself
                    // failed (a hook, a lock), not the work; merge again.
                    let integration = self.ensure_integration(&run, &project).await?;
                    self.merge_task(&run, &integration, &task)
                        .await
                        .map_err(Error::from)?;
                    return self.continue_after_task_action(&task.run_id);
                }
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
            &format!("cm({}): manual changes before review", task.key),
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

#[cfg(test)]
mod push_tests {
    #[test]
    fn push_error_shows_the_hook() {
        let tail = "Enumerating objects\n\npre-push: x is behind origin/develop. Rebase before pushing\n  git rebase origin/develop\nerror: failed to push some refs to 'origin'";
        assert_eq!(
            super::push_error(tail),
            "pre-push: x is behind origin/develop. Rebase before pushing"
        );
        assert_eq!(super::push_error("a\nb\nc\nd\ne"), "b · c · d · e");
    }
}
