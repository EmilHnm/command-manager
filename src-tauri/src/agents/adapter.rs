//! One adapter per AI CLI: the headless command line for a step, and a
//! reader for its stream-json output.
//!
//! Output formats (checked against the real CLIs):
//! - `claude -p --output-format stream-json --verbose` and `cursor-agent -p
//!   --output-format stream-json`: `system/init` (session_id), `assistant`
//!   messages with text / tool_use blocks, `user` tool results, and a final
//!   `result` (result text, usage, total_cost_usd, structured_output).
//! - `codex exec --json`: `thread.started` (thread_id = session),
//!   `item.started` / `item.completed` (agent_message, command_execution,
//!   file_change, ...), `turn.completed` (usage) or `turn.failed`.
//! - `agy -p --output-format stream-json`: `init` (conversation_id),
//!   `step_update` (tool steps, agent_response text deltas) and `result`
//!   (status, response, usage).

use crate::db::repos::agents::CliProfile;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cli {
    Claude,
    Codex,
    Agy,
    Cursor,
}

impl Cli {
    pub const ALL: [Cli; 4] = [Cli::Claude, Cli::Codex, Cli::Agy, Cli::Cursor];

    pub fn parse(name: &str) -> Result<Self> {
        Ok(match name {
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            "agy" => Self::Agy,
            "cursor" => Self::Cursor,
            _ => return Err(Error::msg(format!("CLI không hỗ trợ: {name}"))),
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Agy => "agy",
            Self::Cursor => "cursor",
        }
    }

    /// Executable names, most specific first.
    pub fn binaries(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["claude"],
            Self::Codex => &["codex"],
            Self::Agy => &["agy"],
            Self::Cursor => &["cursor-agent", "agent"],
        }
    }

    /// Whether the CLI can be held to a JSON schema by a flag. Without one
    /// the schema goes into the prompt.
    pub fn has_schema_flag(self) -> bool {
        !matches!(self, Self::Cursor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Design, plan, review: look but do not touch.
    ReadOnly,
    /// Implement and fix: write inside the worktree.
    Write,
    /// "Push bằng AI": write, run git against the repository (rebase,
    /// commit) and reach the network to push. Never resumed.
    Push,
}

pub struct Request<'a> {
    pub prompt: String,
    pub cwd: &'a Path,
    pub access: Access,
    /// Session to continue (worker fixes, the leader's plan step).
    pub resume: Option<&'a str>,
    /// JSON schema the final answer must follow.
    pub schema: Option<&'a Value>,
    /// The schema written to a file, for CLIs that take a path.
    pub schema_file: Option<&'a Path>,
    pub profile: &'a CliProfile,
    /// What the step may still spend, for CLIs that can stop themselves.
    pub budget_usd: Option<f64>,
    /// Folders outside `cwd` the agent reads (the run's context files).
    pub extra_dirs: &'a [PathBuf],
}

/// Commands a worker leaves to the orchestrator (it commits, merges and
/// opens pull requests).
const CLAUDE_BLOCKED_COMMANDS: &[&str] = &[
    "Bash(git push:*)",
    "Bash(git commit:*)",
    "Bash(git checkout:*)",
    "Bash(git switch:*)",
    "Bash(git rebase:*)",
    "Bash(git reset --hard:*)",
    "Bash(gh pr create:*)",
    "Bash(gh pr merge:*)",
];

/// Claude Code tools a headless step must not use: sub-agents and
/// background work multiply the cost of one step many times over (one
/// design step with three Explore agents burned ~24M tokens) and wake the
/// session up again after its answer.
const CLAUDE_BLOCKED_TOOLS: &[&str] = &[
    "Agent",
    "Task",
    "Workflow",
    "Monitor",
    "ScheduleWakeup",
    "CronCreate",
    "RemoteTrigger",
    "SendMessage",
    "PushNotification",
    "EnterWorktree",
];

/// Arguments after the program name. The prompt is always an argument of
/// its own (never parsed by a shell).
pub fn args(cli: Cli, request: &Request) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let mut push = |items: &[&str]| args.extend(items.iter().map(|item| item.to_string()));
    let profile = request.profile;
    let bypass = profile.bypass_permissions;
    let write = request.access != Access::ReadOnly;
    let pushing = request.access == Access::Push;
    let mut prompt = request.prompt.clone();

    match cli {
        Cli::Claude => {
            push(&["-p", "--output-format", "stream-json", "--verbose"]);
            // A list option: the flag after it ends the list.
            push(&["--disallowedTools"]);
            push(CLAUDE_BLOCKED_TOOLS);
            if write {
                // Courtesy only: these rules match a command's start, so
                // `git -C x push` gets through. The process environment is
                // what actually stops pushes (see `engine::write_env`).
                if !pushing {
                    push(CLAUDE_BLOCKED_COMMANDS);
                }
                if !bypass {
                    // acceptEdits approves edits but asks for every command,
                    // and nobody answers a headless run: tests, typecheck and
                    // git would all be refused.
                    push(&["--allowedTools", "Bash"]);
                }
            }
            let mode = match (request.access, bypass) {
                (Access::ReadOnly, _) => "plan",
                (_, true) => "bypassPermissions",
                (_, false) => "acceptEdits",
            };
            push(&["--permission-mode", mode]);
            if let Some(session) = request.resume {
                push(&["--resume", session]);
            }
            if let Some(schema) = request.schema {
                push(&["--json-schema", &schema.to_string()]);
            }
            if let Some(model) = &profile.model {
                push(&["--model", model]);
            }
            if let Some(effort) = &profile.effort {
                push(&["--effort", effort]);
            }
            if let Some(budget) = request.budget_usd {
                push(&["--max-budget-usd", &format!("{:.2}", budget.max(0.01))]);
            }
            for dir in request.extra_dirs {
                push(&["--add-dir", &dir.to_string_lossy()]);
            }
        }
        Cli::Codex => {
            match request.resume {
                Some(session) => push(&["exec", "resume", session]),
                None => push(&["exec"]),
            }
            push(&["--json", "--skip-git-repo-check"]);
            if bypass && write {
                push(&["--dangerously-bypass-approvals-and-sandbox"]);
            } else {
                // `exec resume` has no `-s`; the config key works for both.
                let sandbox = if write {
                    "workspace-write"
                } else {
                    "read-only"
                };
                push(&["-c", &format!("sandbox_mode=\"{sandbox}\"")]);
                if pushing {
                    // Pushing needs the network; rebasing and committing
                    // write to the repository's git folder (outside).
                    push(&["-c", "sandbox_workspace_write.network_access=true"]);
                    for dir in request.extra_dirs {
                        push(&["--add-dir", &dir.to_string_lossy()]);
                    }
                }
            }
            if let Some(file) = request.schema_file {
                push(&["--output-schema", &file.to_string_lossy()]);
            }
            if let Some(model) = &profile.model {
                push(&["-m", model]);
            }
            if let Some(effort) = &profile.effort {
                push(&["-c", &format!("model_reasoning_effort=\"{effort}\"")]);
            }
        }
        Cli::Agy => {
            push(&["--output-format", "stream-json"]);
            match (request.access, bypass) {
                (Access::ReadOnly, _) => push(&["--mode", "plan"]),
                (_, true) => push(&["--dangerously-skip-permissions"]),
                // The sandbox restricts the terminal: no push from there.
                (Access::Push, false) => push(&["--mode", "accept-edits"]),
                (Access::Write, false) => push(&["--mode", "accept-edits", "--sandbox"]),
            }
            if let Some(session) = request.resume {
                push(&["--conversation", session]);
            }
            if let Some(file) = request.schema_file {
                push(&["--json-schema", &file.to_string_lossy()]);
            }
            if let Some(model) = &profile.model {
                push(&["--model", model]);
            }
            if let Some(effort) = &profile.effort {
                push(&["--effort", effort]);
            }
        }
        Cli::Cursor => {
            push(&["-p", "--output-format", "stream-json", "--trust"]);
            push(&["--workspace", &request.cwd.to_string_lossy()]);
            // `-p` alone may write and run shell commands: read-only steps
            // must say so, write steps get the sandbox.
            match (request.access, bypass) {
                (Access::ReadOnly, _) => push(&["--mode", "plan"]),
                (_, true) => push(&["--force"]),
                (Access::Push, false) => push(&["--sandbox", "disabled"]),
                (Access::Write, false) => push(&["--sandbox", "enabled"]),
            }
            if let Some(session) = request.resume {
                push(&["--resume", session]);
            }
            if let Some(model) = &profile.model {
                push(&["--model", model]);
            }
            if let Some(schema) = request.schema {
                prompt.push_str(&schema_instruction(schema));
            }
        }
    }
    args.extend(profile.extra_args.iter().cloned());
    match cli {
        // agy reads its prompt as the value of -p.
        Cli::Agy => {
            args.push("-p".into());
            args.push(prompt);
        }
        // List options (`--add-dir`, `--disallowedTools`, maybe in the extra
        // arguments) would take the prompt as one more item.
        Cli::Claude => {
            args.push("--".into());
            args.push(prompt);
        }
        _ => args.push(prompt),
    }
    args
}

fn schema_instruction(schema: &Value) -> String {
    format!(
        "\n\nRespond with ONLY one JSON object (no prose, no code fence) that matches this \
         JSON schema:\n{}\n",
        serde_json::to_string_pretty(schema).unwrap_or_default()
    )
}

/// What a finished CLI process produced.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    pub session_id: Option<String>,
    /// The final answer.
    pub text: String,
    /// Schema-checked output when the CLI reports it separately.
    pub structured: Option<Value>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: Option<f64>,
    /// The CLI said the turn failed.
    pub error: Option<String>,
    /// Tool calls the CLI refused for lack of permission.
    pub denials: u32,
    pub finished: bool,
    /// The account's usage limits, as last reported during the run.
    pub quota: Option<Quota>,
}

/// One usage-limit window of a subscription.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct QuotaWindow {
    /// 0.0 to 1.0.
    pub utilization: f64,
    /// Unix seconds.
    pub resets_at: i64,
}

/// Claude's subscription limits (`rate_limit_event`).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Quota {
    pub five_hour: Option<QuotaWindow>,
    pub seven_day: Option<QuotaWindow>,
    /// The account is over a limit right now.
    pub rejected: bool,
}

impl Quota {
    /// The fullest window: its name and state.
    pub fn fullest(&self) -> Option<(&'static str, &QuotaWindow)> {
        [("5 giờ", &self.five_hour), ("7 ngày", &self.seven_day)]
            .into_iter()
            .filter_map(|(name, window)| window.as_ref().map(|window| (name, window)))
            .max_by(|a, b| a.1.utilization.total_cmp(&b.1.utilization))
    }

    /// The window that blocks work now: over `limit` (0..1), or any window
    /// while the account is refused, and not reset yet at `now`.
    pub fn blocking(&self, limit: Option<f64>, now: i64) -> Option<(&'static str, &QuotaWindow)> {
        [("5 giờ", &self.five_hour), ("7 ngày", &self.seven_day)]
            .into_iter()
            .filter_map(|(name, window)| window.as_ref().map(|window| (name, window)))
            .filter(|(_, window)| window.resets_at == 0 || window.resets_at > now)
            .filter(|(_, window)| self.rejected || limit.is_some_and(|l| window.utilization >= l))
            .max_by(|a, b| a.1.utilization.total_cmp(&b.1.utilization))
    }

    fn parse(info: &Value) -> Option<Self> {
        let window = |value: Option<&Value>| {
            let value = value?;
            Some(QuotaWindow {
                utilization: value.get("utilization")?.as_f64()?,
                resets_at: value.get("resetsAt").and_then(Value::as_i64).unwrap_or(0),
            })
        };
        let windows = info.get("unifiedWindows");
        let quota = Self {
            five_hour: window(windows.and_then(|w| w.get("five_hour"))),
            seven_day: window(windows.and_then(|w| w.get("seven_day"))),
            rejected: str_at(info, "status") == "rejected",
        };
        (quota.five_hour.is_some() || quota.seven_day.is_some() || quota.rejected).then_some(quota)
    }
}

impl Outcome {
    /// The CLI stopped because the account is out of usage (Claude's "You've
    /// hit your session limit", codex's "usage limit"): wait, do not fail.
    pub fn hit_limit(&self) -> bool {
        if self.quota.as_ref().is_some_and(|quota| quota.rejected) {
            return true;
        }
        let Some(error) = &self.error else {
            return false;
        };
        let error = error.to_lowercase();
        [
            "usage limit",
            "session limit",
            "rate limit",
            "out of usage",
            "quota exceeded",
            "quota reached",
            "resource_exhausted",
        ]
        .iter()
        .any(|needle| error.contains(needle))
    }

    /// Seconds until the limit resets, when the error says ("Resets in
    /// 149h8m18s", "resets in 2d 3h").
    pub fn limit_resets_in(&self) -> Option<u64> {
        let error = self.error.as_deref()?.to_lowercase();
        let rest = &error[error.find("resets in ")? + "resets in ".len()..];
        let (mut total, mut number, mut any) = (0u64, 0u64, false);
        for ch in rest.chars() {
            match ch {
                '0'..='9' => number = number * 10 + ch.to_digit(10)? as u64,
                'd' | 'h' | 'm' | 's' => {
                    let unit = match ch {
                        'd' => 86_400,
                        'h' => 3_600,
                        'm' => 60,
                        _ => 1,
                    };
                    total += number * unit;
                    number = 0;
                    any = true;
                }
                ' ' => {}
                _ => break,
            }
        }
        any.then_some(total)
    }

    /// The structured answer: the CLI's own, else JSON found in the text.
    pub fn json(&self) -> Option<Value> {
        self.structured.clone().or_else(|| extract_json(&self.text))
    }
}

/// The first JSON object in `text`: the whole text, a ```json fence, or
/// the span from the first `{` to the last `}`.
pub fn extract_json(text: &str) -> Option<Value> {
    let text = text.trim();
    if let Ok(value @ Value::Object(_)) = serde_json::from_str(text) {
        return Some(value);
    }
    if let Some(start) = text.find("```") {
        let body = &text[start + 3..];
        let body = body.strip_prefix("json").unwrap_or(body);
        if let Some(end) = body.find("```") {
            if let Ok(value @ Value::Object(_)) = serde_json::from_str(body[..end].trim()) {
                return Some(value);
            }
        }
    }
    let (start, end) = (text.find('{')?, text.rfind('}')?);
    match serde_json::from_str(text.get(start..=end)?) {
        Ok(value @ Value::Object(_)) => Some(value),
        _ => None,
    }
}

/// A line for the live log tab.
#[derive(Debug, Clone, PartialEq)]
pub enum LogLine {
    /// What the agent says.
    Text(String),
    /// A tool it runs: name and short detail.
    Tool(String, String),
    /// A tool result that failed.
    ToolError(String),
    Info(String),
    Error(String),
}

/// Reads one CLI's stream-json, line by line.
pub struct Parser {
    cli: Cli,
    pub outcome: Outcome,
    /// agy streams text in deltas; kept until the step is done.
    pending_text: String,
    /// codex: the last agent message is the answer.
    last_message: Option<String>,
}

impl Parser {
    pub fn new(cli: Cli) -> Self {
        Self {
            cli,
            outcome: Outcome::default(),
            pending_text: String::new(),
            last_message: None,
        }
    }

    pub fn feed(&mut self, line: &str) -> Vec<LogLine> {
        let line = line.trim();
        if line.is_empty() {
            return Vec::new();
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            // Not JSON: a CLI printing plain text (warnings, errors).
            return vec![LogLine::Info(line.to_string())];
        };
        match self.cli {
            Cli::Claude | Cli::Cursor => self.claude(&value),
            Cli::Codex => self.codex(&value),
            Cli::Agy => self.agy(&value),
        }
    }

    /// Call when the process has exited.
    pub fn finish(mut self) -> Outcome {
        if self.outcome.text.is_empty() {
            if let Some(message) = self.last_message.take() {
                self.outcome.text = message;
            } else if !self.pending_text.is_empty() {
                self.outcome.text = std::mem::take(&mut self.pending_text);
            }
        }
        self.outcome
    }

    fn claude(&mut self, value: &Value) -> Vec<LogLine> {
        let mut out = Vec::new();
        match str_at(value, "type") {
            "system" => {
                if let Some(session) = value.get("session_id").and_then(Value::as_str) {
                    self.outcome.session_id = Some(session.to_string());
                }
                if str_at(value, "subtype") == "init" {
                    let model = value.get("model").and_then(Value::as_str).unwrap_or("");
                    out.push(LogLine::Info(format!(
                        "Phiên {} {model}",
                        self.outcome.session_id.as_deref().unwrap_or("?")
                    )));
                }
            }
            "assistant" => {
                for block in content_blocks(value) {
                    match str_at(block, "type") {
                        "text" => {
                            let text = str_at(block, "text").trim();
                            if !text.is_empty() {
                                out.push(LogLine::Text(text.to_string()));
                            }
                        }
                        "tool_use" => out.push(LogLine::Tool(
                            str_at(block, "name").to_string(),
                            tool_detail(block.get("input")),
                        )),
                        _ => {}
                    }
                }
            }
            "user" => {
                for block in content_blocks(value) {
                    if str_at(block, "type") == "tool_result"
                        && block.get("is_error").and_then(Value::as_bool) == Some(true)
                    {
                        out.push(LogLine::ToolError(first_line(&content_text(
                            block.get("content"),
                        ))));
                    }
                }
            }
            "rate_limit_event" => {
                if let Some(quota) = value.get("rate_limit_info").and_then(Quota::parse) {
                    self.outcome.quota = Some(quota);
                }
            }
            // cursor-agent reports tools as their own events.
            "tool_call" if str_at(value, "subtype") == "started" => {
                if let Some(Value::Object(call)) = value.get("tool_call") {
                    if let Some((name, body)) = call.iter().next() {
                        let name = name.trim_end_matches("ToolCall");
                        out.push(LogLine::Tool(
                            name.to_string(),
                            tool_detail(body.get("args")),
                        ));
                    }
                }
            }
            "result" => {
                let outcome = &mut self.outcome;
                outcome.finished = true;
                if let Some(session) = value.get("session_id").and_then(Value::as_str) {
                    outcome.session_id = Some(session.to_string());
                }
                outcome.text = str_at(value, "result").to_string();
                outcome.structured = value
                    .get("structured_output")
                    .filter(|v| v.is_object())
                    .cloned();
                // `modelUsage` covers the whole session (every turn and
                // model); `usage` only the last turn.
                if let Some(Value::Object(models)) = value.get("modelUsage") {
                    outcome.input_tokens = models
                        .values()
                        .map(|usage| {
                            [
                                "inputTokens",
                                "cacheCreationInputTokens",
                                "cacheReadInputTokens",
                            ]
                            .iter()
                            .map(|key| num_at(usage, key))
                            .sum::<u64>()
                        })
                        .sum();
                    outcome.output_tokens = models
                        .values()
                        .map(|usage| num_at(usage, "outputTokens"))
                        .sum();
                } else if let Some(usage) = value.get("usage") {
                    outcome.input_tokens = [
                        "input_tokens",
                        "cache_creation_input_tokens",
                        "cache_read_input_tokens",
                    ]
                    .iter()
                    .map(|key| num_at(usage, key))
                    .sum();
                    outcome.output_tokens = num_at(usage, "output_tokens");
                }
                outcome.cost_usd = value.get("total_cost_usd").and_then(Value::as_f64);
                if str_at(value, "api_error") == "usage_limit_reached" {
                    outcome.quota.get_or_insert_with(Quota::default).rejected = true;
                }
                outcome.denials = value
                    .get("permission_denials")
                    .and_then(Value::as_array)
                    .map_or(0, |list| list.len() as u32);
                if value.get("is_error").and_then(Value::as_bool) == Some(true)
                    || str_at(value, "subtype").starts_with("error")
                {
                    let errors = value
                        .get("errors")
                        .and_then(Value::as_array)
                        .map(|list| {
                            list.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join("; ")
                        })
                        .unwrap_or_default();
                    let message = if !outcome.text.is_empty() {
                        outcome.text.clone()
                    } else if str_at(value, "subtype") == "error_max_budget_usd" {
                        format!("Dừng vì chạm ngân sách ({errors})")
                    } else if !errors.is_empty() {
                        errors
                    } else {
                        str_at(value, "subtype").to_string()
                    };
                    outcome.error = Some(message.clone());
                    out.push(LogLine::Error(message));
                }
            }
            _ => {}
        }
        out
    }

    fn codex(&mut self, value: &Value) -> Vec<LogLine> {
        let mut out = Vec::new();
        match str_at(value, "type") {
            "thread.started" => {
                if let Some(thread) = value.get("thread_id").and_then(Value::as_str) {
                    self.outcome.session_id = Some(thread.to_string());
                    out.push(LogLine::Info(format!("Phiên {thread}")));
                }
            }
            "item.started" | "item.completed" => {
                let completed = str_at(value, "type") == "item.completed";
                let Some(item) = value.get("item") else {
                    return out;
                };
                match (str_at(item, "type"), completed) {
                    ("agent_message", true) => {
                        let text = str_at(item, "text").to_string();
                        out.push(LogLine::Text(text.clone()));
                        self.last_message = Some(text);
                    }
                    ("command_execution", false) => out.push(LogLine::Tool(
                        "shell".into(),
                        str_at(item, "command").to_string(),
                    )),
                    ("command_execution", true) => {
                        let code = item.get("exit_code").and_then(Value::as_i64);
                        if code.is_some_and(|code| code != 0) {
                            out.push(LogLine::ToolError(format!(
                                "{} → mã {}",
                                str_at(item, "command"),
                                code.unwrap_or(-1)
                            )));
                        }
                    }
                    ("file_change", true) => {
                        let files: Vec<String> = item
                            .get("changes")
                            .and_then(Value::as_array)
                            .map(|changes| {
                                changes
                                    .iter()
                                    .map(|c| format!("{} {}", str_at(c, "kind"), str_at(c, "path")))
                                    .collect()
                            })
                            .unwrap_or_default();
                        out.push(LogLine::Tool("edit".into(), files.join(", ")));
                    }
                    ("mcp_tool_call", false) => out.push(LogLine::Tool(
                        format!("{}.{}", str_at(item, "server"), str_at(item, "tool")),
                        String::new(),
                    )),
                    ("web_search", false) => out.push(LogLine::Tool(
                        "web_search".into(),
                        str_at(item, "query").into(),
                    )),
                    ("error", _) => out.push(LogLine::Error(str_at(item, "message").to_string())),
                    _ => {}
                }
            }
            "turn.completed" => {
                self.outcome.finished = true;
                if let Some(usage) = value.get("usage") {
                    self.outcome.input_tokens += num_at(usage, "input_tokens");
                    self.outcome.output_tokens += num_at(usage, "output_tokens");
                }
            }
            "turn.failed" => {
                let message = value
                    .get("error")
                    .map(|error| str_at(error, "message").to_string())
                    .unwrap_or_else(|| "turn.failed".into());
                self.outcome.error = Some(message.clone());
                out.push(LogLine::Error(message));
            }
            "error" => {
                let message = str_at(value, "message").to_string();
                // Reconnect notices come as errors too; only a failed turn fails.
                out.push(LogLine::Error(message));
            }
            _ => {}
        }
        out
    }

    fn agy(&mut self, value: &Value) -> Vec<LogLine> {
        let mut out = Vec::new();
        match str_at(value, "event") {
            "init" => {
                if let Some(id) = value.get("conversation_id").and_then(Value::as_str) {
                    self.outcome.session_id = Some(id.to_string());
                    out.push(LogLine::Info(format!("Phiên {id}")));
                }
            }
            "step_update" => {
                let Some(step) = value.get("step_update") else {
                    return out;
                };
                let state = str_at(step, "state");
                match str_at(step, "step_type") {
                    "agent_response" => {
                        self.pending_text.push_str(str_at(step, "text_delta"));
                        if state == "DONE" {
                            let text = std::mem::take(&mut self.pending_text);
                            if !text.trim().is_empty() {
                                out.push(LogLine::Text(text.trim().to_string()));
                            }
                        }
                    }
                    "tool" if state == "ACTIVE" => {
                        let info = step.get("tool_info");
                        out.push(LogLine::Tool(
                            str_at(step, "tool_name").to_string(),
                            tool_detail(info.and_then(|info| info.get("parameters"))),
                        ));
                    }
                    "tool" if state == "ERROR" || state == "FAILED" => {
                        out.push(LogLine::ToolError(str_at(step, "tool_name").to_string()))
                    }
                    _ => {}
                }
            }
            "result" => {
                let Some(result) = value.get("result") else {
                    return out;
                };
                let outcome = &mut self.outcome;
                outcome.finished = true;
                if let Some(id) = result.get("conversation_id").and_then(Value::as_str) {
                    outcome.session_id = Some(id.to_string());
                }
                outcome.text = str_at(result, "response").trim().to_string();
                if let Some(usage) = result.get("usage") {
                    outcome.input_tokens =
                        num_at(usage, "input_tokens") + num_at(usage, "cache_read_tokens");
                    outcome.output_tokens = num_at(usage, "output_tokens");
                }
                let status = str_at(result, "status");
                if !status.is_empty() && status != "SUCCESS" {
                    // "Individual quota reached. ... Resets in 149h8m18s."
                    let error = str_at(result, "error").trim();
                    let message = if error.is_empty() {
                        format!("agy kết thúc với trạng thái {status}")
                    } else {
                        format!("agy: {error}")
                    };
                    outcome.error = Some(message.clone());
                    out.push(LogLine::Error(message));
                }
            }
            "error" => {
                let message = value
                    .get("error")
                    .map(|error| match error {
                        Value::String(text) => text.clone(),
                        other => str_at(other, "message").to_string(),
                    })
                    .unwrap_or_default();
                self.outcome.error = Some(message.clone());
                out.push(LogLine::Error(message));
            }
            _ => {}
        }
        out
    }
}

fn str_at<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn num_at(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn content_blocks(value: &Value) -> impl Iterator<Item = &Value> {
    value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn content_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .map(|block| str_at(block, "text"))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    shorten(line, 200)
}

pub fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// The interesting argument of a tool call: a command, a path, a pattern.
fn tool_detail(input: Option<&Value>) -> String {
    let Some(input) = input else {
        return String::new();
    };
    for key in [
        "command",
        "CommandLine",
        "file_path",
        "path",
        "Path",
        "AbsolutePath",
        "TargetFile",
        "pattern",
        "Query",
        "query",
        "url",
        "description",
    ] {
        if let Some(text) = input.get(key).and_then(Value::as_str) {
            return shorten(text.lines().next().unwrap_or(""), 160);
        }
    }
    String::new()
}

/// Places a CLI may be installed that an app started from the desktop does
/// not have on its PATH (npm, nvm, bun, ~/.local/bin).
pub fn extra_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let Some(home) = crate::ssh::known_hosts::home_dir() else {
        return dirs;
    };
    for rel in [
        ".local/bin",
        ".bun/bin",
        ".npm-global/bin",
        ".volta/bin",
        ".cargo/bin",
        "bin",
    ] {
        dirs.push(home.join(rel));
    }
    // Newest node first.
    if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
        let mut nodes: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path().join("bin"))
            .collect();
        nodes.sort();
        nodes.reverse();
        dirs.extend(nodes);
    }
    #[cfg(not(windows))]
    dirs.extend(
        ["/usr/local/bin", "/opt/homebrew/bin"]
            .iter()
            .map(PathBuf::from),
    );
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs
}

/// PATH entries to search: the process PATH, then [`extra_bin_dirs`].
pub fn search_path() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    for dir in extra_bin_dirs() {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

/// Where the CLI is installed, if anywhere.
pub fn find(cli: Cli) -> Option<PathBuf> {
    let dirs = search_path();
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };
    for name in cli.binaries() {
        for dir in &dirs {
            for ext in exts {
                let candidate = dir.join(format!("{name}{ext}"));
                if is_executable(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

/// PATH for a CLI process: its own folder first (a node script finds the
/// node it was installed with), then everything searched.
pub fn child_path(program: &Path) -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = program
        .parent()
        .map(Path::to_path_buf)
        .into_iter()
        .collect();
    for dir in search_path() {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(cli: Cli, lines: &[&str]) -> (Vec<LogLine>, Outcome) {
        let mut parser = Parser::new(cli);
        let mut log = Vec::new();
        for line in lines {
            log.extend(parser.feed(line));
        }
        (log, parser.finish())
    }

    #[test]
    fn claude_stream_gives_session_text_tools_and_usage() {
        let (log, outcome) = feed(
            Cli::Claude,
            &[
                r#"{"type":"system","subtype":"init","session_id":"s1","model":"opus"}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":""},{"type":"tool_use","name":"Bash","input":{"command":"cargo test\nmore"}}]},"session_id":"s1"}"#,
                r#"{"type":"user","message":{"content":[{"type":"tool_result","is_error":true,"content":"denied\nx"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"OK"}]}}"#,
                r#"{"type":"result","subtype":"success","is_error":false,"result":"OK","session_id":"s1","total_cost_usd":0.08,"usage":{"input_tokens":2,"cache_creation_input_tokens":10,"cache_read_input_tokens":5,"output_tokens":51},"permission_denials":[{}],"structured_output":{"a":1}}"#,
            ],
        );
        assert_eq!(
            log,
            vec![
                LogLine::Info("Phiên s1 opus".into()),
                LogLine::Tool("Bash".into(), "cargo test".into()),
                LogLine::ToolError("denied".into()),
                LogLine::Text("OK".into()),
            ]
        );
        assert_eq!(outcome.session_id.as_deref(), Some("s1"));
        assert_eq!(outcome.text, "OK");
        assert_eq!((outcome.input_tokens, outcome.output_tokens), (17, 51));
        assert_eq!(outcome.cost_usd, Some(0.08));
        assert_eq!(outcome.denials, 1);
        assert_eq!(outcome.json(), Some(serde_json::json!({"a": 1})));
        assert!(outcome.error.is_none() && outcome.finished);
    }

    #[test]
    fn codex_stream_uses_the_last_message_and_thread_id() {
        let (log, outcome) = feed(
            Cli::Codex,
            &[
                r#"{"type":"thread.started","thread_id":"t1"}"#,
                r#"{"type":"item.started","item":{"type":"command_execution","command":"ls"}}"#,
                r#"{"type":"item.completed","item":{"type":"command_execution","command":"ls","exit_code":2}}"#,
                r#"{"type":"item.completed","item":{"type":"file_change","changes":[{"path":"a.rs","kind":"update"}]}}"#,
                r#"{"type":"item.completed","item":{"type":"agent_message","text":"{\"verdict\":\"approved\"}"}}"#,
                r#"{"type":"turn.completed","usage":{"input_tokens":13,"output_tokens":5}}"#,
                "Reading additional input from stdin...",
            ],
        );
        assert_eq!(log[1], LogLine::Tool("shell".into(), "ls".into()));
        assert_eq!(log[2], LogLine::ToolError("ls → mã 2".into()));
        assert_eq!(log[3], LogLine::Tool("edit".into(), "update a.rs".into()));
        assert_eq!(outcome.session_id.as_deref(), Some("t1"));
        assert_eq!(
            outcome.json(),
            Some(serde_json::json!({"verdict": "approved"}))
        );
        assert_eq!((outcome.input_tokens, outcome.output_tokens), (13, 5));
        assert!(outcome.finished);
    }

    #[test]
    fn agy_stream_joins_text_deltas_and_reads_the_result() {
        let (log, outcome) = feed(
            Cli::Agy,
            &[
                r#"{"event":"init","conversation_id":"c1"}"#,
                r#"{"event":"step_update","step_update":{"state":"ACTIVE","step_type":"tool","tool_name":"write_file","tool_info":{"parameters":{"Path":"/x/plan.md"}}}}"#,
                r#"{"event":"step_update","step_update":{"state":"ACTIVE","step_type":"agent_response","text_delta":"O"}}"#,
                r#"{"event":"step_update","step_update":{"state":"DONE","step_type":"agent_response","text_delta":"K\n"}}"#,
                r#"{"event":"result","result":{"conversation_id":"c1","status":"SUCCESS","response":"OK\n","usage":{"input_tokens":14,"output_tokens":3,"cache_read_tokens":6}}}"#,
            ],
        );
        assert_eq!(
            log[1],
            LogLine::Tool("write_file".into(), "/x/plan.md".into())
        );
        assert_eq!(log[2], LogLine::Text("OK".into()));
        assert_eq!(outcome.text, "OK");
        assert_eq!(outcome.session_id.as_deref(), Some("c1"));
        assert_eq!((outcome.input_tokens, outcome.output_tokens), (20, 3));
    }

    #[test]
    fn json_is_found_in_fences_and_prose() {
        assert_eq!(
            extract_json("Here:\n```json\n{\"a\": 1}\n```\nDone"),
            Some(serde_json::json!({"a": 1}))
        );
        assert_eq!(
            extract_json("Kết quả {\"a\": {\"b\": 2}} xong"),
            Some(serde_json::json!({"a": {"b": 2}}))
        );
        assert_eq!(extract_json("no json"), None);
    }

    #[test]
    fn read_only_steps_never_get_write_or_bypass_flags() {
        let profile = CliProfile {
            cli: "x".into(),
            bypass_permissions: true,
            ..Default::default()
        };
        let cwd = Path::new("/repo");
        let schema = serde_json::json!({"type": "object"});
        for cli in Cli::ALL {
            let request = Request {
                prompt: "review".into(),
                cwd,
                access: Access::ReadOnly,
                resume: Some("s1"),
                schema: Some(&schema),
                schema_file: Some(Path::new("/tmp/schema.json")),
                profile: &profile,
                budget_usd: None,
                extra_dirs: &[],
            };
            let args = args(cli, &request).join(" ");
            for flag in [
                "dangerously",
                "bypass",
                "--force",
                "workspace-write",
                "accept",
            ] {
                assert!(!args.contains(flag), "{cli:?}: {args}");
            }
            assert!(
                args.contains("plan") || args.contains("read-only"),
                "{cli:?}: {args}"
            );
            assert!(args.contains("s1"), "{cli:?} resumes: {args}");
        }
        let request = Request {
            prompt: "do it".into(),
            cwd,
            access: Access::Write,
            resume: None,
            schema: None,
            schema_file: None,
            profile: &CliProfile::default(),
            budget_usd: Some(1.5),
            extra_dirs: &[],
        };
        assert_eq!(
            args(Cli::Codex, &request),
            vec![
                "exec",
                "--json",
                "--skip-git-repo-check",
                "-c",
                "sandbox_mode=\"workspace-write\"",
                "do it"
            ]
        );
        let agy = args(Cli::Agy, &request);
        assert_eq!(&agy[agy.len() - 2..], ["-p", "do it"]);
    }
    #[test]
    fn claude_steps_are_capped_and_counted_over_the_whole_session() {
        let request = Request {
            prompt: "design".into(),
            cwd: Path::new("/repo"),
            access: Access::ReadOnly,
            resume: None,
            schema: None,
            schema_file: None,
            profile: &CliProfile::default(),
            budget_usd: Some(2.345),
            extra_dirs: &[PathBuf::from("/runs/r1/context")],
        };
        let args = super::args(Cli::Claude, &request);
        let at = |flag: &str| args.iter().position(|arg| arg == flag).unwrap();
        assert_eq!(args[at("--max-budget-usd") + 1], "2.35");
        // The tool list ends at the next flag, never swallowing the prompt.
        assert_eq!(args[at("--disallowedTools") + 1], "Agent");
        assert!(args[at("--disallowedTools") + CLAUDE_BLOCKED_TOOLS.len() + 1].starts_with("--"));
        assert_eq!(args[args.len() - 2..], ["--", "design"]);
        // Reading steps neither run commands freely nor need the git rules.
        assert!(!args.contains(&"--allowedTools".to_string()));
        assert!(!args.iter().any(|arg| arg.starts_with("Bash(")));
        let write = Request {
            access: Access::Write,
            ..request
        };
        let args = super::args(Cli::Claude, &write);
        let at = |flag: &str| args.iter().position(|arg| arg == flag).unwrap();
        assert_eq!(args[at("--allowedTools") + 1], "Bash");
        assert!(args.contains(&"Bash(git push:*)".to_string()));
        assert_eq!(args[at("--permission-mode") + 1], "acceptEdits");
        // Shipping: commands run, git is not blocked, codex reaches the
        // network and the repository's git folder.
        let git_dir = [PathBuf::from("/repo/.git")];
        let shipping = Request {
            access: Access::Push,
            extra_dirs: &git_dir,
            ..write
        };
        let claude = super::args(Cli::Claude, &shipping);
        assert!(claude.contains(&"--allowedTools".to_string()));
        assert!(!claude.iter().any(|arg| arg.starts_with("Bash(git")));
        let codex = super::args(Cli::Codex, &shipping).join(" ");
        assert!(
            codex.contains("sandbox_workspace_write.network_access=true"),
            "{codex}"
        );
        assert!(codex.contains("--add-dir /repo/.git"), "{codex}");
        let agy = super::args(Cli::Agy, &shipping).join(" ");
        assert!(!agy.contains("--sandbox"), "{agy}");

        let mut parser = Parser::new(Cli::Claude);
        parser.feed(
            r#"{"type":"result","subtype":"error_max_budget_usd","is_error":true,"total_cost_usd":2.5,
            "errors":["Reached maximum budget ($2.35)"],"usage":{"input_tokens":0,"output_tokens":0},
            "modelUsage":{"a":{"inputTokens":10,"cacheReadInputTokens":1000,"cacheCreationInputTokens":100,"outputTokens":50},
            "b":{"inputTokens":1,"cacheReadInputTokens":0,"cacheCreationInputTokens":0,"outputTokens":5}}}"#
                .replace('\n', " ")
                .as_str(),
        );
        let outcome = parser.finish();
        assert_eq!((outcome.input_tokens, outcome.output_tokens), (1111, 55));
        assert_eq!(outcome.cost_usd, Some(2.5));
        assert!(outcome.error.unwrap().contains("ngân sách"));
    }

    #[test]
    fn agy_out_of_quota_is_a_limit_with_its_reset() {
        let mut parser = Parser::new(Cli::Agy);
        parser.feed(
            r#"{"event":"result","result":{"conversation_id":"c9","status":"ERROR","response":"","error":"Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 149h8m18s.","usage":{"input_tokens":0,"output_tokens":0}}}"#,
        );
        let outcome = parser.finish();
        assert!(outcome
            .error
            .as_deref()
            .unwrap()
            .contains("Individual quota reached"));
        assert!(outcome.hit_limit());
        assert_eq!(outcome.limit_resets_in(), Some(149 * 3600 + 8 * 60 + 18));
        assert_eq!(outcome.session_id.as_deref(), Some("c9"));
    }
}
