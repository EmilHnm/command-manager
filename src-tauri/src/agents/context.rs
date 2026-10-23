//! What the app prepares so agents do not dig for it themselves (every
//! `ls`, `grep` and `gh` an agent runs costs tokens, and its output stays in
//! the context for every later turn):
//! - a map of the repos, from `git ls-files`, for the design step;
//! - pull requests linked in the request, fetched once with `gh`;
//! - the design as a file, so workers read it instead of carrying it in
//!   every prompt.
//!
//! Files live in `<run dir>/context`, outside every worktree.

use super::adapter::{child_path, search_path};
use super::git;
use super::log::AgentLog;
use super::workspace;
use crate::db::repos::agents::RunRepo;
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

pub const DIR: &str = "context";
pub const DESIGN_FILE: &str = "design.md";

/// Folders listed per repo in the map, at most.
const MAP_DIRS: usize = 60;
/// Files at the top of a repo listed by name, at most.
const MAP_TOP_FILES: usize = 25;
const GH_TIMEOUT: Duration = Duration::from_secs(90);

/// A prepared file and what it holds, as the prompts describe it.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextFile {
    pub what: String,
    pub path: PathBuf,
}

/// The map of the repos: tracked folders with file counts and top-level
/// files (rules and skills are listed by `guides`).
pub async fn overview(project: &Path, repos: &[RunRepo]) -> String {
    let mut out = String::new();
    for repo in repos {
        let dir = workspace::repo_dir(project, &repo.path);
        let Ok(listing) = git::git(&dir, &["ls-files"]).await else {
            continue;
        };
        let files: Vec<&str> = listing.lines().filter(|line| !line.is_empty()).collect();
        out.push_str(&repo_map(&repo.path, &files));
    }
    out
}

fn repo_map(repo: &str, files: &[&str]) -> String {
    let prefix = workspace::project_path(repo, "");
    let mut dirs: BTreeMap<String, usize> = BTreeMap::new();
    let mut top = Vec::new();
    for file in files {
        let parts: Vec<&str> = file.split('/').collect();
        if parts.len() == 1 {
            top.push(*file);
        }
        for depth in 1..parts.len().min(3) {
            *dirs.entry(parts[..depth].join("/")).or_default() += 1;
        }
    }
    let mut out = format!(
        "Repository `{}` ({} tracked files):\n",
        if prefix.is_empty() { "." } else { &prefix },
        files.len()
    );
    for (dir, count) in dirs.iter().take(MAP_DIRS) {
        out.push_str(&format!("  {prefix}{dir}/ ({count})\n"));
    }
    if dirs.len() > MAP_DIRS {
        out.push_str(&format!("  … {} more folders\n", dirs.len() - MAP_DIRS));
    }
    if !top.is_empty() {
        let shown: Vec<&str> = top.iter().take(MAP_TOP_FILES).copied().collect();
        out.push_str(&format!(
            "  top-level files: {}{}\n",
            shown.join(", "),
            if top.len() > MAP_TOP_FILES {
                ", …"
            } else {
                ""
            }
        ));
    }
    out
}

/// A pull request the request links to.
#[derive(Debug, Clone, PartialEq)]
pub struct PrLink {
    pub url: String,
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl PrLink {
    fn stem(&self) -> String {
        format!("pr-{}-{}-{}", self.owner, self.repo, self.number)
    }
}

pub fn pr_links(text: &str) -> Vec<PrLink> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r"https?://github\.com/([\w.-]+)/([\w.-]+)/pull/(\d+)").expect("PR pattern")
    });
    let mut links: Vec<PrLink> = Vec::new();
    for found in pattern.captures_iter(text) {
        let link = PrLink {
            url: format!(
                "https://github.com/{}/{}/pull/{}",
                &found[1], &found[2], &found[3]
            ),
            owner: found[1].to_string(),
            repo: found[2].to_string(),
            number: found[3].parse().unwrap_or(0),
        };
        if !links.contains(&link) {
            links.push(link);
        }
    }
    links
}

/// Fetches the linked pull requests not fetched yet (`refresh`: again, for
/// review comments made since). Failures are logged, never fatal: the agent
/// can still fetch them itself.
pub async fn fetch_prs(dir: &Path, request: &str, log: &AgentLog, refresh: bool) {
    let links = pr_links(request);
    if links.is_empty() {
        return;
    }
    let Some(gh) = find_gh() else {
        log.info("Không tìm thấy gh: agent sẽ tự đọc pull request");
        return;
    };
    let _ = std::fs::create_dir_all(dir);
    for link in links {
        let summary = dir.join(format!("{}.md", link.stem()));
        let diff = dir.join(format!("{}.diff", link.stem()));
        if summary.exists() && diff.exists() && !refresh {
            continue;
        }
        log.heading(&format!("gh pr view {}", link.url));
        let view = gh_output(
            &gh,
            &[
                "pr",
                "view",
                &link.url,
                "--json",
                "number,title,body,state,baseRefName,headRefName,files,reviews,comments",
            ],
        )
        .await;
        let text = match view.map(|out| serde_json::from_str::<Value>(&out)) {
            Ok(Ok(value)) => {
                // Line comments of reviews are only in the REST API.
                let inline = gh_output(
                    &gh,
                    &[
                        "api",
                        "--paginate",
                        &format!(
                            "repos/{}/{}/pulls/{}/comments",
                            link.owner, link.repo, link.number
                        ),
                        "--jq",
                        r#".[] | "- `\(.path):\(.line // .original_line // "?")` \(.user.login): \(.body | gsub("\r?\n"; " "))""#,
                    ],
                )
                .await
                .unwrap_or_default();
                pr_summary(&link, &value, &inline)
            }
            Ok(Err(error)) => {
                log.result(false, &format!("gh trả về dữ liệu lạ: {error}"));
                continue;
            }
            Err(error) => {
                log.result(false, &error);
                continue;
            }
        };
        match gh_output(&gh, &["pr", "diff", &link.url]).await {
            Ok(patch) => {
                let saved = std::fs::write(&summary, text).and(std::fs::write(&diff, &patch));
                match saved {
                    Ok(()) => log.result(
                        true,
                        &format!(
                            "Đã lưu PR #{} ({} KiB diff)",
                            link.number,
                            patch.len() / 1024
                        ),
                    ),
                    Err(error) => log.result(false, &error.to_string()),
                }
            }
            Err(error) => log.result(false, &error),
        }
    }
}

pub fn find_gh() -> Option<PathBuf> {
    let name = if cfg!(windows) { "gh.exe" } else { "gh" };
    search_path()
        .into_iter()
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

pub async fn gh_output(gh: &Path, args: &[&str]) -> Result<String, String> {
    let run = tokio::process::Command::new(gh)
        .args(args)
        .env("PATH", child_path(gh))
        .env("NO_COLOR", "1")
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(GH_TIMEOUT, run).await {
        Err(_) => Err("gh chạy quá lâu".into()),
        Ok(Err(error)) => Err(format!("Không chạy được gh: {error}")),
        Ok(Ok(out)) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).into()),
        Ok(Ok(out)) => Err(format!(
            "gh lỗi: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// One line per comment or review with a body: `author (state): body`.
fn discussion(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let body = item.get("body").and_then(Value::as_str)?.trim();
                    if body.is_empty() {
                        return None;
                    }
                    let author = item
                        .pointer("/author/login")
                        .and_then(Value::as_str)
                        .unwrap_or("?");
                    let state = item
                        .get("state")
                        .and_then(Value::as_str)
                        .map(|state| format!(" ({state})"))
                        .unwrap_or_default();
                    Some(format!("- {author}{state}: {}", body.replace('\n', "\n  ")))
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn pr_summary(link: &PrLink, value: &Value, inline: &str) -> String {
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or("");
    let files = value
        .get("files")
        .and_then(Value::as_array)
        .map(|files| {
            files
                .iter()
                .map(|file| {
                    format!(
                        "- {} (+{} -{})",
                        file.get("path").and_then(Value::as_str).unwrap_or("?"),
                        file.get("additions").and_then(Value::as_u64).unwrap_or(0),
                        file.get("deletions").and_then(Value::as_u64).unwrap_or(0)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let mut review = String::new();
    for (title, text) in [
        ("Reviews", discussion(value, "reviews")),
        ("Line comments", inline.trim().to_string()),
        ("Conversation", discussion(value, "comments")),
    ] {
        if !text.is_empty() {
            review.push_str(&format!("\n## {title}\n\n{text}\n"));
        }
    }
    format!(
        "# {}/{} PR #{}: {}\n\n{} · {} ← {}\n{}\n\n## Description\n\n{}\n\n## Changed files\n\n{}\n{review}",
        link.owner,
        link.repo,
        link.number,
        text("title"),
        text("state"),
        text("baseRefName"),
        text("headRefName"),
        link.url,
        text("body"),
        files
    )
}

/// A review comment on GitHub that still waits for the author.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PrComment {
    pub author: String,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub body: String,
    pub url: Option<String>,
}

/// What GitHub says about a pull request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrState {
    pub state: String,
    pub review_decision: String,
    pub head: String,
    pub base: String,
    /// Paths relative to the repository.
    pub files: Vec<String>,
    /// Review threads not resolved whose last word is not the author's,
    /// and the reviews that asked for changes.
    pub open_comments: Vec<PrComment>,
}

const THREADS_QUERY: &str = "query($owner:String!,$name:String!,$number:Int!){repository(\
owner:$owner,name:$name){pullRequest(number:$number){author{login} reviewThreads(first:100)\
{nodes{isResolved comments(first:50){nodes{author{login} body path line originalLine url}}}}}}}";

/// Reads a pull request with gh: its files, state and the review comments
/// nobody answered yet.
pub async fn pr_state(gh: &Path, link: &PrLink) -> Result<PrState, String> {
    let view = gh_output(
        gh,
        &[
            "pr",
            "view",
            &link.url,
            "--json",
            "state,reviewDecision,headRefName,baseRefName,files,author,reviews",
        ],
    )
    .await?;
    let view: Value = serde_json::from_str(&view).map_err(|error| error.to_string())?;
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let author = view
        .pointer("/author/login")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let files = view
        .get("files")
        .and_then(Value::as_array)
        .map(|files| files.iter().map(|file| text(file, "path")).collect())
        .unwrap_or_default();
    let mut open_comments = Vec::new();
    // The latest review of each reviewer that asks for changes.
    if let Some(reviews) = view.get("reviews").and_then(Value::as_array) {
        let mut latest: Vec<&Value> = Vec::new();
        for review in reviews {
            let who = review.pointer("/author/login");
            latest.retain(|known| known.pointer("/author/login") != who);
            latest.push(review);
        }
        for review in latest {
            let body = text(review, "body");
            if text(review, "state") == "CHANGES_REQUESTED" && !body.trim().is_empty() {
                open_comments.push(PrComment {
                    author: review
                        .pointer("/author/login")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                        .to_string(),
                    body,
                    ..Default::default()
                });
            }
        }
    }
    let threads = gh_output(
        gh,
        &[
            "api",
            "graphql",
            "-f",
            &format!("query={THREADS_QUERY}"),
            "-F",
            &format!("owner={}", link.owner),
            "-F",
            &format!("name={}", link.repo),
            "-F",
            &format!("number={}", link.number),
        ],
    )
    .await?;
    let threads: Value = serde_json::from_str(&threads).map_err(|error| error.to_string())?;
    let nodes = threads
        .pointer("/data/repository/pullRequest/reviewThreads/nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for thread in nodes {
        if thread.get("isResolved").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let comments = thread
            .pointer("/comments/nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let (Some(first), Some(last)) = (comments.first(), comments.last()) else {
            continue;
        };
        let last_author = last.pointer("/author/login").and_then(Value::as_str);
        if last_author == Some(author.as_str()) {
            // The author answered; the reviewer has not come back yet.
            continue;
        }
        let body = comments
            .iter()
            .map(|comment| {
                format!(
                    "{}: {}",
                    comment
                        .pointer("/author/login")
                        .and_then(Value::as_str)
                        .unwrap_or("?"),
                    text(comment, "body").trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        open_comments.push(PrComment {
            author: last_author.unwrap_or("?").to_string(),
            path: first
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string),
            line: first
                .get("line")
                .and_then(Value::as_u64)
                .or_else(|| first.get("originalLine").and_then(Value::as_u64)),
            body,
            url: first.get("url").and_then(Value::as_str).map(str::to_string),
        });
    }
    Ok(PrState {
        state: text(&view, "state"),
        review_decision: text(&view, "reviewDecision"),
        head: text(&view, "headRefName"),
        base: text(&view, "baseRefName"),
        files,
        open_comments,
    })
}

/// Writes the current design for workers to read.
pub fn write_design(dir: &Path, design: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(DESIGN_FILE);
    std::fs::write(&path, design)?;
    Ok(path)
}

/// The prepared files besides the design, for the prompts.
pub fn files(dir: &Path) -> Vec<ContextFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<ContextFile> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            let what = if !name.starts_with("pr-") {
                return None;
            } else if name.ends_with(".diff") {
                "full diff of a pull request linked in the request"
            } else if name.ends_with(".md") {
                "description, changed files and review comments of a pull request linked in the request"
            } else {
                return None;
            };
            Some(ContextFile {
                what: what.into(),
                path,
            })
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_each_linked_pull_request_once() {
        let links = pr_links(
            "Kiểm tra PR https://github.com/kiaisoft-vn/health_system_docs/pull/904/changes và \
             https://github.com/kiaisoft-vn/health_system_docs/pull/904, rồi http://github.com/a/b.c/pull/7",
        );
        assert_eq!(links.len(), 2);
        assert_eq!(
            links[0].url,
            "https://github.com/kiaisoft-vn/health_system_docs/pull/904"
        );
        assert_eq!(links[0].stem(), "pr-kiaisoft-vn-health_system_docs-904");
        assert_eq!((links[1].repo.as_str(), links[1].number), ("b.c", 7));
    }

    #[test]
    fn maps_folders_and_top_files() {
        let map = repo_map(
            "be",
            &[
                "AGENTS.md",
                "package.json",
                "src/main.ts",
                "src/auth/login.ts",
                "src/auth/deep/x.ts",
                "docs/CLAUDE.md",
            ],
        );
        assert!(
            map.starts_with("Repository `be/` (6 tracked files):"),
            "{map}"
        );
        assert!(map.contains("  be/src/ (3)\n"), "{map}");
        assert!(map.contains("  be/src/auth/ (2)\n"), "{map}");
        assert!(!map.contains("deep"), "{map}");
        assert!(
            map.contains("top-level files: AGENTS.md, package.json"),
            "{map}"
        );
    }
}
