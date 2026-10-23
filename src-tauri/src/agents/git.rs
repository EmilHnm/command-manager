//! The git work of a run, through the `git` CLI: worktrees per task, app
//! commits, diffs for review, merges into the integration branch.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Commits the app makes when the repo has no identity configured.
const FALLBACK_NAME: &str = "Command Manager Agents";
const FALLBACK_EMAIL: &str = "agents@command-manager.local";

async fn output(cwd: &Path, args: &[&str]) -> Result<std::process::Output> {
    Command::new("git")
        .args(args)
        .current_dir(cwd)
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
        &["merge", "--no-ff", "--no-edit", "-m", message, branch],
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
}
