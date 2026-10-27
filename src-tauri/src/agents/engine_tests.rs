//! The whole pipeline against a fake CLI that speaks Claude's stream-json:
//! design → plan → gate 1 → implement / check / review / fix → merge.

use super::adapter::Cli;
use super::engine::{AgentEnv, AiPush, AttachPr, Engine, NewRun, QaAnswer, WORKTREE_DIR_SETTING};
use super::git;
use super::log::AgentLog;
use crate::db::repos::agents::{
    self as repo, run_status, stage, task_status, ProjectSettings, RepoConfig, RoleCli, TaskDraft,
};
use crate::db::repos::settings;
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
    if [ -f "$here/picky" ]; then verdict=changes_requested; fi
    case "$last" in *'Task `b`'*)
      if [ ! -f "$here/reviewed-b" ]; then touch "$here/reviewed-b"; verdict=changes_requested; fi;;
    esac
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"rev","result":"","structured_output":{"verdict":"'$verdict'","summary":"s","comments":[{"file":"b.txt","line":1,"issue":"more","severity":"must"}]}}'
    ;;
  *"find out whether anything blocks it"*|*"asks you to look again"*)
    printf '%s\n' "$*" >> "$here/args-research"
    qa='[]'
    if [ -f "$here/research-blocker" ] && [ ! -f "$here/research-again" ]; then
      qa='[{"id":"Q1","title":"API-361 chưa có","severity":"blocker","detail":"Không thấy route","question":"API-361 xong chưa?","assumption":"Dùng mock","source":"api_list.md"},{"id":"Q2","title":"Câu chữ","severity":"question","detail":"","question":"Dùng chữ nào?","assumption":"Theo BD","source":""}]'
    fi
    out '{"type":"system","subtype":"init","session_id":"research"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"research","result":"","structured_output":{"summary":"Tìm hiểu xong","findings":[{"title":"API-360","status":"ready","detail":"có","source":"x"}],"qa":'"$qa"'}}'
    ;;
  *"a question (it is"*)
    printf '%s\n' "$*" >> "$here/args-leaderchat"
    out '{"type":"system","subtype":"init","session_id":"lead-chat"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"lead-chat","result":"Vì a phải xong trước b"}'
    ;;
  *"commit-msg hook refused"*)
    printf '%s\n' "$last" >> "$here/prompts-commit"
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"'$resume'","result":"<commit>\nfeat: add the work file\n</commit>"}'
    ;;
  *"Pushing this run's work was refused"*)
    printf '%s\n' "$last" >> "$here/prompts-pushfail"
    action=tasks
    if [ -f "$here/push-action" ]; then action=$(cat "$here/push-action"); fi
    tasks='[]'
    if [ "$action" = tasks ]; then
      tasks='[{"key":"fix-push","title":"Sửa test hỏng","role":"backend","repos":["."],"description":"test hỏng","acceptance":["xanh"],"scope":["fix-push.txt"],"depends_on":[]}]'
    fi
    out '{"type":"system","subtype":"init","session_id":"pf"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"pf","result":"","structured_output":{"summary":"Test X hỏng","action":"'$action'","tasks":'"$tasks"'}}'
    ;;
  *"was refused and you found"*)
    printf '%s\n' "$*" >> "$here/args-selffix"
    echo work > self-fix.txt
    out '{"type":"system","subtype":"init","session_id":"pf"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"pf","result":"Đã sửa test"}'
    ;;
  *"You write the pull request"*)
    printf '%s\n' "$last" >> "$here/prompts-describe"
    out '{"type":"system","subtype":"init","session_id":"desc"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"desc","result":"","structured_output":{"title":"feat: thêm a và b\nthừa","body":"**Summary**\nThêm a, b"}}'
    ;;
  *"PERSON will do by hand"*)
    printf '%s\n' "$last" >> "$here/prompts-prepare"
    out '{"type":"system","subtype":"init","session_id":"prep"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"prep","result":"","structured_output":{"summary":"Test a và b","setup":["Mở /login"],"cases":[{"id":"C1","title":"Đăng nhập đúng","steps":["Nhập","Bấm"],"expected":"Vào trang chủ"},{"id":"C2","title":"Sai mật khẩu","steps":["Nhập sai"],"expected":"Báo lỗi"}],"notes":""}}'
    ;;
  *"by hand and reports failures"*)
    printf '%s\n' "$last" >> "$here/prompts-triage"
    out '{"type":"system","subtype":"init","session_id":"tri"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"tri","result":"","structured_output":{"summary":"Sẽ sửa thông báo lỗi","tasks":[{"key":"fix-x","title":"Sửa thông báo","role":"backend","repos":["."],"description":"C2 không báo lỗi","acceptance":["báo lỗi"],"scope":["fix-x.txt"],"depends_on":[]}]}}'
    ;;
  *"is stuck. Help them"*|*"Câu hỏi 2"*)
    printf '%s\n' "$*" >> "$here/args-chat"
    out '{"type":"system","subtype":"init","session_id":"chat"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"chat","result":"Dùng tài khoản admin"}'
    ;;
  *"engineer who ships"*)
    out '{"type":"system","subtype":"init","session_id":"ship"}'
    if [ ! -f "$here/push-lazy" ]; then
      git fetch -q origin && git rebase -q origin/main && git push -q --force-with-lease origin HEAD:feat/manual
    fi
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"ship","result":"rebased and pushed","total_cost_usd":0.02}'
    ;;
  *"follow-up request"*)
    out '{"type":"system","subtype":"init","session_id":"lead"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"lead","result":"","structured_output":{"contract":"fn more()","tasks":[{"key":"c","title":"Task C","role":"backend","repos":'"$repos"',"description":"write c","acceptance":["c.txt"],"scope":["c.txt"],"depends_on":["a"]}]}}'
    ;;
  *"split the work"*|*"could not be used"*)
    out '{"type":"system","subtype":"init","session_id":"lead"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"lead","result":"","structured_output":{"contract":"fn work()","checks":[{"repo":".","setup":" ","commands":["test -f README"," "]}],"tasks":[{"key":"a","title":"Task A","role":"backend","repos":'"$repos"',"description":"write a","acceptance":["a.txt"],"scope":["a.txt"],"depends_on":[]},{"key":"b","title":"Task B","role":"backend","repos":'"$repos"',"description":"write b","acceptance":["b.txt"],"scope":["b.txt"],"depends_on":["a"]}]}}'
    ;;
  *"cut short"*)
    out '{"type":"system","subtype":"init","session_id":"'$resume'"}'
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"'$resume'","result":"Design resumed"}'
    ;;
  *"lead architect"*|*"reviewed your design"*|*"Your research is settled"*)
    printf '%s\n' "$*" >> "$here/args-lead"
    out '{"type":"system","subtype":"init","session_id":"lead"}'
    if [ -f "$here/quota" ]; then
      resets=$(( $(date +%s) + 2 ))
      out '{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","unifiedWindows":{"five_hour":{"utilization":0.9,"resetsAt":'$resets'},"seven_day":{"utilization":0.3,"resetsAt":'$resets'}}}}'
      sleep 20
    fi
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"lead","result":"Design OK"}'
    ;;
  *)
    key=""
    case "$last" in *'Task `a`'*) key=a;; *'Task `b`'*) key=b;; *'Task `c`'*) key=c;; *'Task `fix-x`'*) key=fix-x;; *'Task `fix-push`'*) key=fix-push;; esac
    if [ -z "$key" ]; then key=${resume#w-}; fi
    printf '%s\n' "$last" >> "$here/prompts-$key"
    printf '%s\n' "$*" >> "$here/args-$key"
    printf '%s\n' "$GIT_CONFIG_KEY_0" > "$here/env-$key"
    out '{"type":"system","subtype":"init","session_id":"w-'$key'"}'
    # A resumed session reports its whole cost so far.
    cost=0.01
    if [ -n "$resume" ]; then cost=0.03; fi
    if [ -n "$resume" ] && [ -f "$here/lazy-fix" ]; then
      touch "$here/lazy-$key"
    fi
    if [ ! -f "$here/lazy-$key" ]; then
      # Every repo of the workspace: the folder itself, or its sub-folders.
      for d in . */; do
        if [ -e "$d/.git" ]; then echo "work" >> "$d/$key.txt"; fi
      done
    fi
    out '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Write","input":{"file_path":"'$key'.txt"}}]}}'
    answer=done
    if [ -f "$here/commits" ]; then answer="done\\n<commit>\\nfeat($key): add $key.txt\\n</commit>"; fi
    out '{"type":"result","subtype":"success","is_error":false,"session_id":"w-'$key'","result":"'"$answer"'","usage":{"input_tokens":10,"output_tokens":2},"total_cost_usd":'$cost'}'
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

    fn find_gh(&self) -> Option<PathBuf> {
        Some(self.dir.join("cli/gh"))
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
        leader_model: Some("lead-model".into()),
        leader_effort: None,
        roles: vec![RoleCli {
            role: "Backend".into(),
            cli: "claude".into(),
            model: Some(" role-model ".into()),
            ..Default::default()
        }],
        plan_gate,
        merge_gate: true,
        limits: None,
        research: false,
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
            kind: t.kind.clone(),
        })
        .collect();
    s.engine
        .approve_plan(
            &run.id,
            "Design OK (edited)".into(),
            "fn work()".into(),
            drafts,
            Vec::new(),
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
    // The leader's worktree and the task slots stay for pushes and
    // follow-ups; merged task branches go.
    let wt = s.env.data_dir().join("agents/wt").join(&run.id[..8]);
    assert!(wt.join("_integration").exists());
    let mut kept: Vec<String> = std::fs::read_dir(&wt)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    kept.sort();
    assert!(
        kept.iter()
            .all(|name| name == "_integration" || name.starts_with("slot-")),
        "{kept:?}"
    );
    assert!(detail.worktrees);
    assert!(git::branch_exists(&s.repo, &run.integration_branch).await);
    assert!(!git::branch_exists(&s.repo, &format!("cm/{}/a", &run.id[..8])).await);
    s.engine.clean_worktrees(&run.id).await.unwrap();
    assert!(!wt.exists());
    assert!(!s.engine.detail(&run.id).unwrap().worktrees);
    assert!(git::branch_exists(&s.repo, &run.integration_branch).await);
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
    s.engine.delete(&run.id).await.unwrap();
    assert!(s.engine.detail(&run.id).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_run_keeps_its_worktrees_where_the_setting_said_when_it_started() {
    let s = setup(&[]).await;
    let root = s.env.dir.join("big-drive");
    s.env
        .db
        .write(|c| settings::set(c, WORKTREE_DIR_SETTING, &root.to_string_lossy()))
        .unwrap();
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    assert_eq!(run.worktree_root.as_deref(), Some(&*root.to_string_lossy()));
    // A later change of the setting is for new runs only.
    s.env
        .db
        .write(|c| settings::set(c, WORKTREE_DIR_SETTING, ""))
        .unwrap();
    idle(&s.engine, &run.id).await;

    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    let wt = root.join(&run.id[..8]);
    assert!(wt.join("_integration").exists());
    assert!(!s
        .env
        .data_dir()
        .join("agents/wt")
        .join(&run.id[..8])
        .exists());
    s.engine.discard(&run.id).await.unwrap();
    assert!(!wt.join("_integration").exists());

    s.env
        .db
        .write(|c| settings::set(c, WORKTREE_DIR_SETTING, "rel/wt"))
        .unwrap();
    let error = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("tuyệt đối"), "{error}");
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
                budget_usd: None,
                extra_dirs: &[],
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
                quota_limit: None,
                env: Vec::new(),
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
            Err(stopped) => eprintln!("{cli:?}: {}", stopped.error),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failing_checks_go_back_to_the_worker_without_a_review() {
    // Passes once the worker has written a.txt twice (implement + fix).
    let s = setup(&["test ! -f a.txt || [ $(wc -l < a.txt) -ge 2 ]"]).await;
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    let a = &detail.tasks[0];
    assert_eq!(a.status, task_status::MERGED);
    assert_eq!(a.round, 1);
    let steps: Vec<&str> = detail
        .invocations
        .iter()
        .filter(|i| i.task_id.as_deref() == Some(a.id.as_str()))
        .map(|i| i.step.as_str())
        .collect();
    // No review of the failing round.
    assert_eq!(steps, vec!["implement", "fix", "review"]);
    // Each check ran in a terminal of its task, listed with the run.
    let checks: Vec<_> = detail
        .side_logs
        .iter()
        .filter(|log| log.kind == "check" && log.task_id.as_deref() == Some(a.id.as_str()))
        .collect();
    assert_eq!(checks.len(), 2, "{:?}", detail.side_logs);
    assert_eq!((checks[0].ok, checks[1].ok), (Some(false), Some(true)));
    assert!(checks
        .iter()
        .all(|log| log.id.starts_with("agent:check-") && log.finished_at.is_some()));
    assert!(detail
        .side_logs
        .iter()
        .any(|log| log.kind == "check" && log.task_id.is_none()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_usage_limit_pauses_and_the_step_goes_on_in_its_session() {
    let s = setup(&[]).await;
    std::fs::write(s.cli_dir.join("quota"), "").unwrap();
    let run = s
        .engine
        .start_run(new_run(&s.project_id, true))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.stage, stage::DESIGN);
    assert_eq!(detail.run.status, run_status::PAUSED);
    let note = detail.run.status_note.clone().unwrap_or_default();
    assert!(note.contains("5 giờ") && note.contains("90%"), "{note}");
    assert!(detail.resume_at.is_some());
    assert!(detail.quotas.contains_key("claude"));
    let cut = &detail.invocations[0];
    assert!(cut.resumable, "{cut:?}");
    assert_eq!(cut.session_id.as_deref(), Some("lead"));

    // The window has reset: going on continues the same session.
    std::fs::remove_file(s.cli_dir.join("quota")).unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    s.engine.resume(&run.id).unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::PLAN_GATE,
        "{:?}",
        detail.run.status_note
    );
    assert_eq!(detail.run.design_md.as_deref(), Some("Design resumed"));
    assert!(detail.resume_at.is_none());
    let design: Vec<_> = detail
        .invocations
        .iter()
        .filter(|i| i.step == "design")
        .collect();
    assert_eq!(design.len(), 2);
    assert_eq!(design[1].session_id.as_deref(), Some("lead"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn later_tasks_get_handoffs_and_a_delivered_run_takes_a_follow_up() {
    let s = setup(&[]).await;
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    // a's worker left its note; b's worker got it.
    let a = &detail.tasks[0];
    assert_eq!(a.handoff.as_deref(), Some("done\nFiles changed: a.txt"));
    let b_prompts = std::fs::read_to_string(s.cli_dir.join("prompts-b")).unwrap();
    assert!(b_prompts.contains("Notes from teammates"), "{b_prompts}");
    assert!(b_prompts.contains("### `a` — Task A\ndone\nFiles changed: a.txt"));
    let a_prompts = std::fs::read_to_string(s.cli_dir.join("prompts-a")).unwrap();
    assert!(!a_prompts.contains("Notes from teammates"));
    // Models picked for the run beat the profile's (claude: Sonnet for work).
    let lead_args = std::fs::read_to_string(s.cli_dir.join("args-lead")).unwrap();
    assert!(lead_args.contains("--model lead-model"), "{lead_args}");
    let a_args = std::fs::read_to_string(s.cli_dir.join("args-a")).unwrap();
    assert!(a_args.contains("--model role-model"), "{a_args}");
    assert!(!a_args.contains("sonnet"), "{a_args}");

    // Models can change while the run is not running, roles stay.
    s.engine
        .set_models(
            &run.id,
            Some("lead-2".into()),
            Some(" ".into()),
            vec![RoleCli {
                role: "backend".into(),
                cli: "codex".into(),
                model: Some("role-2".into()),
                effort: Some("low".into()),
            }],
        )
        .unwrap();
    let changed = s.engine.detail(&run.id).unwrap().run;
    assert_eq!(changed.leader_model.as_deref(), Some("lead-2"));
    assert_eq!(changed.leader_effort, None);
    assert_eq!(changed.roles[0].cli, "claude");
    assert_eq!(changed.roles[0].model.as_deref(), Some("role-2"));
    assert!(s
        .engine
        .set_models(
            &run.id,
            None,
            None,
            vec![RoleCli {
                role: "qa".into(),
                ..Default::default()
            }]
        )
        .is_err());

    // Only delivered runs go on.
    assert!(s
        .engine
        .follow_up(&run.id, "x".into(), false)
        .await
        .is_err());
    s.engine.merge_into_base(&run.id).await.unwrap();
    // Someone commits on the base branch after the merge.
    std::fs::write(s.repo.join("later.txt"), "x\n").unwrap();
    git::commit_all(&s.repo, "later").await.unwrap();
    let head = git::commit_of(&s.repo, "main").await.unwrap();

    s.engine
        .follow_up(&run.id, "Reviewer: thêm c".into(), false)
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    assert_eq!(detail.run.followups.len(), 1);
    assert_eq!(detail.run.repos[0].base_commit, head);
    assert_eq!(
        detail.run.contract.as_deref(),
        Some("fn work()\n\n## Follow-up 1\nfn more()")
    );
    let keys: Vec<(&str, u32, &str)> = detail
        .tasks
        .iter()
        .map(|t| (t.key.as_str(), t.batch, t.status.as_str()))
        .collect();
    assert_eq!(
        keys,
        vec![
            ("a", 0, task_status::MERGED),
            ("b", 0, task_status::MERGED),
            ("c", 1, task_status::MERGED)
        ]
    );
    // The dependency on delivered work was dropped.
    assert!(detail.tasks[2].depends_on.is_empty());
    // The follow-up plan ran with the new leader model, c with the role's.
    let lead_args = std::fs::read_to_string(s.cli_dir.join("args-c")).unwrap();
    assert!(lead_args.contains("--model role-2"), "{lead_args}");
    let c_prompts = std::fs::read_to_string(s.cli_dir.join("prompts-c")).unwrap();
    assert!(c_prompts.contains("### `b` — Task B"), "{c_prompts}");
    // The leader planned it in its own session.
    let plan = detail
        .invocations
        .iter()
        .rev()
        .find(|i| i.step == "plan")
        .unwrap();
    assert_eq!(plan.session_id.as_deref(), Some("lead"));

    // Only the follow-up's work is merged this time, on top of the new base.
    s.engine.merge_into_base(&run.id).await.unwrap();
    assert!(s.repo.join("c.txt").exists());
    assert!(s.repo.join("later.txt").exists());
    assert_eq!(
        std::fs::read_to_string(s.repo.join("a.txt")).unwrap(),
        "work\n"
    );
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.status, run_status::MERGED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_tasks_are_handed_over_together() {
    let s = setup(&[]).await;
    std::fs::write(s.cli_dir.join("lazy-a"), "").unwrap();
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    let a = detail.tasks[0].clone();
    assert_eq!(a.status, task_status::FAILED, "{a:?}");

    // Nothing changes when one of them cannot be handed over.
    assert!(s
        .engine
        .reassign_tasks(&run.id, &[], "claude", None, None, None)
        .is_err());
    assert!(s
        .engine
        .reassign_tasks(
            &run.id,
            std::slice::from_ref(&a.id),
            "gemini",
            None,
            None,
            None
        )
        .is_err());
    assert!(s
        .engine
        .reassign_tasks(
            &run.id,
            std::slice::from_ref(&a.id),
            "claude",
            None,
            None,
            Some(0)
        )
        .is_err());
    assert_eq!(
        s.engine.detail(&run.id).unwrap().tasks[0].status,
        task_status::FAILED
    );

    std::fs::remove_file(s.cli_dir.join("lazy-a")).unwrap();
    let ids: Vec<String> = detail.tasks.iter().map(|t| t.id.clone()).collect();
    s.engine
        .reassign_tasks(&run.id, &ids, "claude", Some("m2".into()), None, Some(90))
        .unwrap();
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
        assert_eq!(task.model.as_deref(), Some("m2"));
        assert_eq!(task.timeout_minutes, Some(90));
    }
    // A task's own timeout can change, within bounds.
    s.engine.set_task_timeout(&a.id, None).unwrap();
    assert!(s.engine.set_task_timeout(&a.id, Some(601)).is_err());
    // The run's limits became the project's defaults, and follow changes.
    let project = s
        .env
        .db
        .read(|c| repo::project_get(c, &s.project_id))
        .unwrap();
    assert_eq!(project.limits, Some(detail.run.limits));
    let mut limits = detail.run.limits;
    limits.timeout_minutes = 75;
    s.engine.set_limits(&run.id, limits).unwrap();
    let project = s
        .env
        .db
        .read(|c| repo::project_get(c, &s.project_id))
        .unwrap();
    assert_eq!(project.limits.unwrap().timeout_minutes, 75);
    limits.timeout_minutes = 601;
    assert!(s.engine.set_limits(&run.id, limits).is_err());
    let a_args = std::fs::read_to_string(s.cli_dir.join("args-a")).unwrap();
    // The first try ran with the role's model, the handed-over one with m2.
    let runs: Vec<&str> = a_args.lines().filter(|l| l.starts_with("-p ")).collect();
    assert!(runs[0].contains("--model role-model"), "{a_args}");
    assert!(runs.last().unwrap().contains("--model m2"), "{a_args}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_skills_reach_the_agents_and_proposed_checks_wait_for_the_gate() {
    let s = setup(&[]).await;
    // The repo's own rules and skills, committed.
    std::fs::write(s.repo.join("AGENTS.md"), "Run pnpm check.\n").unwrap();
    let skill = s.repo.join(".claude/skills/code-review");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: code-review\ndescription: Review by the team's checklist.\n---\n",
    )
    .unwrap();
    git::commit_all(&s.repo, "rules").await.unwrap();

    let run = s
        .engine
        .start_run(new_run(&s.project_id, true))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.stage, stage::PLAN_GATE);
    // Proposed, not applied: nothing runs before the user says so.
    assert_eq!(detail.run.suggested_checks.len(), 1);
    assert_eq!(
        detail.run.suggested_checks[0].commands,
        vec!["test -f README"]
    );
    assert_eq!(detail.run.suggested_checks[0].setup, "");
    let project = s
        .env
        .db
        .read(|c| repo::project_get(c, &s.project_id))
        .unwrap();
    assert!(project.repos[0].check_cmds.is_empty());

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
            kind: t.kind.clone(),
        })
        .collect();
    s.engine
        .approve_plan(
            &run.id,
            "Design OK".into(),
            "fn work()".into(),
            drafts,
            detail.run.suggested_checks.clone(),
        )
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    assert!(detail.run.suggested_checks.is_empty());
    let project = s
        .env
        .db
        .read(|c| repo::project_get(c, &s.project_id))
        .unwrap();
    assert_eq!(project.repos[0].check_cmds, vec!["test -f README"]);
    let checks = detail.tasks[0].checks.as_ref().unwrap();
    assert!(checks[0].ok && checks[0].command.contains("test -f README"));

    // Workers cannot push: every push URL is rewritten to a helper that
    // does not exist, whatever remote or -C names it.
    let env = std::fs::read_to_string(s.cli_dir.join("env-a")).unwrap();
    assert!(env.contains("pushInsteadOf"), "{env}");
    let remote = s.cli_dir.join("remote.git");
    git::git(&s.cli_dir, &["init", "-q", "--bare", "remote.git"])
        .await
        .unwrap();
    let pushed = std::process::Command::new("git")
        .args([
            "-C",
            &s.repo.to_string_lossy(),
            "push",
            &remote.to_string_lossy(),
            "main",
        ])
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", env.trim())
        .env("GIT_CONFIG_VALUE_0", "")
        .output()
        .unwrap();
    assert!(!pushed.status.success());
    assert!(git::git(&remote, &["show-ref"]).await.is_err());

    // Workers are pointed at the rules and the skill.
    let a_prompts = std::fs::read_to_string(s.cli_dir.join("prompts-a")).unwrap();
    assert!(a_prompts.contains("- `AGENTS.md`"), "{a_prompts}");
    assert!(a_prompts.contains(
        "- `code-review`: Review by the team's checklist. → `.claude/skills/code-review/SKILL.md`"
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gate_two_pushes_and_opens_a_pull_request_once() {
    use std::os::unix::fs::PermissionsExt;
    let s = setup(&[]).await;
    let remote = s.cli_dir.join("remote.git");
    git::git(&s.cli_dir, &["init", "-q", "--bare", "remote.git"])
        .await
        .unwrap();
    git::git(
        &s.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    )
    .await
    .unwrap();
    // A gh that records its arguments and answers like the real one.
    let gh = s.cli_dir.join("gh");
    std::fs::write(
        &gh,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$(dirname \"$0\")/gh-args\"\necho 'Creating pull request'\necho 'https://github.com/o/r/pull/7'\n",
    )
    .unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();

    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let mut draft = s.engine.pr_draft(&run.id).await.unwrap();
    assert_eq!(draft.repos.len(), 1);
    assert_eq!(draft.repos[0].branch, run.integration_branch);
    assert_eq!(draft.repos[0].base, "main");
    assert!(draft.body.contains("`a` Task A"), "{}", draft.body);

    draft.repos[0].branch = "-x".into();
    assert!(s
        .engine
        .open_pull_requests(&run.id, draft.clone())
        .await
        .is_err());
    draft.repos[0].branch = "feat/cart".into();
    let done = s
        .engine
        .open_pull_requests(&run.id, draft.clone())
        .await
        .unwrap();
    assert_eq!(done.status, run_status::KEPT);
    assert_eq!(done.stage, stage::DONE);
    assert_eq!(done.pull_requests[0].url, "https://github.com/o/r/pull/7");
    assert!(git::git(&remote, &["show-ref", "refs/heads/feat/cart"])
        .await
        .is_ok());
    let args = std::fs::read_to_string(s.cli_dir.join("gh-args")).unwrap();
    assert!(
        args.contains("pr create --base main --head feat/cart --title Thêm a và b --body-file"),
        "{args}"
    );

    // A follow-up pushes to the same branch and does not open another PR.
    s.engine
        .follow_up(&run.id, "Reviewer: thêm c".into(), false)
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let draft = s.engine.pr_draft(&run.id).await.unwrap();
    assert_eq!(draft.repos[0].branch, "feat/cart");
    let done = s.engine.open_pull_requests(&run.id, draft).await.unwrap();
    assert_eq!(done.pull_requests.len(), 1);
    let args = std::fs::read_to_string(s.cli_dir.join("gh-args")).unwrap();
    assert_eq!(args.lines().count(), 1, "{args}");
    let pushed = git::git(&remote, &["log", "--oneline", "feat/cart"])
        .await
        .unwrap();
    assert!(pushed.contains("merge c"), "{pushed}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_forced_review_takes_hand_made_changes_and_merges() {
    let s = setup(&[]).await;
    std::fs::write(s.cli_dir.join("lazy-a"), "").unwrap();
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let a = s.engine.detail(&run.id).unwrap().tasks[0].clone();
    assert_eq!(a.status, task_status::FAILED);
    // Nothing to review yet.
    assert!(s.engine.rereview_task(&a.id).await.is_err());

    // The user writes the code by hand in the task's worktree.
    let worktree = PathBuf::from(a.worktree.clone().unwrap());
    std::fs::write(worktree.join("a.txt"), "by hand\n").unwrap();
    std::fs::remove_file(s.cli_dir.join("lazy-a")).unwrap();
    s.engine.rereview_task(&a.id).await.unwrap();
    idle(&s.engine, &run.id).await;

    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    assert_eq!(detail.tasks[0].status, task_status::MERGED);
    assert_eq!(
        detail.tasks[0].last_review.as_ref().unwrap().verdict,
        "approved"
    );
    // a's worker never ran again: the leader reviewed the hand-made work.
    let a_runs = detail
        .invocations
        .iter()
        .filter(|i| i.task_id.as_deref() == Some(a.id.as_str()))
        .map(|i| i.step.as_str())
        .collect::<Vec<_>>();
    assert_eq!(a_runs, vec!["implement", "review"]);
    let log = git::git(&s.repo, &["log", "--format=%s", &run.integration_branch])
        .await
        .unwrap();
    assert!(log.contains("manual changes before review"), "{log}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fix_round_that_changes_nothing_stops_instead_of_looping() {
    let s = setup(&[]).await;
    // The reviewer never approves; the worker's fixes change nothing.
    std::fs::write(s.cli_dir.join("picky"), "").unwrap();
    std::fs::write(s.cli_dir.join("lazy-fix"), "").unwrap();
    let mut new = new_run(&s.project_id, false);
    new.limits = Some(repo::Limits {
        max_rounds: 50,
        max_invocations: 900,
        ..Default::default()
    });
    let run = s.engine.start_run(new).await.unwrap();
    idle(&s.engine, &run.id).await;

    let detail = s.engine.detail(&run.id).unwrap();
    let a = &detail.tasks[0];
    assert_eq!(a.status, task_status::NEEDS_HUMAN, "{a:?}");
    assert!(
        a.status_note
            .as_deref()
            .unwrap()
            .contains("không thay đổi gì"),
        "{:?}",
        a.status_note
    );
    let steps: Vec<&str> = detail
        .invocations
        .iter()
        .filter(|i| i.task_id.as_deref() == Some(a.id.as_str()))
        .map(|i| i.step.as_str())
        .collect();
    assert_eq!(steps, vec!["implement", "review", "fix"]);
    // The fix resumed the worker's session: only its own share is counted.
    let fix = detail.invocations.iter().find(|i| i.step == "fix").unwrap();
    assert_eq!(fix.session_id.as_deref(), Some("w-a"));
    assert!((fix.cost_usd.unwrap() - 0.02).abs() < 1e-9, "{fix:?}");
    assert_eq!((fix.input_tokens, fix.output_tokens), (0, 0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pr_opened_by_hand_is_attached_and_followed_up_on_its_branch() {
    use std::os::unix::fs::PermissionsExt;
    let s = setup(&[]).await;
    let remote = s.cli_dir.join("remote.git");
    git::git(&s.cli_dir, &["init", "-q", "--bare", "remote.git"])
        .await
        .unwrap();
    git::git(
        &s.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    )
    .await
    .unwrap();
    // gh answers `pr view` like GitHub; `pr create` must never be called.
    let gh = s.cli_dir.join("gh");
    std::fs::write(
        &gh,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$(dirname \"$0\")/gh-args\"\ncase \"$*\" in *'pr view'*) echo '{\"headRefName\":\"feat/manual\",\"baseRefName\":\"main\"}';; *) exit 1;; esac\n",
    )
    .unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();

    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    // The user makes their own branch from the work, adds to it, pushes.
    git::git(&s.repo, &["branch", "feat/manual", &run.integration_branch])
        .await
        .unwrap();
    let side = s.cli_dir.join("side");
    git::git(
        &s.repo,
        &[
            "worktree",
            "add",
            "-q",
            &side.to_string_lossy(),
            "feat/manual",
        ],
    )
    .await
    .unwrap();
    std::fs::write(side.join("manual.txt"), "by hand\n").unwrap();
    git::commit_all(&side, "manual").await.unwrap();
    git::git(&side, &["push", "-q", "origin", "feat/manual"])
        .await
        .unwrap();
    let manual = git::commit_of(&side, "HEAD").await.unwrap();

    let bad = AttachPr {
        repo: ".".into(),
        url: "https://example.com/pr/1".into(),
        branch: None,
        base: None,
    };
    assert!(s.engine.attach_pull_request(&run.id, bad).await.is_err());
    let done = s
        .engine
        .attach_pull_request(
            &run.id,
            AttachPr {
                repo: ".".into(),
                url: "https://github.com/o/r/pull/9/files".into(),
                branch: None,
                base: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (done.stage.as_str(), done.status.as_str()),
        (stage::DONE, run_status::KEPT)
    );
    assert_eq!(done.pull_requests[0].branch, "feat/manual");
    assert_eq!(done.pull_requests[0].url, "https://github.com/o/r/pull/9");

    // The follow-up starts from the PR's branch and, when asked, pushes onto
    // it by itself once its work passes.
    s.engine
        .follow_up(&run.id, "Reviewer: thêm c".into(), true)
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        (detail.run.stage.as_str(), detail.run.status.as_str()),
        (stage::DONE, run_status::KEPT),
        "{:?}",
        detail.run.status_note
    );
    assert!(detail.run.followups[0].push_pr);
    assert_eq!(detail.run.repos[0].base_commit, manual);
    assert_eq!(detail.run.pull_requests.len(), 1);
    let pushed = git::git(&remote, &["log", "--oneline", "feat/manual"])
        .await
        .unwrap();
    assert!(
        pushed.contains("manual") && pushed.contains("merge c"),
        "{pushed}"
    );
    let args = std::fs::read_to_string(s.cli_dir.join("gh-args")).unwrap();
    assert!(!args.contains("pr create"), "{args}");
}

const FAKE_GH: &str = r#"#!/bin/sh
d=$(dirname "$0")
printf '%s\n' "$*" >> "$d/gh-args"
case "$*" in
  *"pr view"*"--json state"*)
    echo '{"state":"OPEN","reviewDecision":"CHANGES_REQUESTED","headRefName":"feat/manual","baseRefName":"main","author":{"login":"me"},"reviews":[],"files":'"$(cat "$d/gh-files")"'}';;
  *"pr view"*)
    echo '{"headRefName":"feat/manual","baseRefName":"main"}';;
  *"api graphql"*)
    echo '{"data":{"repository":{"pullRequest":{"author":{"login":"me"},"reviewThreads":{"nodes":[
      {"isResolved":false,"comments":{"nodes":[{"author":{"login":"rv"},"body":"Rename x","path":"a.txt","line":1,"url":"u"}]}},
      {"isResolved":false,"comments":{"nodes":[{"author":{"login":"rv"},"body":"why?","path":"b.txt","line":2},{"author":{"login":"me"},"body":"because","path":"b.txt","line":2}]}},
      {"isResolved":true,"comments":{"nodes":[{"author":{"login":"rv"},"body":"old","path":"a.txt","line":1}]}}
    ]}}}}}';;
  *) exit 1;;
esac
"#;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn verify_pr_checks_it_is_the_runs_and_follows_up_open_comments() {
    use std::os::unix::fs::PermissionsExt;
    let s = setup(&[]).await;
    let remote = s.cli_dir.join("remote.git");
    git::git(&s.cli_dir, &["init", "-q", "--bare", "remote.git"])
        .await
        .unwrap();
    git::git(
        &s.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    )
    .await
    .unwrap();
    let gh = s.cli_dir.join("gh");
    std::fs::write(&gh, FAKE_GH).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();

    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    // The user pushed the work as feat/manual and opened the PR by hand.
    git::git(
        &s.repo,
        &[
            "push",
            "-q",
            "origin",
            &format!("{}:refs/heads/feat/manual", run.integration_branch),
        ],
    )
    .await
    .unwrap();
    git::git(&s.repo, &["push", "-q", "origin", "main"])
        .await
        .unwrap();
    std::fs::write(s.cli_dir.join("gh-files"), r#"[{"path":"x.txt"}]"#).unwrap();
    s.engine
        .attach_pull_request(
            &run.id,
            AttachPr {
                repo: ".".into(),
                url: "https://github.com/o/r/pull/9".into(),
                branch: None,
                base: None,
            },
        )
        .await
        .unwrap();

    // A PR with other files: reported, nothing started.
    let found = s.engine.verify_pull_requests(&run.id).await.unwrap();
    let pr = &found.prs[0];
    assert!(pr.error.is_none(), "{:?}", pr.error);
    assert_eq!((pr.run_files, pr.matched, pr.pr_files), (2, 0, 1));
    assert!(pr.mismatch);
    assert!(!found.followed_up);
    assert!(found.note.as_deref().unwrap().contains("gắn nhầm"));
    // Only the thread nobody answered is open.
    assert_eq!(pr.open_comments.len(), 1);
    assert!(pr.open_comments[0].body.contains("Rename x"));
    assert_eq!(found.review.as_ref().unwrap().verdict, "approved");
    assert_eq!(s.engine.detail(&run.id).unwrap().run.stage, stage::DONE);

    // The right PR: its open comment becomes a follow-up.
    std::fs::write(
        s.cli_dir.join("gh-files"),
        r#"[{"path":"a.txt"},{"path":"b.txt"}]"#,
    )
    .unwrap();
    let found = s.engine.verify_pull_requests(&run.id).await.unwrap();
    assert_eq!((found.prs[0].matched, found.prs[0].mismatch), (2, false));
    assert!(found.followed_up);
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert!(detail.run.followups[0]
        .request
        .contains("[rv] a.txt:1: rv: Rename x"));
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    assert!(detail.verification.unwrap().followed_up);
    // The leader read the PR's code in a worktree that is gone now.
    let verify = detail
        .invocations
        .iter()
        .find(|i| i.step == "verify")
        .unwrap();
    assert_eq!(verify.ok, Some(true));
    assert!(!s
        .env
        .data_dir()
        .join("agents/wt")
        .join(&run.id[..8])
        .join("_verify")
        .exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_kept_run_pushes_what_its_pr_lacks() {
    use std::os::unix::fs::PermissionsExt;
    let s = setup(&[]).await;
    let remote = s.cli_dir.join("remote.git");
    git::git(&s.cli_dir, &["init", "-q", "--bare", "remote.git"])
        .await
        .unwrap();
    git::git(
        &s.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    )
    .await
    .unwrap();
    let gh = s.cli_dir.join("gh");
    std::fs::write(&gh, FAKE_GH).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();

    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    git::git(
        &s.repo,
        &[
            "push",
            "-q",
            "origin",
            &format!("{}:refs/heads/feat/manual", run.integration_branch),
        ],
    )
    .await
    .unwrap();
    s.engine
        .attach_pull_request(
            &run.id,
            AttachPr {
                repo: ".".into(),
                url: "https://github.com/o/r/pull/9".into(),
                branch: None,
                base: None,
            },
        )
        .await
        .unwrap();
    // A follow-up reviewed at gate 2, then kept instead of pushed.
    s.engine
        .follow_up(&run.id, "Reviewer: thêm c".into(), false)
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    s.engine.keep_branch(&run.id).await.unwrap();

    let pending = s.engine.pending_pushes(&run.id).await.unwrap();
    assert_eq!(pending[0].branch, "feat/manual");
    assert_eq!(pending[0].commits, Some(2));
    let done = s.engine.push_to_pull_requests(&run.id).await.unwrap();
    assert_eq!(
        (done.stage.as_str(), done.status.as_str()),
        (stage::DONE, run_status::KEPT)
    );
    let pushed = git::git(&remote, &["log", "--oneline", "feat/manual"])
        .await
        .unwrap();
    assert!(pushed.contains("merge c"), "{pushed}");
    assert_eq!(
        s.engine.pending_pushes(&run.id).await.unwrap()[0].commits,
        Some(0)
    );
    // Nothing new: pushing again is harmless.
    s.engine.push_to_pull_requests(&run.id).await.unwrap();
    let args = std::fs::read_to_string(s.cli_dir.join("gh-args")).unwrap();
    assert!(!args.contains("pr create"), "{args}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pr_branch_behind_its_base_is_rebased_and_pushed_with_a_lease() {
    use std::os::unix::fs::PermissionsExt;
    let s = setup(&[]).await;
    let remote = s.cli_dir.join("remote.git");
    git::git(&s.cli_dir, &["init", "-q", "--bare", "remote.git"])
        .await
        .unwrap();
    git::git(
        &s.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    )
    .await
    .unwrap();
    git::git(&s.repo, &["push", "-q", "origin", "main"])
        .await
        .unwrap();
    let gh = s.cli_dir.join("gh");
    std::fs::write(&gh, FAKE_GH).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();

    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    git::git(
        &s.repo,
        &[
            "push",
            "-q",
            "origin",
            &format!("{}:refs/heads/feat/manual", run.integration_branch),
        ],
    )
    .await
    .unwrap();
    s.engine
        .attach_pull_request(
            &run.id,
            AttachPr {
                repo: ".".into(),
                url: "https://github.com/o/r/pull/9".into(),
                branch: None,
                base: None,
            },
        )
        .await
        .unwrap();
    // The base moves on, and the repo's hook now refuses branches that are
    // behind it or carry merge commits (the run merges its tasks with them).
    std::fs::write(s.repo.join("later.txt"), "x\n").unwrap();
    git::commit_all(&s.repo, "later on main").await.unwrap();
    git::git(&s.repo, &["push", "-q", "origin", "main"])
        .await
        .unwrap();
    let hook = s.repo.join(".git/hooks/pre-push");
    std::fs::write(
        &hook,
        "#!/bin/sh\nbase=$(git rev-parse refs/remotes/origin/main)\nwhile read l lsha r rsha; do\n  git merge-base --is-ancestor $base $lsha || { echo \"pre-push: behind origin/main\"; exit 1; }\n  [ -z \"$(git rev-list --merges $base..$lsha)\" ] || { echo \"pre-push: merge commit\"; exit 1; }\ndone\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();

    let pending = s.engine.pending_pushes(&run.id).await.unwrap();
    assert!(pending[0].needs_rebase, "{pending:?}");
    let done = s.engine.push_to_pull_requests(&run.id).await.unwrap();
    assert_eq!(done.status, run_status::KEPT);
    // The PR branch is now linear on top of the new base, with the work.
    let tip = git::commit_of(&remote, "feat/manual").await.unwrap();
    let main = git::commit_of(&remote, "main").await.unwrap();
    assert!(
        git::git(&remote, &["merge-base", "--is-ancestor", &main, &tip])
            .await
            .is_ok()
    );
    let merges = git::git(
        &remote,
        &["rev-list", "--merges", &format!("{main}..{tip}")],
    )
    .await
    .unwrap();
    assert!(merges.is_empty(), "{merges}");
    let files = git::git(&remote, &["ls-tree", "--name-only", &tip])
        .await
        .unwrap();
    assert!(files.contains("a.txt") && files.contains("b.txt") && files.contains("later.txt"));
    // The run goes on from what the PR has now.
    assert_eq!(
        git::commit_of(&s.repo, &run.integration_branch)
            .await
            .unwrap(),
        tip
    );
    let pending = s.engine.pending_pushes(&run.id).await.unwrap();
    assert_eq!(
        (pending[0].commits, pending[0].needs_rebase),
        (Some(0), false)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ai_push_counts_only_what_origin_shows() {
    use std::os::unix::fs::PermissionsExt;
    let s = setup(&[]).await;
    let remote = s.cli_dir.join("remote.git");
    git::git(&s.cli_dir, &["init", "-q", "--bare", "remote.git"])
        .await
        .unwrap();
    git::git(
        &s.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    )
    .await
    .unwrap();
    git::git(&s.repo, &["push", "-q", "origin", "main"])
        .await
        .unwrap();
    let gh = s.cli_dir.join("gh");
    std::fs::write(&gh, FAKE_GH).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    git::git(
        &s.repo,
        // The PR on origin lacks the run's new work.
        &["push", "-q", "origin", "main:refs/heads/feat/manual"],
    )
    .await
    .unwrap();
    s.engine
        .attach_pull_request(
            &run.id,
            AttachPr {
                repo: ".".into(),
                url: "https://github.com/o/r/pull/9".into(),
                branch: None,
                base: None,
            },
        )
        .await
        .unwrap();
    // The base moves on; the hook wants a rebased, linear branch.
    std::fs::write(s.repo.join("later.txt"), "x\n").unwrap();
    git::commit_all(&s.repo, "later on main").await.unwrap();
    git::git(&s.repo, &["push", "-q", "origin", "main"])
        .await
        .unwrap();
    let hook = s.repo.join(".git/hooks/pre-push");
    std::fs::write(
        &hook,
        "#!/bin/sh\nbase=$(git rev-parse refs/remotes/origin/main)\nwhile read l lsha r rsha; do\n  git merge-base --is-ancestor $base $lsha || { echo \"pre-push: behind\"; exit 1; }\n  [ -z \"$(git rev-list --merges $base..$lsha)\" ] || { echo \"pre-push: merge\"; exit 1; }\ndone\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let request = AiPush {
        cli: "claude".into(),
        model: Some("ship-model".into()),
        effort: None,
        note: None,
    };

    // An agent that says it pushed but did not: not taken for done.
    std::fs::write(s.cli_dir.join("push-lazy"), "").unwrap();
    s.engine.ai_push(&run.id, request.clone()).unwrap();
    idle(&s.engine, &run.id).await;
    let note = s.engine.detail(&run.id).unwrap().run.status_note.unwrap();
    assert!(note.contains("chưa phải bản agent làm ra"), "{note}");

    std::fs::remove_file(s.cli_dir.join("push-lazy")).unwrap();
    s.engine.ai_push(&run.id, request).unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    let note = detail.run.status_note.clone().unwrap();
    assert!(note.starts_with("AI đã push lên PR"), "{note}");
    let tip = git::commit_of(&remote, "feat/manual").await.unwrap();
    let main = git::commit_of(&remote, "main").await.unwrap();
    assert!(
        git::git(&remote, &["merge-base", "--is-ancestor", &main, &tip])
            .await
            .is_ok()
    );
    assert_eq!(
        git::commit_of(&s.repo, &run.integration_branch)
            .await
            .unwrap(),
        tip
    );
    // The agent ran with push rights, the chosen model, and no push block.
    let push = detail
        .invocations
        .iter()
        .rfind(|i| i.step == "push")
        .unwrap();
    assert_eq!(push.ok, Some(true));
    assert!(!git::branch_exists(&s.repo, &format!("cm/{}/ai-push", &run.id[..8])).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_person_tests_by_hand_and_failures_become_fixes() {
    let s = setup(&[]).await;
    let run = s
        .engine
        .start_run(new_run(&s.project_id, true))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    let mut drafts: Vec<TaskDraft> = detail
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
            kind: t.kind.clone(),
        })
        .collect();
    // Added by hand at gate 1: a person tests a and b on the UI.
    let mut qa = TaskDraft {
        key: "qa".into(),
        title: "Test giao diện".into(),
        role: "human".into(),
        cli: "claude".into(),
        description: String::new(),
        acceptance: Vec::new(),
        scope: Vec::new(),
        depends_on: vec!["a".into(), "b".into()],
        repos: vec![".".into()],
        kind: repo::task_kind::HUMAN.into(),
    };
    drafts.push(qa.clone());
    // A person's task needs its description.
    assert!(s
        .engine
        .approve_plan(
            &run.id,
            "D".into(),
            "fn work()".into(),
            drafts.clone(),
            Vec::new()
        )
        .is_err());
    qa.description = "Hướng dẫn test đăng nhập trên UI".into();
    drafts.pop();
    drafts.push(qa);
    s.engine
        .approve_plan(&run.id, "D".into(), "fn work()".into(), drafts, Vec::new())
        .unwrap();
    idle(&s.engine, &run.id).await;

    // The agents' work is merged; the AI prepared the guide; the run waits.
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.status, run_status::WAITING);
    assert!(detail
        .run
        .status_note
        .as_deref()
        .unwrap()
        .contains("Chờ người test: qa"));
    let qa = detail.tasks.iter().find(|t| t.key == "qa").unwrap().clone();
    assert_eq!(qa.status, task_status::AWAITING_HUMAN);
    let guide = qa.human.as_ref().unwrap().guide.clone().unwrap();
    assert_eq!(guide.cases.len(), 2);
    let prepared = std::fs::read_to_string(s.cli_dir.join("prompts-prepare")).unwrap();
    assert!(prepared.contains("Hướng dẫn test đăng nhập trên UI"));
    assert!(prepared.contains("### `a` — Task A"), "{prepared}");

    // Stuck while testing: the leader answers, the second time in its session.
    let answer = s
        .engine
        .human_chat(&qa.id, "Đăng nhập bằng gì?".into())
        .await
        .unwrap();
    assert_eq!(answer.text, "Dùng tài khoản admin");
    s.engine
        .human_chat(&qa.id, "Câu hỏi 2".into())
        .await
        .unwrap();
    // Both in the leader's one session: the first goes on from where the
    // guide was prepared, the second from the first.
    let chat_args = std::fs::read_to_string(s.cli_dir.join("args-chat")).unwrap();
    let calls: Vec<&str> = chat_args.split("-p --output-format").skip(1).collect();
    assert_eq!(calls.len(), 2, "{chat_args}");
    assert!(calls[0].contains("--resume prep"), "{chat_args}");
    assert!(calls[1].contains("--resume chat"), "{chat_args}");

    // C2 fails, with a screenshot.
    let shot = s.engine.human_attach(&qa.id, "lỗi c2.png", b"PNG").unwrap();
    assert_eq!(s.engine.human_file(&qa.id, &shot).unwrap(), b"PNG");
    assert!(s.engine.human_file(&qa.id, "../x").is_err());
    let results = vec![
        repo::CaseResult {
            case_id: "C1".into(),
            pass: Some(true),
            note: String::new(),
        },
        repo::CaseResult {
            case_id: "C2".into(),
            pass: Some(false),
            note: "Không báo lỗi".into(),
        },
    ];
    s.engine
        .submit_human_test(&qa.id, results, false, "C2 hỏng".into(), vec![shot.clone()])
        .unwrap();
    idle(&s.engine, &run.id).await;

    // The leader made a fix task; it was done; the test came back.
    let detail = s.engine.detail(&run.id).unwrap();
    let fix = detail.tasks.iter().find(|t| t.key == "fix-x").unwrap();
    assert_eq!(fix.status, task_status::MERGED);
    assert!(fix.description.contains(&shot), "{}", fix.description);
    let qa = detail.tasks.iter().find(|t| t.key == "qa").unwrap().clone();
    assert_eq!(qa.status, task_status::AWAITING_HUMAN);
    assert!(qa.depends_on.contains(&"fix-x".to_string()));
    let human = qa.human.clone().unwrap();
    assert_eq!(human.rounds.len(), 1);
    assert_eq!(human.rounds[0].fix_tasks, vec!["fix-x"]);
    assert!(human.chat.iter().any(|m| m.text == "Sẽ sửa thông báo lỗi"));
    let triage = std::fs::read_to_string(s.cli_dir.join("prompts-triage")).unwrap();
    assert!(
        triage.contains("[FAIL] C2 Sai mật khẩu: Không báo lỗi"),
        "{triage}"
    );
    let prepared = std::fs::read_to_string(s.cli_dir.join("prompts-prepare")).unwrap();
    assert!(prepared.contains("This is a new round"), "{prepared}");

    // Passed: the run goes on to gate 2.
    s.engine
        .submit_human_test(&qa.id, Vec::new(), true, String::new(), Vec::new())
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    let qa = detail.tasks.iter().find(|t| t.key == "qa").unwrap();
    assert_eq!(qa.status, task_status::PASSED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worktrees_are_reused_and_set_up_once_pushes_included() {
    use std::os::unix::fs::PermissionsExt;
    // Checks and the pre-push hook need what the setup installs.
    let s = setup(&["test -f installed"]).await;
    std::fs::write(s.repo.join(".gitignore"), "installed\n").unwrap();
    git::commit_all(&s.repo, "ignore").await.unwrap();
    let setups = s.cli_dir.join("setups");
    let project = s
        .env
        .db
        .read(|c| repo::project_get(c, &s.project_id))
        .unwrap();
    let mut repos = project.repos.clone();
    repos[0].setup_cmd = Some(format!(
        "basename \"$PWD\" >> {}; touch installed",
        setups.display()
    ));
    s.env
        .db
        .write(|c| repo::project_save(c, &project.id, &ProjectSettings { repos, notes: None }))
        .unwrap();
    let remote = s.cli_dir.join("remote.git");
    git::git(&s.cli_dir, &["init", "-q", "--bare", "remote.git"])
        .await
        .unwrap();
    git::git(
        &s.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    )
    .await
    .unwrap();
    let hook = s.repo.join(".git/hooks/pre-push");
    std::fs::write(&hook, "#!/bin/sh\ntest -f installed\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let gh = s.cli_dir.join("gh");
    std::fs::write(&gh, "#!/bin/sh\necho 'https://github.com/o/r/pull/7'\n").unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();

    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    // b took the slot a left, installed dependencies and all.
    let lines = || std::fs::read_to_string(&setups).unwrap();
    assert_eq!(lines(), "slot-1\n_integration\n");
    assert!(detail.run.integration_checks.as_ref().unwrap()[0].ok);

    let mut draft = s.engine.pr_draft(&run.id).await.unwrap();
    draft.repos[0].branch = "feat/x".into();
    s.engine.open_pull_requests(&run.id, draft).await.unwrap();
    assert!(git::git(&remote, &["show-ref", "refs/heads/feat/x"])
        .await
        .is_ok());

    // A follow-up works in the same slot and pushes from the same leader's
    // worktree: nothing is installed again.
    s.engine
        .follow_up(&run.id, "Reviewer: thêm c".into(), true)
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::DONE,
        "{:?}",
        detail.run.status_note
    );
    let pushed = git::git(&remote, &["log", "--oneline", "feat/x"])
        .await
        .unwrap();
    assert!(pushed.contains("merge c"), "{pushed}");
    assert_eq!(lines(), "slot-1\n_integration\n");

    // A dependency change installs again where it lands.
    let slot = s
        .env
        .data_dir()
        .join("agents/wt")
        .join(&run.id[..8])
        .join("slot-1");
    let integration = slot.with_file_name("_integration");
    std::fs::write(integration.join("package.json"), "{}\n").unwrap();
    git::commit_all(&integration, "deps").await.unwrap();
    s.engine.push_to_pull_requests(&run.id).await.unwrap();
    assert_eq!(lines(), "slot-1\n_integration\n_integration\n");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ai_writes_the_pr_from_the_template_and_the_run() {
    let s = setup(&[]).await;
    std::fs::create_dir(s.repo.join(".github")).unwrap();
    std::fs::write(
        s.repo.join(".github/pull_request_template.md"),
        "## Summary\n## Ticket\n",
    )
    .unwrap();
    git::commit_all(&s.repo, "chore: template").await.unwrap();
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let draft = s.engine.pr_draft(&run.id).await.unwrap();
    let text = s
        .engine
        .pr_ai_draft(&run.id, draft.repos, "Ticket ABC-1".into())
        .await
        .unwrap();
    assert_eq!(text.title, "feat: thêm a và b");
    assert_eq!(text.body, "**Summary**\nThêm a, b");
    let prompt = std::fs::read_to_string(s.cli_dir.join("prompts-describe")).unwrap();
    for part in [
        "pull_request_template.md",
        "## Ticket",
        "cm(a)",
        "- chore: template",
        "a.txt",
        "Ticket ABC-1",
        "Thêm a và b",
    ] {
        assert!(prompt.contains(part), "{part}: {prompt}");
    }
    assert!(s
        .engine
        .pr_ai_draft(&run.id, Vec::new(), String::new())
        .await
        .is_err());
}

/// A repo with an origin, a gh that opens PR 7 and a pre-push hook that
/// refuses until one of the fixes is there.
async fn refusing_remote(s: &Setup) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let remote = s.cli_dir.join("remote.git");
    git::git(&s.cli_dir, &["init", "-q", "--bare", "remote.git"])
        .await
        .unwrap();
    git::git(
        &s.repo,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    )
    .await
    .unwrap();
    let hook = s.repo.join(".git/hooks/pre-push");
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\necho \"$@\" >> {log}\ncat >> {log}\nif [ -f fix-push.txt ] || [ -f self-fix.txt ]; then exit 0; fi\nprintf '\\033[31mFAIL\\033[0m src/x.test.ts > shows the message\\n'\nexit 1\n",
            log = s.cli_dir.join("hook-runs").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let gh = s.cli_dir.join("gh");
    std::fs::write(&gh, "#!/bin/sh\necho 'https://github.com/o/r/pull/7'\n").unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    remote
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_push_goes_to_the_leader_who_hands_out_fixes_and_pushes_again() {
    let s = setup(&[]).await;
    let remote = refusing_remote(&s).await;
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let mut draft = s.engine.pr_draft(&run.id).await.unwrap();
    draft.repos[0].branch = "feat/x".into();
    assert!(s.engine.open_pull_requests(&run.id, draft).await.is_err());
    let failure = s.engine.detail(&run.id).unwrap().push_failure.unwrap();
    assert!(
        failure
            .output
            .contains("FAIL src/x.test.ts > shows the message"),
        "{}",
        failure.output
    );
    assert!(!failure.output.contains('\u{1b}'));

    s.engine.fix_push(&run.id, "xem test X".into()).unwrap();
    idle(&s.engine, &run.id).await;
    // Wait for the driver the fix tasks started.
    tokio::time::sleep(Duration::from_millis(200)).await;
    idle(&s.engine, &run.id).await;
    let prompt = std::fs::read_to_string(s.cli_dir.join("prompts-pushfail")).unwrap();
    assert!(prompt.contains("shows the message") && prompt.contains("xem test X"));
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        (detail.run.stage.as_str(), detail.run.status.as_str()),
        (stage::DONE, run_status::KEPT),
        "{:?}",
        detail.run.status_note
    );
    let fix = detail.tasks.iter().find(|t| t.key == "fix-push").unwrap();
    assert_eq!(fix.status, task_status::MERGED);
    assert!(detail.push_failure.is_none());
    assert_eq!(
        detail.run.pull_requests[0].url,
        "https://github.com/o/r/pull/7"
    );
    let pushed = git::git(&remote, &["log", "--oneline", "feat/x"])
        .await
        .unwrap();
    assert!(pushed.contains("merge fix-push"), "{pushed}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_leader_fixes_a_failed_push_itself_or_says_why_not() {
    let s = setup(&[]).await;
    let remote = refusing_remote(&s).await;
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let mut draft = s.engine.pr_draft(&run.id).await.unwrap();
    draft.repos[0].branch = "feat/x".into();
    assert!(s.engine.open_pull_requests(&run.id, draft).await.is_err());

    // Nothing the leader can do: the failure stays, with its reading.
    std::fs::write(s.cli_dir.join("push-action"), "none").unwrap();
    s.engine.fix_push(&run.id, String::new()).unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.stage, stage::MERGE_GATE);
    assert_eq!(
        detail.run.status_note.as_deref(),
        Some("Leader: Test X hỏng")
    );
    let failure = detail.push_failure.unwrap();
    assert_eq!(failure.diagnosis.as_deref(), Some("Test X hỏng"));
    assert_eq!(failure.action.as_deref(), Some("none"));

    std::fs::write(s.cli_dir.join("push-action"), "self").unwrap();
    s.engine.fix_push(&run.id, String::new()).unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::DONE,
        "{:?}",
        detail.run.status_note
    );
    assert!(detail.push_failure.is_none());
    // A session of its own, in the worktree it writes to.
    let args = std::fs::read_to_string(s.cli_dir.join("args-selffix")).unwrap();
    assert!(!args.contains("--resume"), "{args}");
    let pushed = git::git(&remote, &["log", "--oneline", "feat/x"])
        .await
        .unwrap();
    assert!(pushed.contains("cm(push-fix)"), "{pushed}");
    // The app ran the hook as git would, once per push (git itself skips
    // it), and only then pushed.
    let runs = std::fs::read_to_string(s.cli_dir.join("hook-runs")).unwrap();
    let calls: Vec<&str> = runs.lines().filter(|l| l.starts_with("origin ")).collect();
    assert_eq!(calls.len(), 2, "{runs}");
    assert!(calls[0].ends_with("remote.git"), "{runs}");
    assert!(
        runs.contains(" refs/heads/feat/x 0000000000000000000000000000000000000000"),
        "{runs}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn research_raises_qa_that_a_person_answers_before_the_design() {
    let s = setup(&[]).await;
    std::fs::write(s.cli_dir.join("research-blocker"), "").unwrap();
    let mut new = new_run(&s.project_id, true);
    new.research = true;
    let run = s.engine.start_run(new).await.unwrap();
    idle(&s.engine, &run.id).await;

    // Gate 0: a blocking question stops the run.
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::QA_GATE,
        "{:?}",
        detail.run.status_note
    );
    let report = detail
        .run
        .research
        .as_ref()
        .unwrap()
        .report
        .clone()
        .unwrap();
    assert_eq!(report.qa.len(), 2);
    assert!(s.engine.resume(&run.id).is_err());
    assert!(s
        .engine
        .answer_research(
            &run.id,
            vec![QaAnswer {
                id: "Q2".into(),
                answer: "BD".into()
            }]
        )
        .is_err());

    // Looking again, with what is known: the blocker is gone.
    std::fs::write(s.cli_dir.join("research-again"), "").unwrap();
    s.engine
        .research_again(
            &run.id,
            "API-361 đã merge hôm qua".into(),
            vec![QaAnswer {
                id: "Q2".into(),
                answer: "BD".into(),
            }],
        )
        .unwrap();
    idle(&s.engine, &run.id).await;
    let args = std::fs::read_to_string(s.cli_dir.join("args-research")).unwrap();
    let calls: Vec<&str> = args.split("-p --output-format").skip(1).collect();
    assert_eq!(calls.len(), 2, "{args}");
    assert!(calls[1].contains("--resume research") && calls[1].contains("API-361 đã merge"));
    assert!(calls[1].contains("Answer: BD"), "{args}");
    // Nothing blocks: the design goes on, in the research's session.
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::PLAN_GATE,
        "{:?}",
        detail.run.status_note
    );
    let lead = std::fs::read_to_string(s.cli_dir.join("args-lead")).unwrap();
    assert!(lead.contains("--resume research") && lead.contains("Your research is settled"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_blocker_answered_at_gate_zero_reaches_the_design_and_the_pr() {
    let s = setup(&[]).await;
    std::fs::write(s.cli_dir.join("research-blocker"), "").unwrap();
    let mut new = new_run(&s.project_id, false);
    new.research = true;
    let run = s.engine.start_run(new).await.unwrap();
    idle(&s.engine, &run.id).await;
    s.engine
        .answer_research(
            &run.id,
            vec![QaAnswer {
                id: "Q1".into(),
                answer: "Xong rồi, ở PR #12".into(),
            }],
        )
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
    let lead = std::fs::read_to_string(s.cli_dir.join("args-lead")).unwrap();
    assert!(lead.contains("Answer: Xong rồi, ở PR #12"), "{lead}");
    assert!(
        lead.contains("go on with your assumption: Theo BD"),
        "{lead}"
    );
    let draft = s.engine.pr_draft(&run.id).await.unwrap();
    assert!(draft.body.contains("## QA") && draft.body.contains("→ Xong rồi, ở PR #12"));

    // Chat with the leader: in its one session, kept with the run.
    let answer = s
        .engine
        .leader_chat(&run.id, "Vì sao b chờ a?".into())
        .await
        .unwrap();
    assert_eq!(answer.text, "Vì a phải xong trước b");
    let session = s
        .engine
        .detail(&run.id)
        .unwrap()
        .run
        .leader_session
        .unwrap();
    assert_eq!(session, "lead-chat");
    s.engine
        .leader_chat(&run.id, "Còn gì nữa?".into())
        .await
        .unwrap();
    let args = std::fs::read_to_string(s.cli_dir.join("args-leaderchat")).unwrap();
    let calls: Vec<&str> = args.split("-p --output-format").skip(1).collect();
    assert!(calls[0].contains("--resume lead") && calls[1].contains("--resume lead-chat"));
    let chat = s.engine.detail(&run.id).unwrap().run.leader_chat;
    assert_eq!(chat.len(), 4);
    assert_eq!(
        (chat[0].role.as_str(), chat[1].role.as_str()),
        ("user", "leader")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_repos_commit_hooks_do_not_stop_the_apps_own_merges() {
    use std::os::unix::fs::PermissionsExt;
    let s = setup(&[]).await;
    // A commitlint-like hook that refuses every message the app writes.
    for name in ["commit-msg", "pre-merge-commit"] {
        let hook = s.repo.join(".git/hooks").join(name);
        std::fs::write(&hook, "#!/bin/sh\necho refused >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.stage, stage::MERGE_GATE, "{:?}", detail.tasks);
    assert!(detail.tasks.iter().all(|t| t.status == task_status::MERGED));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_approved_task_whose_merge_failed_is_merged_again_without_a_new_review() {
    let s = setup(&[]).await;
    let run = s
        .engine
        .start_run(new_run(&s.project_id, true))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    // The leader's worktree, made ahead with its index locked: merges fail.
    let short = &run.id[..8];
    let place = s
        .env
        .data_dir()
        .join("agents/wt")
        .join(short)
        .join("_integration");
    std::fs::create_dir_all(place.parent().unwrap()).unwrap();
    git::git(&s.repo, &["branch", &run.integration_branch, "main"])
        .await
        .unwrap();
    git::git(
        &s.repo,
        &[
            "worktree",
            "add",
            &place.to_string_lossy(),
            &run.integration_branch,
        ],
    )
    .await
    .unwrap();
    let lock = git::git_dir(&place).await.unwrap().join("index.lock");
    std::fs::write(&lock, "").unwrap();

    let detail = s.engine.detail(&run.id).unwrap();
    let drafts: Vec<TaskDraft> = detail
        .tasks
        .iter()
        .take(1)
        .map(|t| TaskDraft {
            key: t.key.clone(),
            title: t.title.clone(),
            role: t.role.clone(),
            cli: t.cli.clone(),
            description: t.description.clone(),
            acceptance: t.acceptance.clone(),
            scope: t.scope.clone(),
            depends_on: Vec::new(),
            repos: t.repos.clone(),
            kind: t.kind.clone(),
        })
        .collect();
    s.engine
        .approve_plan(&run.id, "D".into(), "C".into(), drafts, Vec::new())
        .unwrap();
    idle(&s.engine, &run.id).await;
    let a = s.engine.detail(&run.id).unwrap().tasks[0].clone();
    assert_eq!(a.status, task_status::CONFLICT, "{a:?}");
    assert!(a
        .status_note
        .as_deref()
        .unwrap()
        .contains("không phải xung đột code"));
    let reviews = |detail: &super::engine::RunDetail| {
        detail
            .invocations
            .iter()
            .filter(|i| i.step == "review")
            .count()
    };
    let before = reviews(&s.engine.detail(&run.id).unwrap());

    std::fs::remove_file(&lock).unwrap();
    s.engine.retry_task(&a.id).await.unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.tasks[0].status, task_status::MERGED);
    assert_eq!(reviews(&detail), before);
    assert_eq!(
        detail.run.stage,
        stage::MERGE_GATE,
        "{:?}",
        detail.run.status_note
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workers_name_their_commits_the_way_the_repo_does() {
    use std::os::unix::fs::PermissionsExt;
    let s = setup(&[]).await;
    std::fs::write(s.cli_dir.join("commits"), "").unwrap();
    std::fs::write(
        s.repo.join("commitlint.config.js"),
        "module.exports = { extends: ['@commitlint/config-conventional'] };\n",
    )
    .unwrap();
    git::commit_all(&s.repo, "chore: add commitlint")
        .await
        .unwrap();
    // A commitlint stand-in that takes no scope.
    let hook = s.repo.join(".git/hooks/commit-msg");
    std::fs::write(
        &hook,
        "#!/bin/sh\nif grep -q '^feat(' \"$1\"; then echo 'scope-empty: scope must be empty' >&2; exit 1; fi\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();

    let run = s
        .engine
        .start_run(new_run(&s.project_id, false))
        .await
        .unwrap();
    idle(&s.engine, &run.id).await;
    let detail = s.engine.detail(&run.id).unwrap();
    assert_eq!(detail.run.stage, stage::MERGE_GATE, "{:?}", detail.tasks);
    // The worker was told the repo's way.
    let prompt = std::fs::read_to_string(s.cli_dir.join("prompts-a")).unwrap();
    assert!(
        prompt.contains("between <commit> and </commit>"),
        "{prompt}"
    );
    assert!(prompt.contains("config-conventional") && prompt.contains("- chore: add commitlint"));
    // Its message was refused once, then corrected in its session.
    let retry = std::fs::read_to_string(s.cli_dir.join("prompts-commit")).unwrap();
    assert!(retry.contains("feat(a): add a.txt") && retry.contains("scope-empty"));
    let log = git::git(&s.repo, &["log", "--format=%s", &run.integration_branch])
        .await
        .unwrap();
    assert!(log.contains("feat: add the work file"), "{log}");
    assert!(!log.lines().any(|line| line.starts_with("cm(a)")), "{log}");
    // The handoff note does not carry the commit block.
    assert!(!detail.tasks[0]
        .handoff
        .as_deref()
        .unwrap()
        .contains("<commit>"));
}
