//! A project folder and its repos: one repo at the root, or a folder of
//! several (fe/, be/, docs/...). Task and integration workspaces mirror the
//! folder: a worktree per repo at the same relative path, so agents see the
//! same layout as the user.

use super::git;
use crate::db::repos::agents::ROOT_REPO;
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Repos deeper than this below the folder are not looked for.
const MAX_DEPTH: usize = 2;
const MAX_REPOS: usize = 30;
/// Folders that never hold a project repo and can be huge.
const SKIP: &[&str] = &[
    "node_modules",
    "target",
    "vendor",
    "dist",
    "build",
    "__pycache__",
];

/// The project folder for `path` and the repos in it. A path inside a repo
/// is that repo (its root); any other folder is searched for repos.
pub async fn detect(path: &Path) -> Result<(PathBuf, Vec<String>)> {
    if !path.is_dir() {
        return Err(Error::msg("Thư mục không tồn tại"));
    }
    if let Ok(root) = git::repo_root(path).await {
        return Ok((PathBuf::from(root), vec![ROOT_REPO.to_string()]));
    }
    let mut found = Vec::new();
    scan(path, Path::new(""), 1, &mut found)?;
    if found.is_empty() {
        return Err(Error::msg(
            "Thư mục không phải git repo và không chứa git repo nào (tìm tới 2 cấp thư mục con)",
        ));
    }
    found.sort();
    Ok((path.to_path_buf(), distinct(path, found).await))
}

/// Drops checkouts that are linked worktrees of another repo in the list
/// (`wt-x/` made by `git worktree add` from `fe/`): one git repository can
/// check a branch out only once, and a run makes its branches in every repo.
/// Main checkouts win; the order is kept.
pub async fn distinct(base: &Path, repos: Vec<String>) -> Vec<String> {
    let mut entries = Vec::new();
    for repo in repos {
        let dir = repo_dir(base, &repo);
        let linked = dir.join(".git").is_file();
        let common = git::common_dir(&dir).await.ok();
        entries.push((repo, linked, common));
    }
    let mut owners: Vec<(PathBuf, String)> = Vec::new();
    for linked in [false, true] {
        for (repo, _, common) in entries.iter().filter(|entry| entry.1 == linked) {
            if let Some(common) = common {
                if !owners.iter().any(|(dir, _)| dir == common) {
                    owners.push((common.clone(), repo.clone()));
                }
            }
        }
    }
    entries
        .into_iter()
        .filter(|(repo, _, common)| match common {
            Some(common) => owners
                .iter()
                .any(|(dir, owner)| dir == common && owner == repo),
            // Not readable as a repo: left for the caller to report.
            None => true,
        })
        .map(|(repo, _, _)| repo)
        .collect()
}

fn scan(dir: &Path, rel: &Path, depth: usize, found: &mut Vec<String>) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    let mut children: Vec<_> = entries.flatten().collect();
    children.sort_by_key(|entry| entry.file_name());
    for entry in children {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || SKIP.contains(&name.as_str()) {
            continue;
        }
        // Symlinked folders could loop or point outside the project.
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let path = entry.path();
        let rel = rel.join(&name);
        if path.join(".git").exists() {
            if found.len() >= MAX_REPOS {
                return Err(Error::msg(format!(
                    "Thư mục chứa quá nhiều repo (tối đa {MAX_REPOS})"
                )));
            }
            found.push(rel.to_string_lossy().replace('\\', "/"));
        } else if depth < MAX_DEPTH {
            scan(&path, &rel, depth + 1, found)?;
        }
    }
    Ok(())
}

/// Where `repo` lives under a project folder or a workspace.
pub fn repo_dir(base: &Path, repo: &str) -> PathBuf {
    if repo == ROOT_REPO {
        base.to_path_buf()
    } else {
        base.join(repo)
    }
}

/// A repo-relative path as seen from the project folder.
pub fn project_path(repo: &str, file: &str) -> String {
    if repo == ROOT_REPO {
        file.to_string()
    } else {
        format!("{repo}/{file}")
    }
}

/// A label for check results and logs ("" for a single repo).
pub fn label(repo: &str) -> String {
    if repo == ROOT_REPO {
        String::new()
    } else {
        format!("[{repo}] ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn repos_are_found_in_a_folder_or_it_is_one() {
        let dir = tempfile::tempdir().unwrap();
        let cluster = dir.path().join("shop");
        for repo in ["be", "fe", "apps/admin"] {
            let path = cluster.join(repo);
            std::fs::create_dir_all(&path).unwrap();
            git::git(&path, &["init", "-q"]).await.unwrap();
        }
        // Not repos: too deep, skipped or hidden.
        for other in ["libs/a/b", "node_modules/x", ".cache/y"] {
            std::fs::create_dir_all(cluster.join(other).join(".git")).unwrap();
        }
        let (root, repos) = detect(&cluster).await.unwrap();
        assert_eq!(root, cluster);
        assert_eq!(repos, vec!["apps/admin", "be", "fe"]);

        // A folder inside a repo is that repo.
        std::fs::create_dir_all(cluster.join("be/src")).unwrap();
        let (root, repos) = detect(&cluster.join("be/src")).await.unwrap();
        assert_eq!(
            root.canonicalize().unwrap(),
            cluster.join("be").canonicalize().unwrap()
        );
        assert_eq!(repos, vec!["."]);

        let empty = dir.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        assert!(detect(&empty).await.is_err());
        assert_eq!(repo_dir(Path::new("/w"), "."), PathBuf::from("/w"));
        assert_eq!(repo_dir(Path::new("/w"), "fe"), PathBuf::from("/w/fe"));
        assert_eq!(project_path("fe", "src/a.ts"), "fe/src/a.ts");
        assert_eq!(project_path(".", "src/a.ts"), "src/a.ts");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_linked_worktree_is_not_a_repo_of_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let cluster = dir.path().join("health");
        let fe = cluster.join("fe");
        std::fs::create_dir_all(&fe).unwrap();
        git::git(&fe, &["init", "-q", "-b", "main"]).await.unwrap();
        std::fs::write(fe.join("a"), "a").unwrap();
        git::commit_all(&fe, "init").await.unwrap();
        git::git(&fe, &["worktree", "add", "-q", "-b", "q011", "../wt-q011"])
            .await
            .unwrap();
        // Its main checkout elsewhere: then the worktree is the only copy.
        let other = dir.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        git::git(&other, &["init", "-q", "-b", "main"])
            .await
            .unwrap();
        std::fs::write(other.join("b"), "b").unwrap();
        git::commit_all(&other, "init").await.unwrap();
        git::git(
            &other,
            &["worktree", "add", "-q", "-b", "x", "../health/x-wt"],
        )
        .await
        .unwrap();

        let (_, repos) = detect(&cluster).await.unwrap();
        assert_eq!(repos, vec!["fe", "x-wt"]);
        // Order is kept and the main checkout wins even when listed after.
        let kept = distinct(&cluster, vec!["wt-q011".into(), "fe".into()]).await;
        assert_eq!(kept, vec!["fe"]);
    }
}
