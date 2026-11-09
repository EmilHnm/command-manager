//! The git work of a run, through the `git` CLI: worktrees per task, app
//! commits, diffs for review, merges into the integration branch.

use super::adapter::quiet_command;
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Commits the app makes when the repo has no identity configured.
const FALLBACK_NAME: &str = "Command Manager Agents";
const FALLBACK_EMAIL: &str = "agents@command-manager.local";

async fn output(cwd: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut cmd = quiet_command("git");
    super::adapter::git_config_env(&mut cmd);
    cmd.args(args)
        .current_dir(cwd)
        // The agents' PATH: an app started from the desktop lacks nvm's
        // node, which the repo's own git hooks may run.
        .env("PATH", super::adapter::child_path(Path::new("")))
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|error| Error::msg(format!("Không chạy được git: {error}")))
}

/// Runs git and returns trimmed stdout; a non-zero exit is an error with
/// git's own message.
pub async fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let out = output(cwd, args).await?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let message = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        Err(Error::msg(format!("git {}: {message}", args.join(" "))))
    }
}

pub async fn repo_root(path: &Path) -> Result<String> {
    git(path, &["rev-parse", "--show-toplevel"])
        .await
        .map_err(|_| Error::msg("Thư mục này không phải git repo"))
}

/// The git repository a checkout belongs to: a linked worktree shares its
/// main checkout's.
pub async fn common_dir(root: &Path) -> Result<PathBuf> {
    let path = PathBuf::from(git(root, &["rev-parse", "--git-common-dir"]).await?);
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    Ok(path.canonicalize().unwrap_or(path))
}

/// The checked-out branch, None when HEAD is detached.
pub async fn current_branch(root: &Path) -> Result<Option<String>> {
    let out = output(root, &["symbolic-ref", "--short", "-q", "HEAD"]).await?;
    let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((out.status.success() && !branch.is_empty()).then_some(branch))
}

pub async fn commit_of(root: &Path, rev: &str) -> Result<String> {
    git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ],
    )
    .await
    .map_err(|_| Error::msg(format!("Không tìm thấy {rev}")))
}

/// Tracked files with changes (untracked files do not block a merge).
pub async fn dirty_files(root: &Path) -> Result<Vec<String>> {
    let out = git(root, &["status", "--porcelain", "--untracked-files=no"]).await?;
    Ok(out
        .lines()
        .map(|line| line.get(3..).unwrap_or(line).to_string())
        .collect())
}

/// Rewrites as LF the files git keeps LF but the worktree has CRLF: an
/// agent's own git (agy runs its shell without the app's environment) uses a
/// system `core.autocrlf=true` and checks files out CRLF, which formatters
/// that want LF then reject. Returns the files rewritten.
pub async fn normalize_crlf(root: &Path) -> Result<Vec<String>> {
    let out = output(root, &["ls-files", "--eol", "-z"]).await?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    let mut fixed = Vec::new();
    for entry in out.stdout.split(|byte| *byte == 0) {
        let entry = String::from_utf8_lossy(entry);
        // "i/lf    w/crlf  attr/                 \tpath"
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        let mut fields = meta.split_whitespace();
        if fields.next() != Some("i/lf") || fields.next() != Some("w/crlf") {
            continue;
        }
        let file = root.join(path);
        let Ok(data) = std::fs::read(&file) else {
            continue;
        };
        let mut lf = Vec::with_capacity(data.len());
        let mut bytes = data.iter().peekable();
        while let Some(&byte) = bytes.next() {
            if byte == b'\r' && bytes.peek() == Some(&&b'\n') {
                continue;
            }
            lf.push(byte);
        }
        if std::fs::write(&file, lf).is_ok() {
            fixed.push(path.to_string());
        }
    }
    if !fixed.is_empty() {
        // Rewritten with the same content git has: not a change.
        let _ = output(root, &["update-index", "-q", "--refresh"]).await;
    }
    Ok(fixed)
}

/// Repository settings an agent must leave alone: a worktree shares the
/// main checkout's `.git/config`, so a change there reaches the user's own
/// checkout (line endings, hooks).
const GUARDED_CONFIG: [&str; 3] = ["core.autocrlf", "core.eol", "core.hookspath"];

/// The repository's own (`--local`) values of [`GUARDED_CONFIG`].
pub async fn guarded_config(root: &Path) -> Vec<(&'static str, Option<String>)> {
    let mut values = Vec::new();
    for key in GUARDED_CONFIG {
        let value = output(root, &["config", "--local", "--get", key])
            .await
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string());
        values.push((key, value));
    }
    values
}

/// Puts back what [`guarded_config`] saw; returns the keys that had changed.
pub async fn restore_guarded_config(
    root: &Path,
    before: &[(&'static str, Option<String>)],
) -> Vec<String> {
    let now = guarded_config(root).await;
    let mut changed = Vec::new();
    for ((key, was), (_, is)) in before.iter().zip(now.iter()) {
        if was == is {
            continue;
        }
        let restored = match was {
            Some(value) => git(root, &["config", "--local", key, value]).await,
            None => git(root, &["config", "--local", "--unset-all", key]).await,
        };
        if restored.is_ok() {
            changed.push(format!(
                "{key}: {} → {}",
                is.as_deref().unwrap_or("(không đặt)"),
                was.as_deref().unwrap_or("(không đặt)")
            ));
        }
    }
    changed
}

pub async fn branch_exists(root: &Path, branch: &str) -> bool {
    output(
        root,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .await
    .is_ok_and(|out| out.status.success())
}

/// Creates `branch` at `at` unless it exists (a resumed run).
pub async fn ensure_branch(root: &Path, branch: &str, at: &str) -> Result<()> {
    if !branch_exists(root, branch).await {
        git(root, &["branch", branch, at]).await?;
    }
    Ok(())
}

/// Points `branch` at `at`, creating it if needed. The branch must not be
/// checked out anywhere.
pub async fn reset_branch(root: &Path, branch: &str, at: &str) -> Result<()> {
    let _ = git(root, &["worktree", "prune"]).await;
    git(root, &["branch", "-f", branch, at]).await?;
    Ok(())
}

/// A worktree for `branch` at `path`, reusing one that is already there.
pub async fn ensure_worktree(root: &Path, path: &Path, branch: &str) -> Result<()> {
    if path.join(".git").exists() {
        return Ok(());
    }
    // A folder left by an interrupted add.
    let _ = git(root, &["worktree", "prune"]).await;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    git(root, &["worktree", "add", &path.to_string_lossy(), branch]).await?;
    Ok(())
}

/// A throwaway worktree at `commit`, on no branch.
pub async fn detached_worktree(root: &Path, path: &Path, commit: &str) -> Result<()> {
    remove_worktree(root, path).await;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    git(
        root,
        &[
            "worktree",
            "add",
            "--detach",
            &path.to_string_lossy(),
            commit,
        ],
    )
    .await?;
    Ok(())
}

pub async fn remove_worktree(root: &Path, path: &Path) {
    if path.exists() {
        let _ = git(
            root,
            &["worktree", "remove", "--force", &path.to_string_lossy()],
        )
        .await;
        if path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
    let _ = git(root, &["worktree", "prune"]).await;
}

/// The private git folder of a worktree (`.git/worktrees/<name>` for a
/// linked one): a place for notes that go away with it.
pub async fn git_dir(worktree: &Path) -> Result<PathBuf> {
    Ok(PathBuf::from(
        git(worktree, &["rev-parse", "--absolute-git-dir"]).await?,
    ))
}

/// Puts an existing worktree on `branch`, dropping what it had: tracked
/// changes and untracked files, but not ignored ones (installed
/// dependencies, build caches).
pub async fn switch_worktree(worktree: &Path, branch: &str) -> Result<()> {
    abort_in_progress(worktree).await;
    git(worktree, &["checkout", "-q", "--force", branch]).await?;
    git(worktree, &["clean", "-fdq"]).await?;
    Ok(())
}

/// Moves the branch a worktree has checked out to `at`, the same way.
pub async fn reset_worktree(worktree: &Path, at: &str) -> Result<()> {
    abort_in_progress(worktree).await;
    git(worktree, &["reset", "-q", "--hard", at]).await?;
    git(worktree, &["clean", "-fdq"]).await?;
    Ok(())
}

/// Leaves the worktree's branch (so it can be moved or deleted elsewhere)
/// and keeps the files.
pub async fn detach(worktree: &Path) -> Result<()> {
    git(worktree, &["checkout", "-q", "--detach"]).await?;
    Ok(())
}

/// Aborts a rebase or merge an agent left half done.
pub async fn abort_in_progress(worktree: &Path) {
    let _ = output(worktree, &["rebase", "--abort"]).await;
    let _ = output(worktree, &["merge", "--abort"]).await;
}

/// Files that differ between two commits.
pub async fn changed_between(cwd: &Path, from: &str, to: &str) -> Result<Vec<String>> {
    Ok(git(cwd, &["diff", "--name-only", from, to])
        .await?
        .lines()
        .map(str::to_string)
        .collect())
}

/// Where a repository keeps its commitlint configuration.
const COMMITLINT_FILES: &[&str] = &[
    "commitlint.config.js",
    "commitlint.config.cjs",
    "commitlint.config.mjs",
    "commitlint.config.ts",
    "commitlint.config.cts",
    "commitlint.config.mts",
    ".commitlintrc",
    ".commitlintrc.json",
    ".commitlintrc.yaml",
    ".commitlintrc.yml",
    ".commitlintrc.js",
    ".commitlintrc.cjs",
    ".commitlintrc.mjs",
    ".commitlintrc.ts",
];

/// How a repository writes commit messages: its commitlint configuration
/// and the latest subjects on `base` (the app's own `cm(...)` ones left
/// out). Empty when nothing is known.
pub async fn commit_style(dir: &Path, base: &str) -> String {
    let mut parts = Vec::new();
    let config = COMMITLINT_FILES.iter().find_map(|name| {
        std::fs::read_to_string(dir.join(name))
            .ok()
            .map(|text| (name.to_string(), text))
    });
    let config = config.or_else(|| {
        let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        let section = json.get("commitlint")?;
        Some(("package.json \"commitlint\"".into(), section.to_string()))
    });
    if let Some((name, text)) = config {
        parts.push(format!(
            "commitlint config (`{name}`):\n```\n{}\n```",
            cut(text, 1500).0.trim()
        ));
    }
    let subjects: Vec<String> = git(dir, &["log", "-40", "--no-merges", "--format=%s", base])
        .await
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with("cm(") && !line.starts_with("cm:"))
        .take(15)
        .map(|line| format!("- {line}"))
        .collect();
    if !subjects.is_empty() {
        parts.push(format!(
            "Latest commit subjects on `{base}`:\n{}",
            subjects.join("\n")
        ));
    }
    parts.join("\n\n")
}

/// How a repository names its branches: recent branch names, local and
/// remote, and the branches its merge commits name, plus its commit style.
/// Empty when it has no history (a new repository).
pub async fn branch_style(dir: &Path, base: &str) -> String {
    let mut parts = Vec::new();
    let refs = git(
        dir,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--count=80",
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
        ],
    )
    .await
    .unwrap_or_default();
    let mut names: Vec<String> = Vec::new();
    for name in refs.lines().filter_map(short_ref) {
        let trunk = ["main", "master", "develop", "dev", "HEAD", base];
        if name.starts_with("cm/") || trunk.contains(&name) || names.iter().any(|n| n == name) {
            continue;
        }
        names.push(name.to_string());
        if names.len() == 20 {
            break;
        }
    }
    if !names.is_empty() {
        let list: Vec<String> = names.iter().map(|name| format!("- {name}")).collect();
        parts.push(format!("Recent branches:\n{}", list.join("\n")));
    }
    let merges: Vec<String> = git(dir, &["log", "-40", "--merges", "--format=%s", base])
        .await
        .unwrap_or_default()
        .lines()
        .filter(|line| {
            !line.starts_with("cm:") && (line.contains(" from ") || line.contains("branch"))
        })
        .take(10)
        .map(|line| format!("- {line}"))
        .collect();
    if !merges.is_empty() {
        parts.push(format!("Merge commits on `{base}`:\n{}", merges.join("\n")));
    }
    let commits = commit_style(dir, base).await;
    if !commits.is_empty() {
        parts.push(commits);
    }
    parts.join("\n\n")
}

/// `refs/heads/feat/x` and `refs/remotes/origin/feat/x` → `feat/x`.
fn short_ref(refname: &str) -> Option<&str> {
    if let Some(name) = refname.strip_prefix("refs/heads/") {
        return Some(name);
    }
    let rest = refname.strip_prefix("refs/remotes/")?;
    rest.split_once('/').map(|(_, name)| name)
}

/// Whether `name` is a valid branch name that no local or remote branch
/// uses, nor clashes with: git cannot hold both `feat` and `feat/x`.
pub async fn branch_name_free(dir: &Path, name: &str) -> bool {
    if !output(dir, &["check-ref-format", "--branch", name])
        .await
        .is_ok_and(|out| out.status.success())
    {
        return false;
    }
    let Ok(refs) = git(
        dir,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
        ],
    )
    .await
    else {
        return false;
    };
    !refs.lines().filter_map(short_ref).any(|other| {
        other == name
            || other
                .strip_prefix(name)
                .is_some_and(|rest| rest.starts_with('/'))
            || name
                .strip_prefix(other)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// Runs the repository's commit-msg hook on `message`, as `git commit`
/// would. Err: the hook's output. No hook: Ok.
pub async fn check_commit_msg(dir: &Path, message: &str) -> std::result::Result<(), String> {
    let Ok(rel) = git(dir, &["rev-parse", "--git-path", "hooks/commit-msg"]).await else {
        return Ok(());
    };
    let hook = dir.join(rel);
    if !is_executable(&hook) {
        return Ok(());
    }
    let Ok(git_dir) = git_dir(dir).await else {
        return Ok(());
    };
    let file = git_dir.join("CM_COMMIT_MSG");
    if std::fs::write(&file, format!("{}\n", message.trim())).is_err() {
        return Ok(());
    }
    let mut cmd = quiet_command(super::adapter::sh());
    super::adapter::git_config_env(&mut cmd);
    let run = cmd
        .args(["-c", "\"$0\" \"$1\""])
        .arg(&hook)
        .arg(&file)
        .current_dir(dir)
        .env("PATH", super::adapter::child_path(Path::new("")))
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(std::time::Duration::from_secs(180), run).await;
    let _ = std::fs::remove_file(&file);
    match out {
        Ok(Ok(out)) if out.status.success() => Ok(()),
        Ok(Ok(out)) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            Err(cut(text.trim().to_string(), 2000).0)
        }
        // The hook could not run or hung: not the message's fault.
        _ => Ok(()),
    }
}

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

pub async fn delete_branch(root: &Path, branch: &str) {
    let _ = git(root, &["branch", "-D", branch]).await;
}

async fn has_identity(cwd: &Path) -> bool {
    git(cwd, &["config", "user.email"])
        .await
        .is_ok_and(|email| !email.is_empty())
}

/// `git commit` with the user's identity, or the app's when there is none.
async fn commit(cwd: &Path, args: &[&str]) -> Result<String> {
    if has_identity(cwd).await {
        git(cwd, args).await
    } else {
        let name = format!("user.name={FALLBACK_NAME}");
        let email = format!("user.email={FALLBACK_EMAIL}");
        let mut full = vec!["-c", &name, "-c", &email];
        full.extend_from_slice(args);
        git(cwd, &full).await
    }
}

/// Rebases the worktree's HEAD onto `upstream` (merge commits are
/// flattened away). On conflict the rebase is aborted and the conflicting
/// files are returned.
pub async fn rebase(worktree: &Path, upstream: &str) -> std::result::Result<(), MergeError> {
    match commit(worktree, &["rebase", "--no-verify", "-q", upstream]).await {
        Ok(_) => Ok(()),
        Err(error) => {
            let files: Vec<String> = git(worktree, &["diff", "--name-only", "--diff-filter=U"])
                .await
                .map(|out| out.lines().map(str::to_string).collect())
                .unwrap_or_default();
            let _ = git(worktree, &["rebase", "--abort"]).await;
            Err(if files.is_empty() {
                MergeError::Failed(error.to_string())
            } else {
                MergeError::Conflict(files)
            })
        }
    }
}

/// Starts rebasing the worktree's HEAD onto `upstream` and leaves it where
/// it stops: the files in conflict, none when it went through.
pub async fn rebase_start(worktree: &Path, upstream: &str) -> Result<Vec<String>> {
    let started = commit(worktree, &["rebase", "--no-verify", "-q", upstream]).await;
    rebase_stop(worktree, started.err()).await
}

/// Stages the resolved files and lets the stopped rebase go on, to its
/// next stop (the files in conflict) or its end (none).
pub async fn rebase_continue(worktree: &Path) -> Result<Vec<String>> {
    git(worktree, &["add", "-A"]).await?;
    let mut went = commit(
        worktree,
        &["-c", "core.editor=true", "rebase", "--continue"],
    )
    .await;
    if went.is_err() && rebasing(worktree).await && conflicted(worktree).await.is_empty() {
        // The resolution left the commit empty (the base already had it).
        went = git(worktree, &["rebase", "--skip"]).await;
    }
    rebase_stop(worktree, went.err()).await
}

async fn rebase_stop(worktree: &Path, error: Option<Error>) -> Result<Vec<String>> {
    let files = conflicted(worktree).await;
    match error {
        Some(error) if files.is_empty() => {
            let _ = output(worktree, &["rebase", "--abort"]).await;
            Err(error)
        }
        _ => Ok(files),
    }
}

/// `ancestor` is in `of`'s history.
pub async fn is_ancestor(cwd: &Path, ancestor: &str, of: &str) -> bool {
    output(cwd, &["merge-base", "--is-ancestor", ancestor, of])
        .await
        .is_ok_and(|out| out.status.success())
}

/// A rebase is stopped in the worktree.
pub async fn rebasing(worktree: &Path) -> bool {
    match git_dir(worktree).await {
        Ok(dir) => dir.join("rebase-merge").exists() || dir.join("rebase-apply").exists(),
        Err(_) => false,
    }
}

/// The files git holds in conflict.
pub async fn conflicted(worktree: &Path) -> Vec<String> {
    git(worktree, &["diff", "--name-only", "--diff-filter=U"])
        .await
        .map(|out| out.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// Of `files`, those that still hold conflict markers.
pub fn with_markers(worktree: &Path, files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|file| {
            std::fs::read_to_string(worktree.join(file)).is_ok_and(|text| {
                text.lines()
                    .any(|line| line.starts_with("<<<<<<< ") || line.starts_with(">>>>>>> "))
            })
        })
        .cloned()
        .collect()
}

/// The commit a stopped rebase is applying, and how far along it is.
pub async fn rebase_step(worktree: &Path) -> (String, Option<(u32, u32)>) {
    let commit = git(worktree, &["log", "-1", "--format=%h %s", "REBASE_HEAD"])
        .await
        .unwrap_or_default();
    let progress = match git_dir(worktree).await {
        Ok(dir) => {
            let read = |name: &str| {
                std::fs::read_to_string(dir.join("rebase-merge").join(name))
                    .ok()
                    .and_then(|text| text.trim().parse::<u32>().ok())
            };
            read("msgnum").zip(read("end"))
        }
        Err(_) => None,
    };
    (commit, progress)
}

/// Whether `head` has to be rebased before it may go on top of `base`:
/// `base` is not in its history, or it carries merge commits `base` lacks
/// (repos whose hooks refuse both). False when `base` is unknown.
pub async fn needs_rebase(cwd: &Path, base: &str, head: &str) -> bool {
    if commit_of(cwd, base).await.is_err() {
        return false;
    }
    let on_top = output(cwd, &["merge-base", "--is-ancestor", base, head])
        .await
        .is_ok_and(|out| out.status.success());
    let merges = git(cwd, &["rev-list", "--merges", &format!("{base}..{head}")])
        .await
        .map(|out| !out.trim().is_empty())
        .unwrap_or(false);
    !on_top || merges
}

/// Stages everything and commits. False when there was nothing to commit.
pub async fn commit_all(worktree: &Path, message: &str) -> Result<bool> {
    git(worktree, &["add", "-A"]).await?;
    let staged = output(worktree, &["diff", "--cached", "--quiet"]).await?;
    if staged.status.success() {
        return Ok(false);
    }
    commit(worktree, &["commit", "--no-verify", "-q", "-m", message]).await?;
    Ok(true)
}

/// Files changed on HEAD since it left `base` (three-dot range).
pub async fn changed_files(worktree: &Path, base: &str) -> Result<Vec<String>> {
    let out = git(
        worktree,
        &["diff", "--name-only", &format!("{base}...HEAD")],
    )
    .await?;
    Ok(out.lines().map(str::to_string).collect())
}

/// Lines added and removed on HEAD since it left `base`.
pub async fn numstat(worktree: &Path, base: &str) -> Result<(u64, u64)> {
    let out = git(worktree, &["diff", "--numstat", &format!("{base}...HEAD")]).await?;
    Ok(out.lines().fold((0, 0), |(added, removed), line| {
        let mut parts = line.split('\t');
        // Binary files show "-".
        let a = parts
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0);
        let r = parts
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0);
        (added + a, removed + r)
    }))
}

/// The diff of HEAD since it left `base`, cut at `max_bytes`.
pub async fn diff(worktree: &Path, base: &str, max_bytes: usize) -> Result<(String, bool)> {
    let out = git(
        worktree,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            &format!("{base}...HEAD"),
        ],
    )
    .await?;
    if out.len() <= max_bytes {
        return Ok((out, false));
    }
    let mut cut = max_bytes;
    while !out.is_char_boundary(cut) {
        cut -= 1;
    }
    Ok((out[..cut].to_string(), true))
}

/// Commits on `to` that `from` does not have.
pub async fn ahead(cwd: &Path, from: &str, to: &str) -> Result<u64> {
    let out = git(cwd, &["rev-list", "--count", &format!("{from}..{to}")]).await?;
    Ok(out.trim().parse().unwrap_or(0))
}

/// `git diff` of `range` with paths shown under `prefix` (a repo of a
/// multi-repo project), so diffs of several repos read as one.
pub async fn diff_range(cwd: &Path, range: &str, prefix: &str) -> Result<String> {
    let src = format!("--src-prefix=a/{prefix}");
    let dst = format!("--dst-prefix=b/{prefix}");
    git(
        cwd,
        &["diff", "--no-color", "--no-ext-diff", &src, &dst, range],
    )
    .await
}

/// Lines added and removed in `range`, and the number of files.
pub async fn numstat_range(cwd: &Path, range: &str) -> Result<(u64, u64, u64)> {
    let out = git(cwd, &["diff", "--numstat", range]).await?;
    Ok(out
        .lines()
        .fold((0, 0, 0), |(files, added, removed), line| {
            let mut parts = line.split('\t');
            let a = parts
                .next()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0);
            let r = parts
                .next()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0);
            (files + 1, added + a, removed + r)
        }))
}

/// Cuts `text` to at most `max_bytes` on a character boundary.
pub fn cut(text: String, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text, false);
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_string(), true)
}

/// `git merge --no-ff` in `cwd`. On a conflict the merge is aborted and the
/// conflicting files are returned as the error.
pub async fn merge(cwd: &Path, branch: &str, message: &str) -> std::result::Result<(), MergeError> {
    match commit(
        cwd,
        // The app's own merge commit, like its other commits: the repo's
        // commit hooks (commitlint on `cm: merge ...`) are not for it; the
        // pre-push hook checks what goes out.
        &[
            "merge",
            "--no-ff",
            "--no-edit",
            "--no-verify",
            "-m",
            message,
            branch,
        ],
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(error) => {
            let conflicts = git(cwd, &["diff", "--name-only", "--diff-filter=U"])
                .await
                .unwrap_or_default();
            let _ = git(cwd, &["merge", "--abort"]).await;
            if conflicts.trim().is_empty() {
                Err(MergeError::Failed(error.to_string()))
            } else {
                Err(MergeError::Conflict(
                    conflicts.lines().map(str::to_string).collect(),
                ))
            }
        }
    }
}

/// Brings `branch` into `cwd` as one commit (`git merge --squash`), with
/// `message`. Nothing new: no commit. On a conflict the worktree is put back
/// and the conflicting files are returned as the error.
pub async fn squash_merge(
    cwd: &Path,
    branch: &str,
    message: &str,
) -> std::result::Result<(), MergeError> {
    if let Err(error) = git(cwd, &["merge", "--squash", "-q", branch]).await {
        let conflicts = conflicted(cwd).await;
        let _ = git(cwd, &["reset", "-q", "--merge"]).await;
        return Err(if conflicts.is_empty() {
            MergeError::Failed(error.to_string())
        } else {
            MergeError::Conflict(conflicts)
        });
    }
    let staged = output(cwd, &["diff", "--cached", "--quiet"])
        .await
        .map_err(|error| MergeError::Failed(error.to_string()))?;
    if staged.status.success() {
        return Ok(());
    }
    // The app's commit: the repo's commit hooks are not for it; the
    // pre-push hook checks what goes out.
    commit(cwd, &["commit", "-q", "--no-verify", "-m", message])
        .await
        .map(|_| ())
        .map_err(|error| MergeError::Failed(error.to_string()))
}

/// The commit messages of `from..to`, oldest first.
pub async fn messages(cwd: &Path, from: &str, to: &str) -> Vec<String> {
    git(
        cwd,
        &[
            "log",
            "--reverse",
            "--format=%B%x00",
            &format!("{from}..{to}"),
        ],
    )
    .await
    .map(|out| {
        out.split('\0')
            .map(|message| message.trim().to_string())
            .filter(|message| !message.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

#[derive(Debug)]
pub enum MergeError {
    Conflict(Vec<String>),
    Failed(String),
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict(files) => write!(f, "Xung đột merge: {}", files.join(", ")),
            Self::Failed(message) => write!(f, "{message}"),
        }
    }
}

/// Simple glob for task scopes: `**` any depth, `*` within a segment, `?`
/// one character. A pattern ending in `/` matches everything below it.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim().trim_start_matches("./");
    let pattern = if pattern.ends_with('/') {
        format!("{pattern}**")
    } else {
        pattern.to_string()
    };
    fn matches(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (None, Some(_)) => false,
            (Some(b'*'), _) if p.get(1) == Some(&b'*') => {
                // `**/` may also match nothing.
                let rest = if p.get(2) == Some(&b'/') {
                    &p[3..]
                } else {
                    &p[2..]
                };
                (0..=s.len()).any(|i| matches(rest, &s[i..]))
            }
            (Some(b'*'), _) => {
                let mut i = 0;
                loop {
                    if matches(&p[1..], &s[i..]) {
                        return true;
                    }
                    if i == s.len() || s[i] == b'/' {
                        return false;
                    }
                    i += 1;
                }
            }
            (Some(b'?'), Some(c)) if *c != b'/' => matches(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a == b => matches(&p[1..], &s[1..]),
            _ => false,
        }
    }
    matches(pattern.as_bytes(), path.as_bytes())
}

/// Changed files no scope pattern covers. No scope = nothing is outside.
pub fn outside_scope(scope: &[String], files: &[String]) -> Vec<String> {
    if scope.is_empty() {
        return Vec::new();
    }
    files
        .iter()
        .filter(|file| !scope.iter().any(|pattern| glob_match(pattern, file)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn branch_names_are_free_unless_taken_or_clashing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]).await.unwrap();
        std::fs::write(root.join("a"), "a").unwrap();
        commit_all(root, "feat: first").await.unwrap();
        git(root, &["branch", "feat/login"]).await.unwrap();
        git(root, &["branch", "fix"]).await.unwrap();
        git(
            root,
            &["update-ref", "refs/remotes/origin/feat/remote-one", "HEAD"],
        )
        .await
        .unwrap();

        assert!(branch_name_free(root, "feat/cart").await);
        assert!(!branch_name_free(root, "feat/login").await);
        assert!(
            !branch_name_free(root, "feat").await,
            "feat/login is under it"
        );
        assert!(
            !branch_name_free(root, "fix/typo").await,
            "the branch fix is in the way"
        );
        assert!(!branch_name_free(root, "feat/remote-one").await);
        assert!(!branch_name_free(root, "feat/bad..name").await);

        let style = branch_style(root, "main").await;
        assert!(style.contains("- feat/login"), "{style}");
        assert!(style.contains("- feat/remote-one"), "{style}");
        assert!(style.contains("- feat: first"), "{style}");
        assert!(!style.contains("- main"), "{style}");
    }

    #[test]
    fn scope_globs() {
        assert!(glob_match("src/**", "src/a/b.rs"));
        assert!(glob_match("src/**/*.rs", "src/a.rs"));
        assert!(glob_match("src/**/*.rs", "src/x/y/a.rs"));
        assert!(!glob_match("src/*.rs", "src/x/a.rs"));
        assert!(glob_match("src/", "src/x/a.rs"));
        assert!(glob_match("Cargo.toml", "Cargo.toml"));
        assert!(!glob_match("src/**", "srcx/a.rs"));
        assert_eq!(
            outside_scope(&["src/**".into()], &["src/a.rs".into(), "main.rs".into()]),
            vec!["main.rs"]
        );
        assert!(outside_scope(&[], &["x".into()]).is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn worktree_commit_diff_and_merge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]).await.unwrap();
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        assert!(commit_all(&root, "init").await.unwrap());
        let base = commit_of(&root, "HEAD").await.unwrap();
        assert_eq!(
            current_branch(&root).await.unwrap().as_deref(),
            Some("main")
        );

        ensure_branch(&root, "cm/r/integration", &base)
            .await
            .unwrap();
        ensure_branch(&root, "cm/r/t1", &base).await.unwrap();
        let integration = dir.path().join("wt/_integration");
        let task = dir.path().join("wt/t1");
        ensure_worktree(&root, &integration, "cm/r/integration")
            .await
            .unwrap();
        ensure_worktree(&root, &task, "cm/r/t1").await.unwrap();
        // Reusing an existing worktree is fine.
        ensure_worktree(&root, &task, "cm/r/t1").await.unwrap();

        assert!(!commit_all(&task, "nothing").await.unwrap());
        std::fs::write(task.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(task.join("b.txt"), "new\n").unwrap();
        assert!(commit_all(&task, "cm(t1): work").await.unwrap());
        assert_eq!(
            changed_files(&task, "cm/r/integration").await.unwrap(),
            vec!["a.txt", "b.txt"]
        );
        assert_eq!(numstat(&task, "cm/r/integration").await.unwrap(), (2, 0));
        let (text, cut) = diff(&task, "cm/r/integration", 40).await.unwrap();
        assert!(cut && text.len() <= 40);

        merge(&integration, "cm/r/t1", "cm: merge t1")
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(integration.join("b.txt")).unwrap(),
            "new\n"
        );

        // A second task that edits the same line conflicts.
        ensure_branch(&root, "cm/r/t2", &base).await.unwrap();
        let task2 = dir.path().join("wt/t2");
        ensure_worktree(&root, &task2, "cm/r/t2").await.unwrap();
        std::fs::write(task2.join("a.txt"), "uno\n").unwrap();
        commit_all(&task2, "cm(t2)").await.unwrap();
        match merge(&integration, "cm/r/t2", "cm: merge t2").await {
            Err(MergeError::Conflict(files)) => assert_eq!(files, vec!["a.txt"]),
            other => panic!("expected a conflict, got {other:?}"),
        }
        // Aborted: the integration worktree is clean again.
        assert!(dirty_files(&integration).await.unwrap().is_empty());

        remove_worktree(&root, &task).await;
        delete_branch(&root, "cm/r/t1").await;
        assert!(!task.exists());
        assert!(!branch_exists(&root, "cm/r/t1").await);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn crlf_left_by_an_agent_goes_back_to_lf_and_config_is_put_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]).await.unwrap();
        std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(root.join("b.txt"), "keep\n").unwrap();
        assert!(commit_all(&root, "init").await.unwrap());

        // What an agent's own git with autocrlf=true leaves.
        std::fs::write(root.join("a.txt"), "one\r\ntwo\r\n").unwrap();
        assert_eq!(normalize_crlf(&root).await.unwrap(), vec!["a.txt"]);
        assert_eq!(std::fs::read(root.join("a.txt")).unwrap(), b"one\ntwo\n");
        assert!(dirty_files(&root).await.unwrap().is_empty());
        assert!(normalize_crlf(&root).await.unwrap().is_empty());

        let before = guarded_config(&root).await;
        git(&root, &["config", "core.autocrlf", "false"])
            .await
            .unwrap();
        git(&root, &["config", "core.eol", "lf"]).await.unwrap();
        let changed = restore_guarded_config(&root, &before).await;
        assert_eq!(changed.len(), 2, "{changed:?}");
        assert!(output(&root, &["config", "--local", "--get", "core.eol"])
            .await
            .map(|out| !out.status.success())
            .unwrap());
        assert!(restore_guarded_config(&root, &before).await.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn worktrees_stay_lf_when_the_repo_asks_for_crlf() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]).await.unwrap();
        // What Git for Windows sets system-wide.
        git(&root, &["config", "core.autocrlf", "true"])
            .await
            .unwrap();
        std::fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
        assert!(commit_all(&root, "init").await.unwrap());
        let base = commit_of(&root, "HEAD").await.unwrap();
        ensure_branch(&root, "cm/r/t1", &base).await.unwrap();
        let task = dir.path().join("wt/t1");
        ensure_worktree(&root, &task, "cm/r/t1").await.unwrap();
        assert_eq!(std::fs::read(task.join("a.txt")).unwrap(), b"one\ntwo\n");

        // An agent writing CRLF still commits LF.
        std::fs::write(task.join("b.txt"), "x\r\ny\r\n").unwrap();
        assert!(commit_all(&task, "cm(t1): crlf").await.unwrap());
        let blob = git(&task, &["show", "HEAD:b.txt"]).await.unwrap();
        assert!(!blob.contains('\r'));
        assert!(dirty_files(&task).await.unwrap().is_empty());
        // The repo's own config is left as it was.
        assert_eq!(
            git(&root, &["config", "--local", "core.autocrlf"])
                .await
                .unwrap(),
            "true"
        );
    }
}
