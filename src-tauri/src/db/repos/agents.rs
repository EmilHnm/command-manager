//! SCR-09 Agent Studio: projects, CLI profiles, runs, tasks and the CLI
//! processes (invocations) behind them.

use crate::error::{Error, Result};
use rusqlite::{Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const CLIS: &[&str] = &["claude", "codex", "agy", "cursor"];

/// The repo at the project root itself.
pub const ROOT_REPO: &str = ".";

/// A project is a folder: one git repo, or a folder holding several (a
/// "cluster" of fe/, be/, docs/...).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub path: String,
    /// The repos found in the folder, with their settings.
    pub repos: Vec<RepoConfig>,
    /// Added to every agent prompt.
    pub notes: Option<String>,
    /// The limits of the last run started here: the next run's defaults.
    pub limits: Option<Limits>,
    pub created_at: String,
    pub last_opened_at: Option<String>,
}

impl Project {
    pub fn repo(&self, path: &str) -> Option<&RepoConfig> {
        self.repos.iter().find(|repo| repo.path == path)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RepoConfig {
    /// Relative to the project folder (`fe`, `apps/web`), or `.`.
    pub path: String,
    /// Merge target at gate 2; None = the branch checked out at run start.
    #[serde(default)]
    pub base_branch: Option<String>,
    /// Run once in a new task worktree of this repo, e.g. `pnpm install`.
    #[serde(default)]
    pub setup_cmd: Option<String>,
    /// Run in this repo after each worker turn and on the integration branch.
    #[serde(default)]
    pub check_cmds: Vec<String>,
    /// How long one setup or check command may run; None: 15 minutes.
    #[serde(default)]
    pub check_timeout_minutes: Option<u32>,
}

/// Default time a setup or check command may run.
pub const CHECK_TIMEOUT_MINUTES: u32 = 15;
/// The longest any timeout can be set to.
pub const MAX_TIMEOUT_MINUTES: u32 = 600;

impl RepoConfig {
    pub fn check_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(
            self.check_timeout_minutes.unwrap_or(CHECK_TIMEOUT_MINUTES) as u64 * 60,
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectSettings {
    #[serde(default)]
    pub repos: Vec<RepoConfig>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// A repo as a run uses it: where it started.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunRepo {
    pub path: String,
    pub base_branch: String,
    pub base_commit: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CliProfile {
    pub cli: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    /// "Tự duyệt mọi thứ": the CLI's own bypass flag. Off unless set here.
    #[serde(default)]
    pub bypass_permissions: bool,
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Model / effort per step; empty fields fall back to the ones above.
    #[serde(default)]
    pub steps: StepModels,
}

/// What a step needs: the leader designs and plans, workers write code,
/// the reviewer checks it. Cheaper models fit work and review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Lead,
    Work,
    Review,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StepModel {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StepModels {
    #[serde(default)]
    pub lead: StepModel,
    #[serde(default)]
    pub work: StepModel,
    #[serde(default)]
    pub review: StepModel,
}

impl StepModels {
    /// Until the user sets them: Claude writes and reviews with Sonnet, the
    /// lead keeps the account's default (usually Opus).
    pub fn defaults(cli: &str) -> Self {
        let sonnet = |effort: Option<&str>| StepModel {
            model: Some("sonnet".into()),
            effort: effort.map(str::to_string),
        };
        match cli {
            "claude" => Self {
                lead: StepModel::default(),
                work: sonnet(None),
                review: sonnet(Some("low")),
            },
            _ => Self::default(),
        }
    }
}

impl CliProfile {
    /// The profile with the step's model and effort in place.
    pub fn for_phase(&self, phase: Phase) -> CliProfile {
        let step = match phase {
            Phase::Lead => &self.steps.lead,
            Phase::Work => &self.steps.work,
            Phase::Review => &self.steps.review,
        };
        CliProfile {
            model: step.model.clone().or_else(|| self.model.clone()),
            effort: step.effort.clone().or_else(|| self.effort.clone()),
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RoleCli {
    pub role: String,
    pub cli: String,
    /// The role's model / effort for this run; None: the CLI profile's.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Limits {
    pub concurrency: u32,
    pub max_rounds: u32,
    pub max_invocations: u32,
    pub timeout_minutes: u32,
    /// Spending cap of the run, in the CLIs' list-price dollars. Runs saved
    /// before it existed get the default.
    #[serde(default = "default_max_cost")]
    pub max_cost_usd: f64,
    /// Pause when a usage-limit window of the account (Claude's 5 hours /
    /// 7 days) is this full, in percent; 0 turns it off.
    #[serde(default = "default_quota_pause")]
    pub quota_pause_percent: u32,
    /// After such a pause, go on by itself once the window resets.
    #[serde(default = "default_auto_resume")]
    pub auto_resume: bool,
}

fn default_max_cost() -> f64 {
    5.0
}

fn default_quota_pause() -> u32 {
    85
}

fn default_auto_resume() -> bool {
    true
}

impl Limits {
    pub fn validate(&self) -> Result<()> {
        if self.max_rounds == 0
            || self.max_invocations == 0
            || self.timeout_minutes == 0
            || self.max_cost_usd.is_nan()
            || self.max_cost_usd <= 0.0
        {
            return Err(Error::msg("Giới hạn phải lớn hơn 0"));
        }
        if self.timeout_minutes > MAX_TIMEOUT_MINUTES {
            return Err(Error::msg(format!(
                "Timeout tối đa {MAX_TIMEOUT_MINUTES} phút"
            )));
        }
        if self.quota_pause_percent > 100 {
            return Err(Error::msg("Ngưỡng hạn mức phải từ 0 đến 100%"));
        }
        Ok(())
    }

    /// The quota threshold as a fraction, if any.
    pub fn quota_limit(&self) -> Option<f64> {
        (self.quota_pause_percent > 0 && self.quota_pause_percent < 100)
            .then(|| self.quota_pause_percent as f64 / 100.0)
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            concurrency: 2,
            max_rounds: 3,
            max_invocations: 60,
            timeout_minutes: 20,
            max_cost_usd: default_max_cost(),
            quota_pause_percent: default_quota_pause(),
            auto_resume: default_auto_resume(),
        }
    }
}

/// Where a run is in its pipeline (SCR-09 §3).
pub mod stage {
    pub const DESIGN: &str = "design";
    pub const PLAN: &str = "plan";
    pub const PLAN_GATE: &str = "plan_gate";
    pub const IMPLEMENT: &str = "implement";
    pub const INTEGRATE: &str = "integrate";
    pub const MERGE_GATE: &str = "merge_gate";
    pub const DONE: &str = "done";
}

/// Whether the driver is working on the stage.
pub mod run_status {
    pub const RUNNING: &str = "running";
    /// At a gate, or stopped on something only a person can decide.
    pub const WAITING: &str = "waiting";
    pub const PAUSED: &str = "paused";
    pub const FAILED: &str = "failed";
    pub const MERGED: &str = "merged";
    /// Finished with the integration branch kept for the user.
    pub const KEPT: &str = "kept";
    pub const DISCARDED: &str = "discarded";
}

pub mod task_status {
    /// Waiting for its dependencies.
    pub const WAITING: &str = "waiting";
    pub const READY: &str = "ready";
    pub const RUNNING: &str = "running";
    pub const CHECKING: &str = "checking";
    pub const REVIEW: &str = "review";
    /// Review asked for changes; the worker gets them next.
    pub const FIXING: &str = "fixing";
    pub const MERGED: &str = "merged";
    pub const FAILED: &str = "failed";
    pub const CONFLICT: &str = "conflict";
    pub const NEEDS_HUMAN: &str = "needs_human";
    pub const SKIPPED: &str = "skipped";

    /// Statuses that let tasks depending on this one start.
    pub fn is_settled(status: &str) -> bool {
        status == MERGED || status == SKIPPED
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Run {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub request: String,
    pub leader_cli: String,
    /// The leader's model / effort for design and plan; None: the profile's.
    pub leader_model: Option<String>,
    pub leader_effort: Option<String>,
    /// Check commands the leader proposed, until gate 1 settles them.
    pub suggested_checks: Vec<CheckProposal>,
    /// Opened from gate 2, one per repo.
    pub pull_requests: Vec<PullRequest>,
    pub roles: Vec<RoleCli>,
    pub plan_gate: bool,
    pub merge_gate: bool,
    pub limits: Limits,
    pub stage: String,
    pub status: String,
    pub status_note: Option<String>,
    pub design_md: Option<String>,
    pub contract: Option<String>,
    pub leader_session: Option<String>,
    /// Gate 1 feedback the leader has not worked in yet.
    pub pending_feedback: Option<String>,
    /// The first repo's, for a one-line summary; see `repos`.
    pub base_branch: String,
    pub base_commit: String,
    /// Every repo of the project, where the run started in it.
    pub repos: Vec<RunRepo>,
    /// Same branch name in every repo.
    pub integration_branch: String,
    pub integration_checks: Option<Vec<CheckResult>>,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
    /// Requests made after the run was delivered (a human review's
    /// comments...), oldest first. Their tasks are batch 1, 2...
    pub followups: Vec<FollowUp>,
}

/// A pull request of the run's work in one repo.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PullRequest {
    pub repo: String,
    /// The branch pushed (the integration branch under the user's name).
    pub branch: String,
    /// What it merges into.
    pub base: String,
    pub url: String,
}

/// Automated checks proposed for a repo that has none.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CheckProposal {
    pub repo: String,
    #[serde(default)]
    pub setup: String,
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FollowUp {
    pub request: String,
    pub created_at: String,
}

impl Run {
    /// The batch of tasks being worked on: 0 for the first request.
    pub fn batch(&self) -> u32 {
        self.followups.len() as u32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckResult {
    pub command: String,
    pub ok: bool,
    pub exit_code: Option<i32>,
    /// The end of the output.
    pub output: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewComment {
    pub file: String,
    pub line: Option<u32>,
    pub issue: String,
    /// `must` or `should`.
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Review {
    /// `approved` or `changes_requested`.
    pub verdict: String,
    pub summary: String,
    pub comments: Vec<ReviewComment>,
    /// Round of the work that was reviewed.
    #[serde(default)]
    pub round: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: String,
    pub run_id: String,
    pub key: String,
    pub title: String,
    pub role: String,
    pub cli: String,
    pub description: String,
    pub acceptance: Vec<String>,
    pub scope: Vec<String>,
    pub depends_on: Vec<String>,
    /// Repos the task works in (setup runs there; scope is relative to the
    /// project folder).
    pub repos: Vec<String>,
    pub status: String,
    pub status_note: Option<String>,
    pub round: u32,
    pub worker_session: Option<String>,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub checks: Option<Vec<CheckResult>>,
    pub last_review: Option<Review>,
    pub added: u64,
    pub removed: u64,
    pub out_of_scope: Vec<String>,
    pub sort_order: i64,
    /// 0: planned for the run's request; n: for its n-th follow-up.
    pub batch: u32,
    /// The worker's notes for the tasks after it.
    pub handoff: Option<String>,
    /// Picked when the task was given to another CLI; None: the role's.
    pub model: Option<String>,
    pub effort: Option<String>,
    /// This task's agent timeout; None: the run's.
    pub timeout_minutes: Option<u32>,
}

/// A task as the plan gate (MOD-22) edits it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskDraft {
    pub key: String,
    pub title: String,
    pub role: String,
    pub cli: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub repos: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Invocation {
    pub id: String,
    pub run_id: String,
    pub task_id: Option<String>,
    pub cli: String,
    pub step: String,
    pub session_id: Option<String>,
    /// The read-only terminal tab showing this process.
    pub terminal_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub ok: Option<bool>,
    pub error: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
    /// Cut short: its session can be continued.
    pub resumable: bool,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Usage {
    pub invocations: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
}

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("serializable")
}

fn parse<T: DeserializeOwned>(raw: &str) -> rusqlite::Result<T> {
    serde_json::from_str(raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn parse_opt<T: DeserializeOwned>(raw: Option<String>) -> rusqlite::Result<Option<T>> {
    raw.map(|raw| parse(&raw)).transpose()
}

// ---- projects -------------------------------------------------------------

const PROJECT_COLUMNS: &str =
    "id, path, base_branch, setup_cmd, check_cmds, notes, created_at, last_opened_at, repos, limits";

fn project_row(row: &Row) -> rusqlite::Result<Project> {
    let mut repos: Vec<RepoConfig> = parse(&row.get::<_, String>(8)?)?;
    if repos.is_empty() {
        // Saved before projects could hold several repos: the root repo
        // with the project's own settings.
        repos.push(RepoConfig {
            path: ROOT_REPO.into(),
            base_branch: row.get(2)?,
            setup_cmd: row.get(3)?,
            check_cmds: parse(&row.get::<_, String>(4)?)?,
            check_timeout_minutes: None,
        });
    }
    Ok(Project {
        id: row.get(0)?,
        path: row.get(1)?,
        repos,
        notes: row.get(5)?,
        limits: parse_opt(row.get(9)?)?,
        created_at: row.get(6)?,
        last_opened_at: row.get(7)?,
    })
}

pub fn project_get(conn: &Connection, id: &str) -> Result<Project> {
    conn.query_row(
        &format!("SELECT {PROJECT_COLUMNS} FROM agent_project WHERE id = ?1"),
        [id],
        project_row,
    )
    .optional()?
    .ok_or_else(|| Error::msg("Không tìm thấy dự án"))
}

pub fn project_by_path(conn: &Connection, path: &str) -> Result<Option<Project>> {
    Ok(conn
        .query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM agent_project WHERE path = ?1"),
            [path],
            project_row,
        )
        .optional()?)
}

pub fn project_list(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PROJECT_COLUMNS} FROM agent_project
         ORDER BY COALESCE(last_opened_at, created_at) DESC"
    ))?;
    let rows = stmt.query_map([], project_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// The project for `path`, created on first open. `found` are the repos in
/// the folder now: settings of repos still there are kept, new ones added,
/// vanished ones dropped.
pub fn project_open(
    conn: &Connection,
    id: &str,
    path: &str,
    found: &[String],
    now: &str,
) -> Result<Project> {
    let existing = project_by_path(conn, path)?;
    let repos: Vec<RepoConfig> = found
        .iter()
        .map(|repo| {
            existing
                .as_ref()
                .and_then(|project| project.repo(repo).cloned())
                .unwrap_or_else(|| RepoConfig {
                    path: repo.clone(),
                    ..Default::default()
                })
        })
        .collect();
    conn.execute(
        "INSERT INTO agent_project (id, path, created_at, last_opened_at, repos)
         VALUES (?1, ?2, ?3, ?3, ?4)
         ON CONFLICT(path) DO UPDATE SET last_opened_at = excluded.last_opened_at,
           repos = excluded.repos",
        [id, path, now, &json(&repos)],
    )?;
    project_by_path(conn, path)?.ok_or_else(|| Error::msg("Không mở được dự án"))
}

fn trim(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn normalize_settings(settings: ProjectSettings) -> Result<ProjectSettings> {
    let mut repos = Vec::new();
    for repo in settings.repos {
        let check_cmds: Vec<String> = repo
            .check_cmds
            .into_iter()
            .map(|cmd| cmd.trim().to_string())
            .filter(|cmd| !cmd.is_empty())
            .collect();
        if check_cmds.len() > 10 {
            return Err(Error::msg(format!(
                "{}: tối đa 10 lệnh kiểm tra",
                repo.path
            )));
        }
        let base_branch = trim(repo.base_branch);
        if base_branch
            .as_deref()
            .is_some_and(|branch| branch.starts_with('-') || branch.contains(char::is_whitespace))
        {
            return Err(Error::msg(format!(
                "{}: tên nhánh gốc không hợp lệ",
                repo.path
            )));
        }
        if repo
            .check_timeout_minutes
            .is_some_and(|minutes| minutes == 0 || minutes > MAX_TIMEOUT_MINUTES)
        {
            return Err(Error::msg(format!(
                "{}: timeout lệnh kiểm tra phải từ 1 đến {MAX_TIMEOUT_MINUTES} phút",
                repo.path
            )));
        }
        repos.push(RepoConfig {
            path: repo.path,
            base_branch,
            setup_cmd: trim(repo.setup_cmd),
            check_cmds,
            check_timeout_minutes: repo.check_timeout_minutes,
        });
    }
    Ok(ProjectSettings {
        repos,
        notes: trim(settings.notes),
    })
}

/// Saves the notes and the settings of the repos the project has (a repo
/// unknown to it is ignored).
pub fn project_save(conn: &Connection, id: &str, settings: &ProjectSettings) -> Result<()> {
    let mut project = project_get(conn, id)?;
    for repo in project.repos.iter_mut() {
        if let Some(edited) = settings.repos.iter().find(|r| r.path == repo.path) {
            *repo = edited.clone();
        }
    }
    conn.execute(
        "UPDATE agent_project SET repos = ?2, notes = ?3 WHERE id = ?1",
        rusqlite::params![id, json(&project.repos), settings.notes],
    )?;
    Ok(())
}

/// Remembers a run's limits as the project's defaults.
pub fn project_save_limits(conn: &Connection, id: &str, limits: &Limits) -> Result<()> {
    conn.execute(
        "UPDATE agent_project SET limits = ?2 WHERE id = ?1",
        rusqlite::params![id, json(limits)],
    )?;
    Ok(())
}

// ---- CLI profiles ---------------------------------------------------------

pub fn profile_get(conn: &Connection, cli: &str) -> Result<CliProfile> {
    Ok(conn
        .query_row(
            "SELECT cli, model, effort, bypass_permissions, extra_args, step_models
             FROM agent_cli_profile WHERE cli = ?1",
            [cli],
            |row| {
                Ok(CliProfile {
                    cli: row.get(0)?,
                    model: row.get(1)?,
                    effort: row.get(2)?,
                    bypass_permissions: row.get::<_, i64>(3)? != 0,
                    extra_args: parse(&row.get::<_, String>(4)?)?,
                    steps: match row.get::<_, Option<String>>(5)? {
                        Some(text) => parse(&text)?,
                        None => StepModels::defaults(cli),
                    },
                })
            },
        )
        .optional()?
        .unwrap_or_else(|| CliProfile {
            cli: cli.to_string(),
            steps: StepModels::defaults(cli),
            ..Default::default()
        }))
}

pub fn profile_save(conn: &Connection, profile: &CliProfile) -> Result<()> {
    if !CLIS.contains(&profile.cli.as_str()) {
        return Err(Error::msg(format!("CLI không hỗ trợ: {}", profile.cli)));
    }
    let trim = |value: &Option<String>| {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    let step = |step: &StepModel| StepModel {
        model: trim(&step.model),
        effort: trim(&step.effort),
    };
    let steps = StepModels {
        lead: step(&profile.steps.lead),
        work: step(&profile.steps.work),
        review: step(&profile.steps.review),
    };
    conn.execute(
        "INSERT INTO agent_cli_profile (cli, model, effort, bypass_permissions, extra_args, step_models)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(cli) DO UPDATE SET model = excluded.model, effort = excluded.effort,
           bypass_permissions = excluded.bypass_permissions, extra_args = excluded.extra_args,
           step_models = excluded.step_models",
        rusqlite::params![
            profile.cli,
            trim(&profile.model),
            trim(&profile.effort),
            profile.bypass_permissions as i64,
            json(&profile.extra_args),
            json(&steps)
        ],
    )?;
    Ok(())
}

// ---- runs -----------------------------------------------------------------

const RUN_COLUMNS: &str = "id, project_id, title, request, leader_cli, roles, plan_gate, \
     merge_gate, limits, stage, status, status_note, design_md, contract, leader_session, \
     pending_feedback, base_branch, base_commit, integration_branch, integration_checks, \
     created_at, updated_at, finished_at, repos, followups, leader_model, leader_effort, \
     suggested_checks, pull_requests";

fn run_row(row: &Row) -> rusqlite::Result<Run> {
    let mut repos: Vec<RunRepo> = parse(&row.get::<_, String>(23)?)?;
    if repos.is_empty() {
        // Runs from before multi-repo projects: the root repo.
        repos.push(RunRepo {
            path: ROOT_REPO.into(),
            base_branch: row.get(16)?,
            base_commit: row.get(17)?,
        });
    }
    Ok(Run {
        repos,
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        request: row.get(3)?,
        leader_cli: row.get(4)?,
        roles: parse(&row.get::<_, String>(5)?)?,
        plan_gate: row.get::<_, i64>(6)? != 0,
        merge_gate: row.get::<_, i64>(7)? != 0,
        limits: parse(&row.get::<_, String>(8)?)?,
        stage: row.get(9)?,
        status: row.get(10)?,
        status_note: row.get(11)?,
        design_md: row.get(12)?,
        contract: row.get(13)?,
        leader_session: row.get(14)?,
        pending_feedback: row.get(15)?,
        base_branch: row.get(16)?,
        base_commit: row.get(17)?,
        integration_branch: row.get(18)?,
        integration_checks: parse_opt(row.get(19)?)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
        finished_at: row.get(22)?,
        followups: parse(&row.get::<_, String>(24)?)?,
        leader_model: row.get(25)?,
        leader_effort: row.get(26)?,
        suggested_checks: parse(&row.get::<_, String>(27)?)?,
        pull_requests: parse(&row.get::<_, String>(28)?)?,
    })
}

pub fn run_insert(conn: &Connection, run: &Run) -> Result<()> {
    conn.execute(
        &format!(
            "INSERT INTO agent_run ({RUN_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
             ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25,
             ?26, ?27, ?28, ?29)"
        ),
        rusqlite::params![
            run.id,
            run.project_id,
            run.title,
            run.request,
            run.leader_cli,
            json(&run.roles),
            run.plan_gate as i64,
            run.merge_gate as i64,
            json(&run.limits),
            run.stage,
            run.status,
            run.status_note,
            run.design_md,
            run.contract,
            run.leader_session,
            run.pending_feedback,
            run.base_branch,
            run.base_commit,
            run.integration_branch,
            run.integration_checks.as_ref().map(json),
            run.created_at,
            run.updated_at,
            run.finished_at,
            json(&run.repos),
            json(&run.followups),
            run.leader_model,
            run.leader_effort,
            json(&run.suggested_checks),
            json(&run.pull_requests),
        ],
    )?;
    Ok(())
}

/// Writes every field the pipeline changes.
pub fn run_save(conn: &Connection, run: &Run) -> Result<()> {
    conn.execute(
        "UPDATE agent_run SET title = ?2, roles = ?3, limits = ?4, stage = ?5, status = ?6,
           status_note = ?7, design_md = ?8, contract = ?9, leader_session = ?10,
           integration_checks = ?11, updated_at = ?12, finished_at = ?13, pending_feedback = ?14,
           repos = ?15, followups = ?16, leader_model = ?17, leader_effort = ?18,
           base_branch = ?19, base_commit = ?20, suggested_checks = ?21,
           pull_requests = ?22
         WHERE id = ?1",
        rusqlite::params![
            run.id,
            run.title,
            json(&run.roles),
            json(&run.limits),
            run.stage,
            run.status,
            run.status_note,
            run.design_md,
            run.contract,
            run.leader_session,
            run.integration_checks.as_ref().map(json),
            run.updated_at,
            run.finished_at,
            run.pending_feedback,
            json(&run.repos),
            json(&run.followups),
            run.leader_model,
            run.leader_effort,
            run.base_branch,
            run.base_commit,
            json(&run.suggested_checks),
            json(&run.pull_requests),
        ],
    )?;
    Ok(())
}

pub fn run_get(conn: &Connection, id: &str) -> Result<Run> {
    conn.query_row(
        &format!("SELECT {RUN_COLUMNS} FROM agent_run WHERE id = ?1"),
        [id],
        run_row,
    )
    .optional()?
    .ok_or_else(|| Error::msg("Không tìm thấy lượt chạy"))
}

pub fn run_list(conn: &Connection, project_id: &str) -> Result<Vec<Run>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {RUN_COLUMNS} FROM agent_run WHERE project_id = ?1 ORDER BY created_at DESC"
    ))?;
    let rows = stmt.query_map([project_id], run_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn run_delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM agent_run WHERE id = ?1", [id])?;
    Ok(())
}

/// Runs left `running` by an app that quit: nothing drives them any more.
pub fn pause_orphaned_runs(conn: &Connection, now: &str) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE agent_run SET status = 'paused', status_note = 'Ứng dụng đã đóng khi đang chạy',
           updated_at = ?1
         WHERE status = 'running'",
        [now],
    )?)
}

// ---- tasks ----------------------------------------------------------------

const TASK_COLUMNS: &str = "id, run_id, key, title, role, cli, description, acceptance, scope, \
     depends_on, status, status_note, round, worker_session, branch, worktree, checks, \
     last_review, added, removed, out_of_scope, sort_order, repos, batch, handoff, model, effort, \
     timeout_minutes";

fn task_row(row: &Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        run_id: row.get(1)?,
        key: row.get(2)?,
        title: row.get(3)?,
        role: row.get(4)?,
        cli: row.get(5)?,
        description: row.get(6)?,
        acceptance: parse(&row.get::<_, String>(7)?)?,
        scope: parse(&row.get::<_, String>(8)?)?,
        depends_on: parse(&row.get::<_, String>(9)?)?,
        status: row.get(10)?,
        status_note: row.get(11)?,
        round: row.get::<_, i64>(12)?.max(0) as u32,
        worker_session: row.get(13)?,
        branch: row.get(14)?,
        worktree: row.get(15)?,
        checks: parse_opt(row.get(16)?)?,
        last_review: parse_opt(row.get(17)?)?,
        added: row.get::<_, i64>(18)?.max(0) as u64,
        removed: row.get::<_, i64>(19)?.max(0) as u64,
        out_of_scope: parse(&row.get::<_, String>(20)?)?,
        sort_order: row.get(21)?,
        repos: {
            let repos: Vec<String> = parse(&row.get::<_, String>(22)?)?;
            if repos.is_empty() {
                vec![ROOT_REPO.to_string()]
            } else {
                repos
            }
        },
        batch: row.get::<_, i64>(23)?.max(0) as u32,
        handoff: row.get(24)?,
        model: row.get(25)?,
        effort: row.get(26)?,
        timeout_minutes: row.get::<_, Option<i64>>(27)?.map(|m| m.max(1) as u32),
    })
}

pub fn task_list(conn: &Connection, run_id: &str) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {TASK_COLUMNS} FROM agent_task WHERE run_id = ?1 ORDER BY sort_order"
    ))?;
    let rows = stmt.query_map([run_id], task_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn task_get(conn: &Connection, id: &str) -> Result<Task> {
    conn.query_row(
        &format!("SELECT {TASK_COLUMNS} FROM agent_task WHERE id = ?1"),
        [id],
        task_row,
    )
    .optional()?
    .ok_or_else(|| Error::msg("Không tìm thấy task"))
}

pub fn task_insert(conn: &Connection, task: &Task) -> Result<()> {
    conn.execute(
        &format!(
            "INSERT INTO agent_task ({TASK_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
             ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24,
             ?25, ?26, ?27, ?28)"
        ),
        rusqlite::params![
            task.id,
            task.run_id,
            task.key,
            task.title,
            task.role,
            task.cli,
            task.description,
            json(&task.acceptance),
            json(&task.scope),
            json(&task.depends_on),
            task.status,
            task.status_note,
            task.round as i64,
            task.worker_session,
            task.branch,
            task.worktree,
            task.checks.as_ref().map(json),
            task.last_review.as_ref().map(json),
            task.added as i64,
            task.removed as i64,
            json(&task.out_of_scope),
            task.sort_order,
            json(&task.repos),
            task.batch as i64,
            task.handoff,
            task.model,
            task.effort,
            task.timeout_minutes,
        ],
    )?;
    Ok(())
}

pub fn task_save(conn: &Connection, task: &Task) -> Result<()> {
    conn.execute(
        "UPDATE agent_task SET title = ?2, role = ?3, cli = ?4, description = ?5,
           acceptance = ?6, scope = ?7, depends_on = ?8, status = ?9, status_note = ?10,
           round = ?11, worker_session = ?12, branch = ?13, worktree = ?14, checks = ?15,
           last_review = ?16, added = ?17, removed = ?18, out_of_scope = ?19, sort_order = ?20,
           repos = ?21, handoff = ?22, model = ?23, effort = ?24,
           timeout_minutes = ?25
         WHERE id = ?1",
        rusqlite::params![
            task.id,
            task.title,
            task.role,
            task.cli,
            task.description,
            json(&task.acceptance),
            json(&task.scope),
            json(&task.depends_on),
            task.status,
            task.status_note,
            task.round as i64,
            task.worker_session,
            task.branch,
            task.worktree,
            task.checks.as_ref().map(json),
            task.last_review.as_ref().map(json),
            task.added as i64,
            task.removed as i64,
            json(&task.out_of_scope),
            task.sort_order,
            json(&task.repos),
            task.handoff,
            task.model,
            task.effort,
            task.timeout_minutes,
        ],
    )?;
    Ok(())
}

/// The tasks of one batch, to plan them again.
pub fn task_delete_batch(conn: &Connection, run_id: &str, batch: u32) -> Result<()> {
    conn.execute(
        "DELETE FROM agent_task WHERE run_id = ?1 AND batch = ?2",
        rusqlite::params![run_id, batch as i64],
    )?;
    Ok(())
}

/// Checks a plan: keys unique and well formed, dependencies known and
/// acyclic, every task has a CLI. Returns the drafts in a runnable order
/// (dependencies first, otherwise as given).
pub fn check_plan(drafts: &[TaskDraft], repos: &[String]) -> Result<Vec<TaskDraft>> {
    if drafts.is_empty() {
        return Err(Error::msg("Kế hoạch chưa có task nào"));
    }
    if drafts.len() > 50 {
        return Err(Error::msg("Kế hoạch có quá nhiều task (tối đa 50)"));
    }
    for (index, draft) in drafts.iter().enumerate() {
        let key = draft.key.trim();
        if key.is_empty()
            || key.len() > 40
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(Error::msg(format!(
                "Key task không hợp lệ: \"{}\" (chỉ chữ, số, - và _)",
                draft.key
            )));
        }
        if drafts[..index].iter().any(|other| other.key == draft.key) {
            return Err(Error::msg(format!("Key task bị trùng: {}", draft.key)));
        }
        if draft.title.trim().is_empty() {
            return Err(Error::msg(format!("Task {} chưa có tiêu đề", draft.key)));
        }
        if !CLIS.contains(&draft.cli.as_str()) {
            return Err(Error::msg(format!(
                "Task {} dùng CLI không hỗ trợ: {}",
                draft.key, draft.cli
            )));
        }
        if draft.repos.is_empty() {
            return Err(Error::msg(format!("Task {} chưa chọn repo", draft.key)));
        }
        if let Some(unknown) = draft.repos.iter().find(|repo| !repos.contains(repo)) {
            return Err(Error::msg(format!(
                "Task {} dùng repo không có trong dự án: {unknown}",
                draft.key
            )));
        }
        for dep in &draft.depends_on {
            if dep == &draft.key {
                return Err(Error::msg(format!("Task {} phụ thuộc chính nó", draft.key)));
            }
            if !drafts.iter().any(|other| &other.key == dep) {
                return Err(Error::msg(format!(
                    "Task {} phụ thuộc task không tồn tại: {dep}",
                    draft.key
                )));
            }
        }
    }
    // Kahn's algorithm, picking the earliest ready task each time so the
    // order stays close to what the leader wrote.
    let mut placed: Vec<TaskDraft> = Vec::with_capacity(drafts.len());
    let mut left: Vec<&TaskDraft> = drafts.iter().collect();
    while !left.is_empty() {
        let Some(index) = left.iter().position(|draft| {
            draft
                .depends_on
                .iter()
                .all(|dep| placed.iter().any(|done| &done.key == dep))
        }) else {
            let keys: Vec<&str> = left.iter().map(|draft| draft.key.as_str()).collect();
            return Err(Error::msg(format!(
                "Phụ thuộc giữa các task bị vòng lặp: {}",
                keys.join(", ")
            )));
        };
        placed.push(left.remove(index).clone());
    }
    Ok(placed)
}

// ---- invocations ----------------------------------------------------------

const INVOCATION_COLUMNS: &str = "id, run_id, task_id, cli, step, session_id, terminal_id, \
     started_at, finished_at, ok, error, input_tokens, output_tokens, cost_usd, resumable";

fn invocation_row(row: &Row) -> rusqlite::Result<Invocation> {
    Ok(Invocation {
        id: row.get(0)?,
        run_id: row.get(1)?,
        task_id: row.get(2)?,
        cli: row.get(3)?,
        step: row.get(4)?,
        session_id: row.get(5)?,
        terminal_id: row.get(6)?,
        started_at: row.get(7)?,
        finished_at: row.get(8)?,
        ok: row.get::<_, Option<i64>>(9)?.map(|ok| ok != 0),
        error: row.get(10)?,
        input_tokens: row.get::<_, i64>(11)?.max(0) as u64,
        output_tokens: row.get::<_, i64>(12)?.max(0) as u64,
        cost_usd: row.get(13)?,
        resumable: row.get::<_, i64>(14)? != 0,
    })
}

pub fn invocation_insert(conn: &Connection, inv: &Invocation) -> Result<()> {
    conn.execute(
        &format!(
            "INSERT INTO agent_invocation ({INVOCATION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"
        ),
        rusqlite::params![
            inv.id,
            inv.run_id,
            inv.task_id,
            inv.cli,
            inv.step,
            inv.session_id,
            inv.terminal_id,
            inv.started_at,
            inv.finished_at,
            inv.ok.map(|ok| ok as i64),
            inv.error,
            inv.input_tokens as i64,
            inv.output_tokens as i64,
            inv.cost_usd,
            inv.resumable as i64,
        ],
    )?;
    Ok(())
}

pub fn invocation_save(conn: &Connection, inv: &Invocation) -> Result<()> {
    conn.execute(
        "UPDATE agent_invocation SET session_id = ?2, finished_at = ?3, ok = ?4, error = ?5,
           input_tokens = ?6, output_tokens = ?7, cost_usd = ?8, resumable = ?9
         WHERE id = ?1",
        rusqlite::params![
            inv.id,
            inv.session_id,
            inv.finished_at,
            inv.ok.map(|ok| ok as i64),
            inv.error,
            inv.input_tokens as i64,
            inv.output_tokens as i64,
            inv.cost_usd,
            inv.resumable as i64,
        ],
    )?;
    Ok(())
}

pub fn invocation_list(conn: &Connection, run_id: &str) -> Result<Vec<Invocation>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {INVOCATION_COLUMNS} FROM agent_invocation WHERE run_id = ?1 ORDER BY started_at, rowid"
    ))?;
    let rows = stmt.query_map([run_id], invocation_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn invocation_get(conn: &Connection, id: &str) -> Result<Option<Invocation>> {
    Ok(conn
        .query_row(
            &format!("SELECT {INVOCATION_COLUMNS} FROM agent_invocation WHERE id = ?1"),
            [id],
            invocation_row,
        )
        .optional()?)
}

pub fn usage(conn: &Connection, run_id: &str) -> Result<Usage> {
    Ok(conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0),
           COALESCE(SUM(cost_usd), 0)
         FROM agent_invocation WHERE run_id = ?1",
        [run_id],
        |row| {
            Ok(Usage {
                invocations: row.get::<_, i64>(0)?.max(0) as u64,
                input_tokens: row.get::<_, i64>(1)?.max(0) as u64,
                output_tokens: row.get::<_, i64>(2)?.max(0) as u64,
                cost_usd: row.get(3)?,
            })
        },
    )?)
}

/// Processes that never reported back (the app quit mid-run).
pub fn close_orphaned_invocations(conn: &Connection, now: &str) -> Result<()> {
    conn.execute(
        "UPDATE agent_invocation SET finished_at = ?1, ok = 0, resumable = 1,
           error = 'Ứng dụng đã đóng khi đang chạy'
         WHERE finished_at IS NULL",
        [now],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(key: &str, deps: &[&str]) -> TaskDraft {
        TaskDraft {
            key: key.into(),
            title: key.into(),
            role: "backend".into(),
            cli: "codex".into(),
            description: String::new(),
            acceptance: Vec::new(),
            scope: Vec::new(),
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            repos: vec!["be".into()],
        }
    }

    #[test]
    fn plans_are_ordered_by_dependency_and_loops_are_refused() {
        let plan = vec![
            draft("ui", &["api"]),
            draft("api", &["schema"]),
            draft("schema", &[]),
        ];
        let repos = vec!["be".to_string(), "fe".to_string()];
        let check_plan = |drafts: &[TaskDraft]| super::check_plan(drafts, &repos);
        let keys: Vec<String> = check_plan(&plan)
            .expect("valid")
            .into_iter()
            .map(|d| d.key)
            .collect();
        assert_eq!(keys, vec!["schema", "api", "ui"]);

        assert!(check_plan(&[draft("a", &["b"]), draft("b", &["a"])]).is_err());
        assert!(check_plan(&[draft("a", &["missing"])]).is_err());
        assert!(check_plan(&[draft("a", &[]), draft("a", &[])]).is_err());
        assert!(check_plan(&[draft("bad key", &[])]).is_err());
        let mut unknown = draft("a", &[]);
        unknown.cli = "gpt".into();
        assert!(check_plan(&[unknown]).is_err());
        let mut elsewhere = draft("a", &[]);
        elsewhere.repos = vec!["docs".into()];
        assert!(check_plan(&[elsewhere]).is_err());
        let mut nowhere = draft("a", &[]);
        nowhere.repos.clear();
        assert!(check_plan(&[nowhere]).is_err());
    }

    #[test]
    fn rows_round_trip_with_their_json_columns() {
        let conn = Connection::open_in_memory().expect("db");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        crate::db::pool::migrate(&conn).expect("migrate");
        let found = vec!["be".to_string(), "fe".to_string()];
        let project = project_open(&conn, "p1", "/repo", &found, "1").expect("open");
        assert_eq!(project.repos.len(), 2);
        project_save(
            &conn,
            "p1",
            &ProjectSettings {
                repos: vec![RepoConfig {
                    path: "be".into(),
                    check_cmds: vec!["cargo test".into()],
                    ..Default::default()
                }],
                notes: Some("n".into()),
            },
        )
        .expect("save");
        // Opening again keeps the id and the settings of repos still there;
        // fe is gone, docs is new.
        let found = vec!["be".to_string(), "docs".to_string()];
        let project = project_open(&conn, "p2", "/repo", &found, "2").expect("open");
        assert_eq!(project.id, "p1");
        assert_eq!(project.repos[0].check_cmds, vec!["cargo test"]);
        assert_eq!(project.repos[1].path, "docs");
        assert_eq!(project.notes.as_deref(), Some("n"));

        let run = Run {
            id: "r1".into(),
            project_id: "p1".into(),
            title: "Giỏ hàng".into(),
            request: "Thêm giỏ hàng".into(),
            leader_cli: "claude".into(),
            roles: vec![RoleCli {
                role: "backend".into(),
                cli: "codex".into(),
                model: Some("gpt".into()),
                effort: None,
            }],
            plan_gate: true,
            merge_gate: true,
            limits: Limits::default(),
            stage: stage::DESIGN.into(),
            status: run_status::RUNNING.into(),
            status_note: None,
            design_md: None,
            contract: None,
            leader_session: None,
            pending_feedback: None,
            base_branch: "main".into(),
            base_commit: "abc".into(),
            repos: vec![RunRepo {
                path: "be".into(),
                base_branch: "main".into(),
                base_commit: "abc".into(),
            }],
            integration_branch: "cm/r1/integration".into(),
            integration_checks: None,
            created_at: "1".into(),
            updated_at: "1".into(),
            finished_at: None,
            followups: Vec::new(),
            leader_model: Some("opus".into()),
            leader_effort: Some("high".into()),
            suggested_checks: Vec::new(),
            pull_requests: Vec::new(),
        };
        let mut run = run;
        run.repos.push(RunRepo {
            path: "wt".into(),
            base_branch: "main".into(),
            base_commit: "abc".into(),
        });
        run_insert(&conn, &run).expect("insert");
        assert_eq!(run_get(&conn, "r1").expect("get"), run);
        // Every field is saved, the repos too.
        run.repos.pop();
        run.followups.push(FollowUp {
            request: "Sửa theo review".into(),
            created_at: "2".into(),
        });
        run.leader_model = None;
        run.base_commit = "def".into();
        run.suggested_checks.push(CheckProposal {
            repo: "be".into(),
            setup: "pnpm install".into(),
            commands: vec!["pnpm test".into()],
        });
        run.pull_requests.push(PullRequest {
            repo: "be".into(),
            branch: "feat/cart".into(),
            base: "develop".into(),
            url: "https://github.com/o/r/pull/1".into(),
        });
        run_save(&conn, &run).expect("save run");
        assert_eq!(run_get(&conn, "r1").expect("get"), run);

        let mut task = Task {
            id: "t1".into(),
            run_id: "r1".into(),
            key: "api".into(),
            title: "API".into(),
            role: "backend".into(),
            cli: "codex".into(),
            description: String::new(),
            acceptance: vec!["works".into()],
            scope: vec!["src/**".into()],
            depends_on: Vec::new(),
            repos: vec!["be".into()],
            status: task_status::READY.into(),
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
            sort_order: 0,
            batch: 0,
            handoff: None,
            model: None,
            effort: None,
            timeout_minutes: None,
        };
        task_insert(&conn, &task).expect("task");
        task.last_review = Some(Review {
            verdict: "changes_requested".into(),
            summary: "x".into(),
            comments: vec![ReviewComment {
                file: "a.rs".into(),
                line: Some(3),
                issue: "y".into(),
                severity: "must".into(),
            }],
            round: 1,
        });
        task.model = Some("o3".into());
        task.timeout_minutes = Some(90);
        task.handoff = Some("note".into());
        task_save(&conn, &task).expect("save");
        assert_eq!(task_list(&conn, "r1").expect("list"), vec![task]);

        assert_eq!(pause_orphaned_runs(&conn, "2").expect("pause"), 1);
        assert_eq!(
            run_get(&conn, "r1").expect("get").status,
            run_status::PAUSED
        );

        run_delete(&conn, "r1").expect("delete");
        assert!(task_list(&conn, "r1").expect("list").is_empty());
    }
}
