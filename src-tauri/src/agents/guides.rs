//! The team's own conventions, found in the project: rule files (AGENTS.md,
//! CLAUDE.md, `.claude/rules/`...) and skills (`<dir>/skills/*/SKILL.md` of
//! Claude, Codex, Cursor, Antigravity...). Every CLI reads only some of them
//! and only from where it starts, so the prompts list them all and the agent
//! reads the ones its work needs.

use super::workspace;
use crate::db::repos::agents::ROOT_REPO;
use std::path::{Path, PathBuf};

const RULE_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "GEMINI.md"];
/// Folders of rule files, one rule per `.md` / `.mdc`.
const RULE_DIRS: &[&str] = &[
    ".claude/rules",
    ".cursor/rules",
    ".agent/rules",
    ".agents/rules",
];
/// Folders holding `<skill>/SKILL.md`, most specific first: a skill found
/// in two of them (kept in sync for several CLIs) is listed once.
const SKILL_DIRS: &[&str] = &[
    ".claude/skills",
    ".agents/skills",
    ".agent/skills",
    ".codex/skills",
    ".cursor/skills",
    ".gemini/skills",
];
const MAX_RULES_PER_DIR: usize = 15;
const MAX_SKILLS: usize = 60;
const DESCRIPTION_CHARS: usize = 220;

/// Where a rule or skill belongs: a repo of the project, or the project
/// folder itself when that is not a repo (`health/.codex/skills`).
#[derive(Debug, Clone, PartialEq)]
pub enum Owner {
    Repo(String),
    Folder,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub owner: Owner,
    /// Relative to the project folder.
    pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Guides {
    /// (owner, path relative to the project folder)
    pub rules: Vec<(Owner, String)>,
    pub skills: Vec<Skill>,
}

/// Rules and skills of the project folder and its repos.
pub fn scan(project: &Path, repos: &[String]) -> Guides {
    let mut guides = Guides::default();
    let multi = !(repos.len() == 1 && repos[0] == ROOT_REPO);
    for repo in repos {
        collect(project, repo, Owner::Repo(repo.clone()), &mut guides);
    }
    if multi {
        collect(project, ROOT_REPO, Owner::Folder, &mut guides);
    }
    guides.skills.truncate(MAX_SKILLS);
    guides
}

fn collect(project: &Path, rel: &str, owner: Owner, guides: &mut Guides) {
    let base = workspace::repo_dir(project, rel);
    let at = |path: &str| workspace::project_path(rel, path);
    for file in RULE_FILES {
        if base.join(file).is_file() {
            guides.rules.push((owner.clone(), at(file)));
        }
    }
    for dir in RULE_DIRS {
        let mut found: Vec<String> = list(&base.join(dir))
            .into_iter()
            .filter(|name| name.ends_with(".md") || name.ends_with(".mdc"))
            .filter(|name| base.join(dir).join(name).is_file())
            .collect();
        found.sort();
        found.truncate(MAX_RULES_PER_DIR);
        for name in found {
            guides
                .rules
                .push((owner.clone(), at(&format!("{dir}/{name}"))));
        }
    }
    let mut seen: Vec<String> = Vec::new();
    for dir in SKILL_DIRS {
        let mut names = list(&base.join(dir));
        names.sort();
        for name in names {
            let file = base.join(dir).join(&name).join("SKILL.md");
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let (title, description) = front_matter(&text);
            let title = title.unwrap_or(name.clone());
            // The same skill kept for several CLIs, or the folder's copy of
            // a repo's skill: listed once.
            let shadowed = guides.skills.iter().any(|skill| {
                skill.name == title && (owner == Owner::Folder || skill.owner == owner)
            });
            if seen.contains(&title) || shadowed {
                continue;
            }
            seen.push(title.clone());
            guides.skills.push(Skill {
                name: title,
                description: shorten(&description.unwrap_or_default()),
                owner: owner.clone(),
                path: at(&format!("{dir}/{name}/SKILL.md")),
            });
        }
    }
}

fn list(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| !name.starts_with('.'))
                .collect()
        })
        .unwrap_or_default()
}

fn shorten(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= DESCRIPTION_CHARS {
        return text;
    }
    format!(
        "{}…",
        text.chars().take(DESCRIPTION_CHARS).collect::<String>()
    )
}

/// `name` and `description` of a SKILL.md's YAML front matter, including
/// folded (`>`) and literal (`|`) multi-line values.
fn front_matter(text: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    let (mut name, mut description) = (None, None);
    let mut block: Option<(&str, Vec<String>)> = None;
    let finish = |block: &mut Option<(&str, Vec<String>)>,
                  name: &mut Option<String>,
                  description: &mut Option<String>| {
        if let Some((key, parts)) = block.take() {
            let value = parts.join(" ");
            match key {
                "name" => *name = Some(value),
                "description" => *description = Some(value),
                _ => {}
            }
        }
    };
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if indented {
            if let Some((_, parts)) = block.as_mut() {
                parts.push(line.trim().to_string());
            }
            continue;
        }
        finish(&mut block, &mut name, &mut description);
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = match key.trim() {
            "name" => "name",
            "description" => "description",
            _ => continue,
        };
        let value = value.trim();
        let value = value.trim_matches('"').trim_matches('\'');
        let mut parts = Vec::new();
        if !matches!(value, ">" | "|" | ">-" | "|-" | "") {
            parts.push(value.to_string());
        }
        block = Some((key, parts));
    }
    finish(&mut block, &mut name, &mut description);
    (
        name.filter(|value| !value.is_empty()),
        description.filter(|value| !value.is_empty()),
    )
}

/// What an agent is doing, for what the guides block asks of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Use {
    Lead,
    Work,
    Review,
}

/// The guides block of a prompt for an agent working in `cwd`, limited to
/// `repos` (None: all), and the folders it must be allowed to read: guides
/// that are not inside `cwd` are given by their path in the project folder.
pub fn block(
    guides: &Guides,
    project: &Path,
    cwd: &Path,
    repos: Option<&[String]>,
    usage: Use,
) -> (String, Vec<PathBuf>) {
    let wanted = |owner: &Owner| match (owner, repos) {
        (Owner::Folder, _) | (_, None) => true,
        (Owner::Repo(repo), Some(repos)) => repos.contains(repo),
    };
    let mut outside: Vec<PathBuf> = Vec::new();
    let mut show = |rel: &str| -> String {
        if cwd.join(rel).exists() {
            return rel.to_string();
        }
        let path = project.join(rel);
        // Its folder: a skill's references come with it.
        if let Some(dir) = path.parent().map(Path::to_path_buf) {
            if !outside.contains(&dir) {
                outside.push(dir);
            }
        }
        path.to_string_lossy().into_owned()
    };
    let rules: Vec<String> = guides
        .rules
        .iter()
        .filter(|(owner, _)| wanted(owner))
        .map(|(_, rel)| format!("- `{}`", show(rel)))
        .collect();
    let skills: Vec<String> = guides
        .skills
        .iter()
        .filter(|skill| wanted(&skill.owner))
        .map(|skill| {
            let place = match &skill.owner {
                Owner::Repo(repo) if repo != ROOT_REPO => format!(" [{repo}]"),
                _ => String::new(),
            };
            format!(
                "- `{}`{place}: {} → `{}`",
                skill.name,
                if skill.description.is_empty() {
                    "(no description)"
                } else {
                    &skill.description
                },
                show(&skill.path)
            )
        })
        .collect();
    if rules.is_empty() && skills.is_empty() {
        return (String::new(), outside);
    }
    let mut text = String::from(
        "\nThe project's own rules and skills (the team's conventions; they win over general \
habits):\n",
    );
    if !rules.is_empty() {
        text.push_str("Rules: read the ones of the repositories you work in before you start.\n");
        text.push_str(&rules.join("\n"));
        text.push('\n');
    }
    if !skills.is_empty() {
        text.push_str(match usage {
            Use::Lead => {
                "Skills: procedures the team follows; name the ones a task should follow in \
its description.\n"
            }
            Use::Work => {
                "Skills: when one fits your task, read its SKILL.md and follow it. Ignore steps \
about committing, pushing or opening pull requests: the orchestrator does that.\n"
            }
            Use::Review => {
                "Skills: if one is a code-review procedure for this repository, review by its \
criteria. The orchestrator already ran the automated checks (results below); do not run \
them again.\n"
            }
        });
        text.push_str(&skills.join("\n"));
        text.push('\n');
    }
    (text, outside)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn front_matter_reads_plain_and_folded_values() {
        let (name, description) = front_matter(
            "---\nname: wire-api\ndescription: >\n  Connect a screen\n  to the backend.\nmetadata:\n  a: b\n---\n# Body",
        );
        assert_eq!(name.as_deref(), Some("wire-api"));
        assert_eq!(
            description.as_deref(),
            Some("Connect a screen to the backend.")
        );
        let (name, description) = front_matter("---\nname: \"x\"\ndescription: 'Do y'\n---\n");
        assert_eq!(
            (name.as_deref(), description.as_deref()),
            (Some("x"), Some("Do y"))
        );
        assert_eq!(front_matter("# no front matter"), (None, None));
    }

    #[test]
    fn guides_of_a_folder_of_repos() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(&root.join("fe/AGENTS.md"), "rules");
        write(&root.join("fe/CLAUDE.md"), "@AGENTS.md");
        write(&root.join("be/.claude/rules/workflow.md"), "rule");
        let review = "---\nname: code-review\ndescription: Review a PR. Run pnpm check.\n---\n";
        write(&root.join("fe/.claude/skills/code-review/SKILL.md"), review);
        // The same skill for another CLI, and the folder's copy: once.
        write(&root.join("fe/.cursor/skills/code-review/SKILL.md"), review);
        write(&root.join(".cursor/skills/code-review/SKILL.md"), review);
        write(
            &root.join(".codex/skills/health-frontend/SKILL.md"),
            "---\nname: health-frontend\ndescription: Work on the frontend.\n---\n",
        );
        write(
            &root.join("be/.claude/skills/no-meta/SKILL.md"),
            "# No front matter",
        );

        let guides = scan(root, &["be".into(), "fe".into()]);
        let rules: Vec<&str> = guides.rules.iter().map(|(_, rel)| rel.as_str()).collect();
        assert_eq!(
            rules,
            vec![
                "be/.claude/rules/workflow.md",
                "fe/AGENTS.md",
                "fe/CLAUDE.md"
            ]
        );
        let skills: Vec<(&str, &str)> = guides
            .skills
            .iter()
            .map(|skill| (skill.name.as_str(), skill.path.as_str()))
            .collect();
        assert_eq!(
            skills,
            vec![
                ("no-meta", "be/.claude/skills/no-meta/SKILL.md"),
                ("code-review", "fe/.claude/skills/code-review/SKILL.md"),
                ("health-frontend", ".codex/skills/health-frontend/SKILL.md"),
            ]
        );

        // A worker in a workspace that mirrors fe/ only: fe's guides by their
        // place there, the folder's by their path in the project.
        let ws = dir.path().join("ws");
        write(&ws.join("fe/AGENTS.md"), "rules");
        write(&ws.join("fe/CLAUDE.md"), "@AGENTS.md");
        write(&ws.join("fe/.claude/skills/code-review/SKILL.md"), review);
        let (text, outside) = block(&guides, root, &ws, Some(&["fe".into()]), Use::Review);
        assert!(text.contains("- `fe/AGENTS.md`"), "{text}");
        assert!(!text.contains("be/"), "{text}");
        assert!(text.contains("`code-review` [fe]: Review a PR. Run pnpm check. → `fe/.claude/skills/code-review/SKILL.md`"));
        assert!(text.contains("do not run them again"));
        let codex = root.join(".codex/skills/health-frontend");
        assert!(
            text.contains(&format!("{}", codex.join("SKILL.md").display())),
            "{text}"
        );
        assert_eq!(outside, vec![codex]);
    }
}
