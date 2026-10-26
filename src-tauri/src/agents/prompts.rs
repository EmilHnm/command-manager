//! What each step asks of its CLI, and the schemas of structured answers.
//!
//! Prompts are in English (every CLI follows English instructions best);
//! the agents are told to write for the user in the language of the
//! request. Schemas are "strict" (every property required, no extras) so
//! `codex --output-schema` accepts them too.

use super::context::ContextFile;
use crate::db::repos::agents::{
    CaseResult, CheckResult, HumanRound, Project, Review, Run, Task, TestGuide, ROOT_REPO,
};
use serde_json::{json, Value};
use std::path::Path;

/// A review prompt carries at most this much diff; the reviewer can run
/// `git diff` itself for the rest.
pub const REVIEW_DIFF_BYTES: usize = 60 * 1024;

/// Every step runs on the user's quota: exploration has to stay small.
const ECONOMY: &str = "Work economically: read only the files this step needs (entry points, \
the modules involved, their conventions) and do not survey the whole codebase or verify \
every detail; prefer targeted searches (grep, file listings) over reading whole files.";

/// How every worker turn ends: the note the tasks after it get.
pub const HANDOFF_ASK: &str = "Finish with a handoff note for the teammates who work after you \
(under 15 lines): what you changed and where, decisions you made, and helpers or \
conventions they should reuse instead of rediscovering them.";

/// A note shown to a task about one merged task before it.
const HANDOFF_NOTE_BYTES: usize = 2000;
/// All notes of one prompt.
const HANDOFF_BYTES: usize = 12 * 1024;

fn cut_text(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Notes of merged tasks, newest last, as many as fit.
fn handoffs_block(done: &[&Task]) -> String {
    let mut notes: Vec<String> = Vec::new();
    let mut size = 0;
    for task in done.iter().rev() {
        let Some(note) = task
            .handoff
            .as_deref()
            .filter(|note| !note.trim().is_empty())
        else {
            continue;
        };
        let note = format!(
            "### `{}` — {}\n{}",
            task.key,
            task.title,
            cut_text(note.trim(), HANDOFF_NOTE_BYTES)
        );
        if size + note.len() > HANDOFF_BYTES {
            break;
        }
        size += note.len();
        notes.push(note);
    }
    if notes.is_empty() {
        return String::new();
    }
    notes.reverse();
    format!(
        "\nNotes from teammates whose work is already merged into your branch (read them \
before exploring, and build on what they did):\n{}\n",
        notes.join("\n\n")
    )
}

fn notes(project: &Project) -> String {
    match project.notes.as_deref() {
        Some(notes) => format!("\nProject notes from the user:\n{notes}\n"),
        None => String::new(),
    }
}

/// The files the app prepared, for the agent to read instead of fetching.
fn files_block(files: &[ContextFile]) -> String {
    if files.is_empty() {
        return String::new();
    }
    let list = files
        .iter()
        .map(|file| format!("- `{}`: {}", file.path.display(), file.what))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\nThe orchestrator already prepared these files; read them instead of fetching \
the same data again (no `gh`, no web):\n{list}\n"
    )
}

fn is_multi(run: &Run) -> bool {
    !(run.repos.len() == 1 && run.repos[0].path == ROOT_REPO)
}

fn repo_names(run: &Run) -> Vec<String> {
    run.repos.iter().map(|repo| repo.path.clone()).collect()
}

/// How the folder is laid out, for a project of several repos.
fn layout(run: &Run, here: &str) -> String {
    if !is_multi(run) {
        return String::new();
    }
    let list = run
        .repos
        .iter()
        .map(|repo| format!("- `{}/` (base branch `{}`)", repo.path, repo.base_branch))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\n{here} is not itself a git repository: it holds several separate git repositories that together form one product:\n{list}\nPaths below are relative to {here} (e.g. `{first}/src/...`).\n",
        first = run.repos[0].path
    )
}

fn roles_line(run: &Run) -> String {
    run.roles
        .iter()
        .map(|role| role.role.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn design(
    run: &Run,
    project: &Project,
    overview: &str,
    files: &[ContextFile],
    guides: &str,
) -> String {
    format!(
        "You are the lead architect of a small team of AI coding agents working on the \
code in the current directory. Read the code you need; do NOT modify any file.
{economy} The design is a high-level guide: the workers will read the details of their \
own files later.
{layout}
Map of the code (tracked folders with file counts), so you can go straight to what matters:
<map>
{overview}</map>
{guides}{files}
Request from the user:
<request>
{request}
</request>
{notes}
Write a concise design document in Markdown for this request:
1. Scope: what changes and what does not.
2. Architecture: components, files and modules involved, data flow.
3. Shared contract: APIs, types, IPC/HTTP signatures, database changes that several \
tasks must agree on. Be exact, this is what parallel workers will rely on.
4. Risks and how to avoid them.

The team has these worker roles: {roles}. Do not list tasks yet.
Write the document in the same language as the request. Reply with the document only.",
        request = run.request,
        notes = notes(project),
        roles = roles_line(run),
        layout = layout(run, "The current directory"),
        economy = ECONOMY,
        files = files_block(files),
        guides = guides,
    )
}

/// Gate 1 sent the design back with feedback.
pub fn redesign(run: &Run, feedback: &str) -> String {
    format!(
        "The user reviewed your design and plan and asks for changes:
<feedback>
{feedback}
</feedback>

Current design (it may contain the user's own edits):
<design>
{design}
</design>

Rewrite the full design document taking the feedback into account. Same format, same \
language. Do NOT modify any file. Reply with the document only.",
        design = run.design_md.as_deref().unwrap_or(""),
    )
}

pub fn plan(run: &Run, project: &Project) -> String {
    format!(
        "Now split the work into tasks for the team, following your design:
<design>
{design}
</design>

{rules}",
        design = run.design_md.as_deref().unwrap_or(""),
        rules = plan_rules(run, project),
    )
}

/// What the plan says about automated checks: proposals for the repos that
/// have none, approved by the user at gate 1 before anything runs them.
fn checks_rule(run: &Run, project: &Project) -> String {
    let mut configured = Vec::new();
    let mut missing = Vec::new();
    for repo in &run.repos {
        let label = format!("`{}`", repo.path);
        match project.repo(&repo.path).map(|config| &config.check_cmds) {
            Some(cmds) if !cmds.is_empty() => {
                configured.push(format!("{label} ({})", cmds.join("; ")))
            }
            _ => missing.push(label),
        }
    }
    if missing.is_empty() {
        return "- `checks`: every repository already has automated checks; reply [].".into();
    }
    format!(
        "- `checks`: for each repository a task changes that has no automated checks yet \
({missing}), propose the commands the project's own rules (AGENTS.md, skills, CI, \
package.json scripts) use to verify a change: typecheck, lint, unit tests; no long builds, \
deploys or anything that writes outside the repository. `setup`: the command that prepares a \
fresh checkout (e.g. installing dependencies), \"\" if none. The user approves them before \
they run.{configured}",
        missing = missing.join(", "),
        configured = if configured.is_empty() {
            String::new()
        } else {
            format!(" Already configured: {}.", configured.join(", "))
        },
    )
}

fn plan_rules(run: &Run, project: &Project) -> String {
    format!(
        "Rules:
- Roles available: {roles}. Every task has exactly one of them.
- `key`: short kebab-case id, unique. `depends_on`: keys that must be merged first. \
Prefer few dependencies so work can go in parallel; a frontend task can start from the \
contract without waiting for the backend.
- `repos`: the repositories the task changes ({repos}). Prefer one repository per \
task; split cross-repository work into tasks linked by `depends_on` or by the contract.
- `scope`: glob patterns of files the task may change, relative to the current \
directory (e.g. `{scope_example}`). Tasks running in parallel should not share files.
- `acceptance`: checkable criteria. Each task must be completable in one focused session.
- Tasks run one after the other and each worker starts fresh: group work on the same \
files into one task rather than several small ones.
- `contract`: the shared contract (types, signatures) from the design, verbatim, so \
every worker gets it.
- When a task should follow one of the project's skills, name it in the description.
- No task for committing, pushing, rebasing or opening pull requests: the orchestrator \
merges the work and opens the pull request after the user's review. Verification belongs \
in the tasks themselves (tests next to the code).
{checks}
Write titles and descriptions in the language of the request.",
        roles = roles_line(run),
        repos = repo_names(run)
            .iter()
            .map(|repo| format!("`{repo}`"))
            .collect::<Vec<_>>()
            .join(", "),
        scope_example = if is_multi(run) {
            format!("{}/src/cart/**", run.repos[0].path)
        } else {
            "src/cart/**".to_string()
        },
        checks = checks_rule(run, project),
    )
}

/// The run was delivered; plan the work a follow-up request asks for.
pub fn follow_up(
    run: &Run,
    project: &Project,
    request: &str,
    done: &[&Task],
    files: &[ContextFile],
) -> String {
    let delivered = if run.status == "merged" {
        "merged into the base branch".to_string()
    } else {
        format!("kept on the branch `{}`", run.integration_branch)
    };
    let keys = done
        .iter()
        .map(|task| format!("`{}`", task.key))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "The work of this run was delivered ({delivered}). The user now has a follow-up \
request for it, often comments from a human code review of the pull request:
<request>
{request}
</request>
{files}
Design of the run:
<design>
{design}
</design>

Shared contract so far:
<contract>
{contract}
</contract>
{handoffs}
Plan only the work this follow-up needs, as new tasks; read code only to settle \
what a comment refers to. If a comment needs no change, leave it out. Keys must be new \
(already used: {keys}); `depends_on` names new tasks only. `contract`: only what this \
follow-up adds to or changes in the shared contract, \"\" if nothing.

{rules}",
        files = files_block(files),
        design = run.design_md.as_deref().unwrap_or(""),
        contract = run.contract.as_deref().unwrap_or("(none)"),
        handoffs = handoffs_block(done),
        rules = plan_rules(run, project),
    )
}

pub fn plan_schema(roles: &[String], repos: &[String]) -> Value {
    let strings = json!({ "type": "array", "items": { "type": "string" } });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["contract", "tasks", "checks"],
        "properties": {
            "contract": { "type": "string" },
            "checks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["repo", "setup", "commands"],
                    "properties": {
                        "repo": { "type": "string", "enum": repos },
                        "setup": { "type": "string" },
                        "commands": strings
                    }
                }
            },
            "tasks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["key", "title", "role", "repos", "description", "acceptance", "scope", "depends_on"],
                    "properties": {
                        "key": { "type": "string" },
                        "title": { "type": "string" },
                        "role": { "type": "string", "enum": roles },
                        "repos": { "type": "array", "items": { "type": "string", "enum": repos } },
                        "description": { "type": "string" },
                        "acceptance": strings,
                        "scope": strings,
                        "depends_on": strings
                    }
                }
            }
        }
    })
}

pub fn review_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["verdict", "summary", "comments"],
        "properties": {
            "verdict": { "type": "string", "enum": ["approved", "changes_requested"] },
            "summary": { "type": "string" },
            "comments": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["file", "line", "issue", "severity"],
                    "properties": {
                        "file": { "type": "string" },
                        "line": { "type": ["integer", "null"] },
                        "issue": { "type": "string" },
                        "severity": { "type": "string", "enum": ["must", "should"] }
                    }
                }
            }
        }
    })
}

/// "Verify PR": the leader checks the pull requests of the run as GitHub
/// has them, before (or between) the human reviewers.
#[allow(clippy::too_many_arguments)]
pub fn verify_pr(
    run: &Run,
    prs: &str,
    open_comments: &str,
    diff: &str,
    truncated: bool,
    guides: &str,
) -> String {
    let followups = if run.followups.is_empty() {
        String::new()
    } else {
        format!(
            "\nFollow-up requests made after it (oldest first):\n{}\n",
            run.followups
                .iter()
                .map(|f| format!("<followup>\n{}\n</followup>", f.request.trim()))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    format!(
        "You are the lead reviewer. Verify the pull request(s) of this run before the team's \
reviewers spend time on them. The current directory holds the code of the pull request \
branch(es) as they are on origin. Do NOT modify any file.
{layout}{guides}
Request of the run:
<request>
{request}
</request>
{followups}
Shared contract:
<contract>
{contract}
</contract>

Pull requests:
{prs}

Review comments on GitHub that nobody has resolved or answered yet:
{open_comments}

Diff of each pull request against its base branch:
```diff
{diff}
```{cut}

Check that the pull request does what the request and the follow-ups ask, completely, and \
nothing else: changes that belong to other work (other tickets, unrelated files, leftovers of \
another branch) are `must`. It must match the contract and the project's rules, and nothing \
may be broken. For each open comment above, say whether the code already settles it. Approve \
only when nothing `must` is left. Write the summary and issues in the language of the request.",
        layout = layout(run, "The current directory"),
        request = run.request,
        contract = run.contract.as_deref().unwrap_or("(none)"),
        open_comments = if open_comments.trim().is_empty() {
            "(none)"
        } else {
            open_comments
        },
        cut = if truncated {
            "\n(The diff was cut. Read the files for the rest.)"
        } else {
            ""
        },
    )
}

/// "Push bằng AI": ship the run's work to its pull request the way the team
/// pushes, solving what the orchestrator cannot (conflicts, failing hooks).
#[allow(clippy::too_many_arguments)]
pub fn ai_push(
    run: &Run,
    local: &str,
    branch: &str,
    base: &str,
    url: &str,
    done: &[&Task],
    note: &str,
    guides: &str,
) -> String {
    format!(
        "You are the engineer who ships this run's work. The current directory is a git \
worktree of the repository, on the local branch `{local}`, which holds the run's work. Bring \
it onto the pull request branch `{branch}` on origin (PR {url}, into `{base}`).
{guides}
Do it the way this team pushes (its push skill and AGENTS.md win over these steps):
1. `git fetch origin`, then rebase onto `origin/{base}`; never merge the base in. Resolve \
conflicts keeping the intent of both sides; what this run changed is described below. \
If `origin/{branch}` has commits this branch lacks (someone pushed), keep them: rebase onto \
or include them as the team's rules say.
2. Run the project's checks (the pre-push hook runs them as well) and fix what breaks with \
minimal changes; commit the fixes with clear messages.
3. `git push --force-with-lease origin HEAD:{branch}`. Never `--no-verify`, never plain \
`--force`. If the hook refuses, read why, fix it and push again.
4. Do not open pull requests and do not push any other branch.
Dependencies are already installed in this worktree: install again only if the lockfile \
changes. Run every command in the foreground and wait for it to finish; never poll for \
processes with `pgrep`, `sleep` loops or the like.
{note}{handoffs}
Finish with a short summary: what you rebased onto, conflicts you resolved, fixes you made, \
and the pushed commit (`git rev-parse HEAD`). Write it in the language of the request:
<request>
{request}
</request>",
        handoffs = handoffs_block(done),
        note = if note.trim().is_empty() {
            String::new()
        } else {
            format!("\nFrom the user:\n<note>\n{}\n</note>\n", note.trim())
        },
        request = adapter_cut(&run.request, 600),
    )
}

fn adapter_cut(text: &str, max: usize) -> String {
    cut_text(text.trim(), max)
}

/// The schema of what the AI prepares for a person's task.
pub fn test_guide_schema() -> Value {
    let strings = json!({ "type": "array", "items": { "type": "string" } });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "setup", "cases", "notes"],
        "properties": {
            "summary": { "type": "string" },
            "setup": strings,
            "cases": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "title", "steps", "expected"],
                    "properties": {
                        "id": { "type": "string" },
                        "title": { "type": "string" },
                        "steps": strings,
                        "expected": { "type": "string" }
                    }
                }
            },
            "notes": { "type": "string" }
        }
    })
}

/// The leader's answer to a failed manual test: fix tasks, or none when it
/// finds no bug (its summary then tells the person why).
pub fn triage_schema(roles: &[String], repos: &[String]) -> Value {
    let plan = plan_schema(roles, repos);
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "tasks"],
        "properties": {
            "summary": { "type": "string" },
            "tasks": plan["properties"]["tasks"].clone()
        }
    })
}

fn results_block(guide: Option<&TestGuide>, results: &[CaseResult]) -> String {
    results
        .iter()
        .map(|result| {
            let title = guide
                .and_then(|guide| guide.cases.iter().find(|case| case.id == result.case_id))
                .map(|case| case.title.as_str())
                .unwrap_or("");
            format!(
                "- [{}] {} {}{}",
                match result.pass {
                    Some(true) => "PASS",
                    Some(false) => "FAIL",
                    None => "not tried",
                },
                result.case_id,
                title,
                if result.note.trim().is_empty() {
                    String::new()
                } else {
                    format!(": {}", result.note.trim())
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A person's task: prepare what it asks for (a test guide, most often),
/// for the work it depends on; after fixes, for what changed.
pub fn prepare_human(
    run: &Run,
    task: &Task,
    done: &[&Task],
    last: Option<&HumanRound>,
    guides: &str,
    files: &[ContextFile],
) -> String {
    let again =
        match last {
            Some(round) => {
                format!(
            "\nThis is a new round: the person tested before and reported:\n{}\nOverall: {}\n\
The fixes made since are in the notes below. Prepare the round so the person re-checks what \
failed and what the fixes touched first; cases that passed and are unaffected can be short.\n",
            results_block(round.guide.as_ref(), &round.results),
            if round.note.trim().is_empty() { "(no note)" } else { round.note.trim() }
        )
            }
            None => String::new(),
        };
    format!(
        "You prepare a task that a PERSON will do by hand, usually testing the product's UI. \
The current directory holds the code as it is now (all the run's work merged). Do NOT modify \
any file. Read the code you need to be exact (routes, labels, messages, validation rules).
{layout}{guides}{files}
The person's task `{key}`: {title}
<description>
{description}
</description>

Produce what the description asks for. When it is about testing (the usual case): `summary` \
says what is tested and why; `setup` lists how to get there (start the app, URL or screen \
path, account or role, data to prepare); `cases` are concrete checks, each with an id \
(C1, C2...), the exact steps and the expected result (exact texts of messages where the \
design gives them); cover the normal path, validation and error cases, and the edges the \
work touched. Anything else the person should know goes to `notes`.
{again}{handoffs}
Write in the language of the request.",
        layout = layout(run, "The current directory"),
        files = files_block(files),
        key = task.key,
        title = task.title,
        description = task.description,
        handoffs = handoffs_block(done),
    )
}

/// A person's test failed: the leader confirms the problems and splits them
/// into fix tasks for the team.
#[allow(clippy::too_many_arguments)]
pub fn triage_human(
    run: &Run,
    task: &Task,
    round: &HumanRound,
    done: &[&Task],
    chat: &str,
    attachments: &[String],
    keys: &[String],
) -> String {
    let shots = if attachments.is_empty() {
        "(none)".to_string()
    } else {
        attachments
            .iter()
            .map(|path| format!("- `{path}`"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "You are the lead. A person tested `{key}` ({title}) by hand and reports failures. Do \
NOT modify any file; read the code to confirm each problem.
{layout}
What the test was about:
<description>
{description}
</description>

Results, case by case:
{results}

The person's note:
<note>
{note}
</note>

Screenshots (open them):
{shots}
{chat}{handoffs}
Confirm what is really wrong. Split the real problems into fix tasks for the team, the same \
way as the plan: roles {roles}; keys must be new (already used: {used}); `repos`, `scope`, \
`acceptance` as usual; put in each description what the person saw, the expected behaviour \
and the screenshot paths that show it. When nothing is a bug (the test was misread, or it is \
expected behaviour), reply with no tasks and explain it to the person in `summary`; \
otherwise `summary` tells the person what will be fixed. Write in the language of the \
request.",
        key = task.key,
        title = task.title,
        layout = layout(run, "The current directory"),
        description = task.description,
        results = results_block(round.guide.as_ref(), &round.results),
        note = if round.note.trim().is_empty() {
            "(none)"
        } else {
            round.note.trim()
        },
        chat = if chat.is_empty() {
            String::new()
        } else {
            format!("\nWhat the person and you said while testing:\n{chat}\n")
        },
        handoffs = handoffs_block(done),
        roles = roles_line(run),
        used = keys
            .iter()
            .map(|key| format!("`{key}`"))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// The first question of a person testing: who they talk to and about what.
pub fn human_chat(run: &Run, task: &Task, message: &str) -> String {
    let guide = task
        .human
        .as_ref()
        .and_then(|human| human.guide.as_ref())
        .map(|guide| serde_json::to_string_pretty(guide).unwrap_or_default())
        .unwrap_or_else(|| "(not prepared yet)".into());
    format!(
        "You are the lead of this run. A person is testing `{key}` ({title}) by hand and is \
stuck. Help them: answer precisely, from the code in the current directory (read it), the \
guide below and the design. Do NOT modify any file. Keep answers short and practical.
{layout}
What the test is about:
<description>
{description}
</description>

The guide they follow:
```json
{guide}
```

Their question:
<question>
{message}
</question>
Answer in the language of the question.",
        key = task.key,
        title = task.title,
        layout = layout(run, "The current directory"),
        description = task.description,
    )
}

/// Continues a step that was cut short (stopped, timed out, usage limit).
pub fn resume_step(answer: &str) -> String {
    format!(
        "Your previous turn was cut short before you finished (the process was stopped). \
Continue from where you left off: do not redo the reading or the changes already done, \
and complete the original instructions. {answer}"
    )
}

/// Asked again when an answer did not match its schema.
pub fn fix_json(error: &str) -> String {
    format!(
        "Your previous answer could not be used: {error}. Reply again with ONLY the corrected \
JSON object, matching the required schema."
    )
}

fn task_block(task: &Task) -> String {
    let list = |items: &[String]| {
        if items.is_empty() {
            "(none)".to_string()
        } else {
            items
                .iter()
                .map(|item| format!("- {item}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
    };
    format!(
        "Task `{key}` ({role}): {title}
{description}

Acceptance criteria:
{acceptance}

Files you are expected to change (scope):
{scope}",
        key = task.key,
        role = task.role,
        title = task.title,
        description = task.description,
        acceptance = list(&task.acceptance),
        scope = list(&task.scope),
    )
}

/// What a worker is given besides its task.
pub struct Shared<'a> {
    /// The design as a file the worker can read; None: in the prompt.
    pub design_file: Option<&'a Path>,
    pub files: &'a [ContextFile],
    /// The project's rules and skills for the task's repos.
    pub guides: &'a str,
    /// Merged tasks before this one, in order.
    pub done: &'a [&'a Task],
}

pub fn implement(
    run: &Run,
    project: &Project,
    task: &Task,
    earlier_work: bool,
    shared: &Shared,
) -> String {
    let earlier = if earlier_work {
        "\nThis worktree already contains earlier work on this task by another agent; \
continue from it.\n"
    } else {
        ""
    };
    format!(
        "You are a {role} engineer in a team of AI agents. {place}
{task}

Shared contract every task follows:
<contract>
{contract}
</contract>

{design}{guides}{files}{handoffs}{notes}{earlier}
Rules:
- Implement the task completely, including tests where the project has them.
- Stay within the scope; if you must touch another file, keep the change minimal.
- Do NOT run git commit, git push or change branches: the orchestrator commits for you \
and opens the pull request (pushing is disabled here). Run tests and other commands freely.
- Do not ask questions; make reasonable decisions and mention them at the end.
- {economy}
{handoff_ask}",
        role = task.role,
        place = if is_multi(run) {
            format!(
                "The current directory mirrors the product folder: each repository is a \
dedicated git worktree on your own branch.{}This task changes: {}.",
                layout(run, "The current directory"),
                task.repos.join(", ")
            )
        } else {
            "You work in a dedicated git worktree (the current directory) on your own branch."
                .to_string()
        },
        task = task_block(task),
        contract = run.contract.as_deref().unwrap_or("(none)"),
        design = match shared.design_file {
            Some(path) => format!(
                "The full design is in `{}`; read the parts that concern your task when \
the task and the contract above are not enough.\n",
                path.display()
            ),
            None => format!(
                "Design for context:\n<design>\n{}\n</design>\n",
                run.design_md.as_deref().unwrap_or("")
            ),
        },
        files = files_block(shared.files),
        handoffs = handoffs_block(shared.done),
        guides = shared.guides,
        notes = notes(project),
        economy = ECONOMY,
        handoff_ask = HANDOFF_ASK,
    )
}

fn checks_block(checks: &[CheckResult]) -> String {
    if checks.is_empty() {
        return "(no automated checks configured)".into();
    }
    checks
        .iter()
        .map(|check| {
            let tail: Vec<&str> = check.output.lines().rev().take(30).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            format!(
                "$ {} → {}\n{}",
                check.command,
                if check.ok { "OK" } else { "FAILED" },
                if check.ok {
                    String::new()
                } else {
                    tail.join("\n")
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn fix(review: &Review, checks: &[CheckResult]) -> String {
    let comments = review
        .comments
        .iter()
        .map(|c| {
            format!(
                "- [{}] {}{}: {}",
                c.severity,
                c.file,
                c.line.map(|line| format!(":{line}")).unwrap_or_default(),
                c.issue
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Your work was reviewed and needs changes.
Summary: {summary}
{comments}

Automated checks:
{checks}

Fix every `must` item (and `should` items when reasonable) in the same worktree. Same \
rules as before: no git commit/push. {handoff_ask}",
        summary = review.summary,
        handoff_ask = HANDOFF_ASK,
        checks = checks_block(checks),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn review(
    run: &Run,
    task: &Task,
    diff: &str,
    truncated: bool,
    base_ref: &str,
    checks: &[CheckResult],
    outside_scope: &[String],
    files: &[ContextFile],
    guides: &str,
) -> String {
    let cut = if truncated {
        let place = if is_multi(run) {
            "inside each repository folder"
        } else {
            "in the current directory"
        };
        format!("\n(The diff was cut. Run `git diff {base_ref}...HEAD` {place} to see all of it.)")
    } else {
        String::new()
    };
    let scope = if outside_scope.is_empty() {
        String::new()
    } else {
        format!(
            "\nFiles changed outside the task scope: {}\n",
            outside_scope.join(", ")
        )
    };
    format!(
        "You are the lead reviewer. Review the work of a {role} agent on this task; the \
current directory is its workspace. Do NOT modify any file. Review from the diff \
below; open other files only to settle a specific doubt.
{layout}{guides}{files}{task}

Shared contract:
<contract>
{contract}
</contract>

Automated checks:
{checks}
{scope}
Diff against the integration branch:
```diff
{diff}
```{cut}

Approve only if the task is complete, matches the contract and the acceptance criteria, \
and nothing is broken. Failing checks always mean changes_requested. Mark real problems \
as `must`, nice-to-haves as `should`. Write the summary and issues in the language of the \
task.",
        role = task.role,
        layout = layout(run, "The current directory"),
        task = task_block(task),
        contract = run.contract.as_deref().unwrap_or("(none)"),
        checks = checks_block(checks),
        files = files_block(files),
        guides = guides,
    )
}
