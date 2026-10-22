//! The whole pipeline against a fake CLI that speaks Claude's stream-json:
//! design → plan → gate 1 → implement / check / review / fix → merge.

use super::adapter::Cli;
use super::engine::{AgentEnv, Engine, NewRun};
use super::git;
use super::log::AgentLog;
use crate::db::repos::agents::{
    self as repo, run_status, stage, task_status, ProjectSettings, RepoConfig, RoleCli, TaskDraft,
};
use crate::db::Db;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Plays every role from the prompt it gets (its last argument).
const FAKE_CLI: &str = r#"#!/bin/sh
last=""; resume=""; prev=""
for a in "$@"; do
  if [ "$prev" = "--resume" ]; then resume="$a"; fi
  prev="$a"; last="$a"
done
here=$(dirname "$0")
repos='["."]'
if [ -f "$here/plan-repos" ]; then repos=$(cat "$here/plan-repos"); fi
out() { printf '%s\n' "$1"; }
case "$last" in
  *"lead reviewer"*)
    out '{"type":"system","subtype":"init","session_id":"rev"}'
    verdict=approved
    case "$last" in *'Task `b`'*)
      if [ ! -f "$here/reviewed-b" ]; then touch "$here/reviewed-b"; verdict=changes_requested; fi;;
    esac
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"rev","result":"","structured_output":{"verdict":"'$verdict'","summary":"s","comments":[{"file":"b.txt","line":1,"issue":"more","severity":"must"}]}}'
    ;;
  *"split the work"*|*"could not be used"*)
    out '{"type":"system","subtype":"init","session_id":"lead"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"lead","result":"","structured_output":{"contract":"fn work()","tasks":[{"key":"a","title":"Task A","role":"backend","repos":'"$repos"',"description":"write a","acceptance":["a.txt"],"scope":["a.txt"],"depends_on":[]},{"key":"b","title":"Task B","role":"backend","repos":'"$repos"',"description":"write b","acceptance":["b.txt"],"scope":["b.txt"],"depends_on":["a"]}]}}'
    ;;
  *"lead architect"*|*"reviewed your design"*)
    out '{"type":"system","subtype":"init","session_id":"lead"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"lead","result":"Design OK"}'
    ;;
  *)
    key=""
    case "$last" in *'Task `a`'*) key=a;; *'Task `b`'*) key=b;; esac
    if [ -z "$key" ]; then key=${resume#w-}; fi
    out '{"type":"system","subtype":"init","session_id":"w-'$key'"}'
    if [ ! -f "$here/lazy-$key" ]; then
      # Every repo of the workspace: the folder itself, or its sub-folders.
      for d in . */; do
        if [ -e "$d/.git" ]; then echo "work" >> "$d/$key.txt"; fi
      done
    fi
    out '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Write","input":{"file_path":"'$key'.txt"}}]}}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"w-'$key'","result":"done","usage":{"input_tokens":10,"output_tokens":2},"total_cost_usd":0.01}'
    ;;
esac
"#;

struct FakeEnv {
    db: Db,
    dir: PathBuf,
    cli: PathBuf,
    logs: Mutex<Vec<Arc<AgentLog>>>,
}

impl AgentEnv for FakeEnv {
    fn db(&self) -> &Db {
        &self.db
    }

    fn data_dir(&self) -> PathBuf {
        self.dir.join("data")
    }

    fn find_cli(&self, _cli: Cli) -> Option<PathBuf> {
        Some(self.cli.clone())
    }

    fn open_log(&self, id: &str, _title: &str) -> Arc<AgentLog> {
        let log = Arc::new(AgentLog::new(id.to_string(), 256 * 1024, None));
        self.logs.lock().unwrap().push(Arc::clone(&log));
        log
    }

    fn close_log(&self, _id: &str, _ok: bool) {}

    fn changed(&self, _run_id: &str) {}
}

struct Setup {
    _dir: tempfile::TempDir,
    repo: PathBuf,
    cli_dir: PathBuf,
    env: Arc<FakeEnv>,
    engine: Arc<Engine>,
    project_id: String,
}

async fn setup(checks: &[&str]) -> Setup {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git::git(&repo, &["init", "-q", "-b", "main"])
        .await
        .unwrap();
    std::fs::write(repo.join("README"), "hi\n").unwrap();
    git::commit_all(&repo, "init").await.unwrap();

    let cli_dir = dir.path().join("cli");
    std::fs::create_dir(&cli_dir).unwrap();
    let cli = cli_dir.join("fake");
    std::fs::write(&cli, FAKE_CLI).unwrap();
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();

    let db = Db::open(dir.path().join("app.db")).unwrap();
    let project = db
        .write(|c| repo::project_open(c, "p1", &repo.to_string_lossy(), &[".".into()], "1"))
        .unwrap();
    db.write(|c| {
        repo::project_save(
            c,
            &project.id,
            &ProjectSettings {
                repos: vec![RepoConfig {
                    path: ".".into(),
                    check_cmds: checks.iter().map(|c| c.to_string()).collect(),
                    ..Default::default()
                }],
                notes: None,
            },
        )
    })
    .unwrap();
    let env = Arc::new(FakeEnv {
        db,
        dir: dir.path().to_path_buf(),
        cli,
        logs: Mutex::new(Vec::new()),
    });
    let engine = Engine::new(env.clone());
    Setup {
        repo,
        cli_dir,
        env,
        engine,
        project_id: project.id,
        _dir: dir,
    }
}

fn new_run(project_id: &str, plan_gate: bool) -> NewRun {
    NewRun {
        project_id: project_id.into(),
        request: "Thêm a và b".into(),
        title: None,
        leader_cli: "claude".into(),
        roles: vec![RoleCli {
            role: "Backend".into(),
            cli: "claude".into(),
        }],
        plan_gate,
        merge_gate: true,
        limits: None,
    }
}

async fn idle(engine: &Engine, run_id: &str) {
    for _ in 0..600 {
        if !engine.is_driving(run_id) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("run {run_id} did not settle");
}

fn exists(path: &Path) -> bool {
    path.exists()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_run_goes_from_design_to_a_merge_in_the_base_branch() {
    let s = setup(&["test -f a.txt"]).await;
    let run = s
        .engine
        .start_run(new_run(&s.project_id, true))
        .await
        .unwrap();
    assert_eq!(
        run.integration_branch,
        format!("cm/{}/integration", &run.id[..8])
    );
    idle(&s.engine, &run.id).await;

    // Gate 1.
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::PLAN_GATE,
        "{:?}",
        detail.run.status_note
    );
    assert_eq!(detail.run.status, run_status::WAITING);
    assert_eq!(detail.run.design_md.as_deref(), Some("Design OK"));
    assert_eq!(detail.run.contract.as_deref(), Some("fn work()"));
    assert_eq!(detail.run.leader_session.as_deref(), Some("lead"));
    let keys: Vec<&str> = detail.tasks.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(keys, vec!["a", "b"]);
    // Roles are matched case-insensitively to their CLI.
    assert!(detail
        .tasks
        .iter()
        .all(|t| t.role == "backend" && t.cli == "claude"));

    let drafts: Vec<TaskDraft> = detail
        .tasks
        .iter()
        .map(|t| TaskDraft {
            key: t.key.clone(),
            title: t.title.clone(),
            role: t.role.clone(),
            cli: t.cli.clone(),
            description: t.description.clone(),
            acceptance: t.acceptance.clone(),
            scope: t.scope.clone(),
            depends_on: t.depends_on.clone(),
            repos: t.repos.clone(),
        })
        .collect();
    s.engine
        .approve_plan(
            &run.id,
            "Design OK (edited)".into(),
            "fn work()".into(),
            drafts,
        )
        .unwrap();
    idle(&s.engine, &run.id).await;

    // Gate 2.
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    for task in &detail.tasks {
        assert_eq!(task.status, task_status::MERGED, "{task:?}");
        assert!(task.worktree.is_none());
        let checks = task.checks.as_ref().unwrap();
        assert!(checks.iter().all(|c| c.ok), "{checks:?}");
    }
    let b = &detail.tasks[1];
    // One review asked for changes, the fix was approved.
    assert_eq!(b.round, 1);
    assert_eq!(b.last_review.as_ref().unwrap().verdict, "approved");
    assert_eq!(b.worker_session.as_deref(), Some("w-b"));
    // design, plan, a: implement + review, b: implement + review + fix + review.
    assert_eq!(detail.usage.invocations, 8, "{:?}", detail.invocations);
    assert!(detail.invocations.iter().all(|i| i.ok == Some(true)));
    let fix = detail.invocations.iter().find(|i| i.step == "fix").unwrap();
    assert_eq!(fix.session_id.as_deref(), Some("w-b"));
    assert!(detail.run.integration_checks.as_ref().unwrap()[0].ok);
    assert_eq!(s.engine.run_diffstat(&run.id).await.unwrap(), (2, 3, 0));

    // The user's checkout is untouched until the merge.
    assert!(!exists(&s.repo.join("a.txt")));
    s.engine.merge_into_base(&run.id).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(s.repo.join("a.txt")).unwrap(),
        "work\n"
    );
    assert_eq!(
        std::fs::read_to_string(s.repo.join("b.txt")).unwrap(),
        "work\nwork\n"
    );
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.status, run_status::MERGED);
    assert_eq!(detail.run.stage, stage::DONE);
    assert!(!s
        .env
        .data_dir()
        .join("agents/wt")
        .join(&run.id[..8])
        .join("_integration")
        .exists());
    assert!(git::branch_exists(&s.repo, &run.integration_branch).await);
    assert!(!git::branch_exists(&s.repo, &format!("cm/{}/a", &run.id[..8])).await);
    // Every process had its log.
    assert!(s.env.logs.lock().unwrap().len() >= 8);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_task_without_changes_blocks_its_dependents_until_skipped() {
    let s = setup(&[]).await;
    std::fs::write(s.cli_dir.join("lazy-a"), "").unwrap();
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;

    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.stage, stage::IMPLEMENT);
    assert_eq!(detail.run.status, run_status::WAITING);
    let a = &detail.tasks[0];
    assert_eq!(a.status, task_status::FAILED);
    assert!(
        a.status_note.as_deref().unwrap().contains("không thay đổi"),
        "{a:?}"
    );
    assert_eq!(detail.tasks[1].status, task_status::WAITING);

    // Retrying is refused while the run drives; skipping lets b go on.
    s.engine.skip_task(&a.id).unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    assert_eq!(detail.tasks[1].status, task_status::MERGED);

    s.engine.discard(&run.id).await.unwrap();
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.status, run_status::DISCARDED);
    assert!(!git::branch_exists(&s.repo, &run.integration_branch).await);
    assert!(!git::branch_exists(&s.repo, &format!("cm/{}/a", &run.id[..8])).await);
    s.engine.delete(&run.id).unwrap();
    assert!(s.engine.detail(&run.id).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gate_one_feedback_goes_back_to_the_leader() {
    let s = setup(&[]).await;
    let run = s
        .engine
        .start_run(new_run(&s.project_id, true))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    assert!(s
        .engine
        .revise_plan(&run.id, "D".into(), "  ".into())
        .is_err());
    s.engine
        .revise_plan(&run.id, "Design (mine)".into(), "Tách nhỏ hơn".into())
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.stage, stage::PLAN_GATE);
    assert!(detail.run.pending_feedback.is_none());
    let redesign = detail
        .invocations
        .iter()
        .filter(|i| i.step == "design")
        .nth(1)
        .unwrap();
    // The leader continues its own session.
    assert_eq!(redesign.session_id.as_deref(), Some("lead"));
}

/// Talks to the real CLIs installed on this machine (costs a few tokens):
/// `cargo test --lib real_clis -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn real_clis_answer_review_json() {
    use super::adapter::{self, Access, Request};
    use super::process::{self, Spawn};
    use crate::db::repos::agents::CliProfile;

    let dir = tempfile::tempdir().unwrap();
    let schema = super::prompts::review_schema();
    let schema_file = dir.path().join("schema.json");
    std::fs::write(&schema_file, schema.to_string()).unwrap();
    for cli in Cli::ALL {
        let Some(program) = adapter::find(cli) else {
            eprintln!("{cli:?}: not installed");
            continue;
        };
        let profile = CliProfile::default();
        let args = adapter::args(
            cli,
            &Request {
                prompt: "This is a connectivity test. Do not read or change any file. Answer with \
                         verdict approved, summary \"ok\" and no comments."
                    .into(),
                cwd: dir.path(),
                access: Access::ReadOnly,
                resume: None,
                schema: Some(&schema),
                schema_file: (cli != Cli::Claude).then_some(schema_file.as_path()),
                profile: &profile,
            },
        );
        let log = AgentLog::new("real".into(), 256 * 1024, None);
        let (_stop, stop) = tokio::sync::watch::channel(false);
        let result = process::run(
            Spawn {
                cli,
                program: &program,
                args,
                cwd: dir.path(),
                timeout: Duration::from_secs(180),
                raw_log: Some(dir.path().join(format!("{}.jsonl", cli.as_str()))),
            },
            &log,
            stop,
        )
        .await;
        match result {
            Ok(outcome) => eprintln!(
                "{cli:?}: session={:?} error={:?} json={:?} tokens={}/{}",
                outcome.session_id,
                outcome.error,
                outcome.json(),
                outcome.input_tokens,
                outcome.output_tokens
            ),
            Err(error) => eprintln!("{cli:?}: {error}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_folder_of_repos_works_repo_by_repo() {
    let s = setup(&[]).await;
    // A cluster next to the single repo: be/ and fe/, the folder itself is
    // not a repository.
    let cluster = s.repo.parent().unwrap().join("shop");
    for repo in ["be", "fe"] {
        let dir = cluster.join(repo);
        std::fs::create_dir_all(&dir).unwrap();
        git::git(&dir, &["init", "-q", "-b", "main"]).await.unwrap();
        std::fs::write(dir.join("README"), repo).unwrap();
        git::commit_all(&dir, "init").await.unwrap();
    }
    let (root, repos) = super::workspace::detect(&cluster).await.unwrap();
    assert_eq!(repos, vec!["be", "fe"]);
    let project = s
        .env
        .db
        .write(|c| {
            repo::project_open(c, "p2", &root.to_string_lossy(), &repos, "1")?;
            repo::project_save(
                c,
                "p2",
                &ProjectSettings {
                    repos: vec![RepoConfig {
                        path: "fe".into(),
                        check_cmds: vec!["test -f a.txt".into()],
                        ..Default::default()
                    }],
                    notes: None,
                },
            )?;
            repo::project_get(c, "p2")
        })
        .unwrap();
    std::fs::write(s.cli_dir.join("plan-repos"), r#"["be","fe"]"#).unwrap();

    let run = s
        .engine
        .start_run(new_run(&project.id, false))
        .await
        .unwrap();
    assert_eq!(run.repos.len(), 2);
    assert!(run.repos.iter().all(|r| r.base_branch == "main"));
    idle(&s.engine, &run.id).await;

    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    for task in &detail.tasks {
        assert_eq!(task.status, task_status::MERGED, "{task:?}");
        assert_eq!(task.repos, vec!["be", "fe"]);
    }
    // Checks ran in fe only, labelled with it.
    let checks = detail.tasks[0].checks.as_ref().unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].command, "[fe] test -f a.txt");
    // Gate 2 numbers and diff span both repos.
    assert_eq!(s.engine.run_diffstat(&run.id).await.unwrap(), (4, 6, 0));
    let diff = s.engine.run_diff(&run.id).await.unwrap();
    assert!(
        diff.contains("b/be/a.txt") && diff.contains("b/fe/b.txt"),
        "{diff}"
    );

    // A repo on another branch blocks the merge before anything is merged.
    git::git(&cluster.join("fe"), &["checkout", "-q", "-b", "other"])
        .await
        .unwrap();
    let error = s.engine.merge_into_base(&run.id).await.unwrap_err();
    assert!(error.to_string().starts_with("[fe] "), "{error}");
    assert!(!cluster.join("be/a.txt").exists());
    git::git(&cluster.join("fe"), &["checkout", "-q", "main"])
        .await
        .unwrap();

    s.engine.merge_into_base(&run.id).await.unwrap();
    for repo in ["be", "fe"] {
        assert_eq!(
            std::fs::read_to_string(cluster.join(repo).join("b.txt")).unwrap(),
            "work\nwork\n"
        );
        assert!(!git::branch_exists(&cluster.join(repo), &format!("cm/{}/a", &run.id[..8])).await);
    }
    assert_eq!(
        s.engine.detail(&run.id).unwrap().run.status,
        run_status::MERGED
    );
}
