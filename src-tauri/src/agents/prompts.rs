//! What each step asks of its CLI, and the schemas of structured answers.
//!
//! Prompts are in English (every CLI follows English instructions best);
//! the agents are told to write for the user in the language of the
//! request. Schemas are "strict" (every property required, no extras) so
//! `codex --output-schema` accepts them too.

use crate::db::repos::agents::{CheckResult, Project, Review, Run, Task, ROOT_REPO};
use serde_json::{json, Value};

/// A review prompt carries at most this much diff; the reviewer can run
/// `git diff` itself for the rest.
pub const REVIEW_DIFF_BYTES: usize = 60 * 1024;

fn notes(project: &Project) -> String {
    match project.notes.as_deref() {
        Some(notes) => format!("\nProject notes from the user:\n{notes}\n"),
        None => String::new(),
    }
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

pub fn design(run: &Run, project: &Project) -> String {
    format!(
        "You are the lead architect of a small team of AI coding agents working on the \
code in the current directory. Read the code you need; do NOT modify any file.
{layout}
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

pub fn plan(run: &Run) -> String {
    format!(
        "Now split the work into tasks for the team, following your design:
<design>
{design}
</design>

Rules:
- Roles available: {roles}. Every task has exactly one of them.
- `key`: short kebab-case id, unique. `depends_on`: keys that must be merged first. \
Prefer few dependencies so work can go in parallel; a frontend task can start from the \
contract without waiting for the backend.
- `repos`: the repositories the task changes ({repos}). Prefer one repository per \
task; split cross-repository work into tasks linked by `depends_on` or by the contract.
- `scope`: glob patterns of files the task may change, relative to the current \
directory (e.g. `{scope_example}`). Tasks running in parallel should not share files.
- `acceptance`: checkable criteria. Each task must be completable in one focused session.
- `contract`: the shared contract (types, signatures) from the design, verbatim, so \
every worker gets it.
Write titles and descriptions in the language of the request.",
        design = run.design_md.as_deref().unwrap_or(""),
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
    )
}

pub fn plan_schema(roles: &[String], repos: &[String]) -> Value {
    let strings = json!({ "type": "array", "items": { "type": "string" } });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["contract", "tasks"],
        "properties": {
            "contract": { "type": "string" },
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

pub fn implement(run: &Run, project: &Project, task: &Task, earlier_work: bool) -> String {
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

Design for context:
<design>
{design}
</design>
{notes}{earlier}
Rules:
- Implement the task completely, including tests where the project has them.
- Stay within the scope; if you must touch another file, keep the change minimal.
- Do NOT run git commit, git push or change branches: the orchestrator commits for you.
- Do not ask questions; make reasonable decisions and mention them at the end.
Finish with a short summary of what you changed.",
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
        design = run.design_md.as_deref().unwrap_or(""),
        notes = notes(project),
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
rules as before: no git commit/push. Finish with a short summary of what you changed.",
        summary = review.summary,
        checks = checks_block(checks),
    )
}

pub fn review(
    run: &Run,
    task: &Task,
    diff: &str,
    truncated: bool,
    base_ref: &str,
    checks: &[CheckResult],
    outside_scope: &[String],
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
current directory is its workspace. Do NOT modify any file.
{layout}{task}

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
    )
}
