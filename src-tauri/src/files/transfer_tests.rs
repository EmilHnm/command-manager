//! Transfers between the local disk and SFTP. The SFTP side is OpenSSH's own
//! `sftp-server` talking over stdin/stdout, serving the same temp folder, so
//! no login is involved but every operation goes through real SFTP.

use super::fs::FsHandle;
use super::transfer::*;
use super::{EntryKind, Location};
use crate::error::Result;
use crate::ssh::BoxFuture;
use russh_sftp::client::SftpSession;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

const SFTP_SERVER: &str = "/usr/lib/openssh/sftp-server";

async fn sftp() -> Option<Arc<SftpSession>> {
    if !Path::new(SFTP_SERVER).exists() {
        eprintln!("skipping: {SFTP_SERVER} not installed");
        return None;
    }
    let mut child = tokio::process::Command::new(SFTP_SERVER)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("sftp-server");
    let stream = tokio::io::join(
        child.stdout.take().expect("stdout"),
        child.stdin.take().expect("stdin"),
    );
    // Keep the process alive for the rest of the test.
    std::mem::forget(child);
    Some(Arc::new(
        SftpSession::new(stream).await.expect("sftp session"),
    ))
}

fn remote() -> Location {
    Location::Ssh {
        host_id: "h".into(),
    }
}

fn p(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

struct Env {
    sftp: Arc<SftpSession>,
    queue: OnceLock<Weak<TransferQueue>>,
    answer: Mutex<Option<ConflictDecision>>,
    prompts: Mutex<Vec<ConflictPrompt>>,
    done: Mutex<Vec<JobSnapshot>>,
    lost: std::sync::atomic::AtomicBool,
}

impl TransferEnv for Env {
    fn fs(&self, location: &Location) -> BoxFuture<'static, Result<FsHandle>> {
        let handle = match location {
            Location::Local => FsHandle::Local,
            Location::Ssh { .. } => FsHandle::Remote(Arc::clone(&self.sftp)),
        };
        Box::pin(async move { Ok(handle) })
    }

    fn progress(&self, _: &JobSnapshot) {}

    fn conflict(&self, prompt: &ConflictPrompt) {
        self.prompts.lock().unwrap().push(prompt.clone());
        let answer = *self.answer.lock().unwrap();
        let queue = self.queue.get().and_then(Weak::upgrade).expect("queue");
        let (job, item) = (prompt.job_id.clone(), prompt.item_id);
        // Answer from another task, as the UI would.
        tokio::spawn(async move {
            if let Some(answer) = answer {
                let _ = queue.resolve(&job, item, answer);
            }
        });
    }

    fn finished(&self, job: &JobSnapshot) {
        self.done.lock().unwrap().push(job.clone());
    }

    fn connection_lost(&self, location: &Location) -> bool {
        location.is_remote() && self.lost.load(std::sync::atomic::Ordering::SeqCst)
    }
}

async fn setup(
    answer: Option<ConflictDecision>,
) -> Option<(tempfile::TempDir, Arc<Env>, Arc<TransferQueue>)> {
    let sftp = sftp().await?;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = Arc::new(Env {
        sftp,
        queue: OnceLock::new(),
        answer: Mutex::new(answer),
        prompts: Mutex::new(Vec::new()),
        done: Mutex::new(Vec::new()),
        lost: std::sync::atomic::AtomicBool::new(false),
    });
    let queue = TransferQueue::new(env.clone());
    env.queue.set(Arc::downgrade(&queue)).ok();
    Some((dir, env, queue))
}

async fn wait(env: &Env, id: &str) -> JobSnapshot {
    for _ in 0..500 {
        if let Some(job) = env.done.lock().unwrap().iter().find(|job| job.id == id) {
            return job.clone();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("transfer {id} did not finish");
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn tree(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                out.push((format!("{rel}/"), String::new()));
                stack.push(path);
            } else {
                out.push((rel, std::fs::read_to_string(&path).unwrap_or_default()));
            }
        }
    }
    out.sort();
    out
}

fn endpoint(location: Location, path: &Path) -> Endpoint {
    Endpoint {
        location,
        path: p(path),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn listing_is_the_same_locally_and_over_sftp() {
    let Some((dir, env, _)) = setup(None).await else {
        return;
    };
    let root = dir.path();
    write(&root.join("b10.txt"), "x");
    write(&root.join("b2.txt"), "xy");
    write(&root.join(".hidden"), "");
    std::fs::create_dir(root.join("sub")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("sub"), root.join("link")).unwrap();

    let local = FsHandle::Local
        .read_dir(&Location::Local, &p(root))
        .await
        .unwrap();
    let remote_list = FsHandle::Remote(env.sftp.clone())
        .read_dir(&remote(), &p(root))
        .await
        .unwrap();
    let view = |entries: &[super::FsEntry]| {
        entries
            .iter()
            .map(|e| (e.name.clone(), e.kind, e.size, e.hidden, e.link_is_dir))
            .collect::<Vec<_>>()
    };
    assert_eq!(view(&local), view(&remote_list));
    let names: Vec<_> = local.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["link", "sub", ".hidden", "b2.txt", "b10.txt"]);
    assert_eq!(local[0].kind, EntryKind::Symlink);
    assert!(local[0].link_is_dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uploads_a_folder_and_keeps_times() {
    let Some((dir, env, queue)) = setup(None).await else {
        return;
    };
    let src = dir.path().join("src/site");
    write(&src.join("index.html"), "<h1>hi</h1>");
    write(&src.join("assets/app.js"), "console.log(1)");
    std::fs::create_dir_all(src.join("empty")).unwrap();
    let old = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    std::fs::File::options()
        .write(true)
        .open(src.join("index.html"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    let dest = dir.path().join("server");
    std::fs::create_dir(&dest).unwrap();

    let id = queue
        .start(
            vec![endpoint(Location::Local, &src)],
            endpoint(remote(), &dest),
            Mode::Copy,
        )
        .unwrap();
    let job = wait(&env, &id).await;
    assert_eq!(job.status, JobStatus::Done, "{job:?}");
    assert_eq!(job.files_total, 2);
    assert_eq!(job.done_bytes, job.total_bytes);
    assert_eq!(tree(&dest.join("site")), tree(&src));
    assert!(src.join("index.html").exists(), "copy keeps the source");
    let copied = std::fs::metadata(dest.join("site/index.html")).unwrap();
    assert_eq!(copied.modified().unwrap(), old);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conflicts_follow_the_answer_and_apply_to_the_rest() {
    let decision = ConflictDecision {
        action: ConflictAction::KeepBoth,
        apply_to_rest: true,
    };
    let Some((dir, env, queue)) = setup(Some(decision)).await else {
        return;
    };
    let src = dir.path().join("src");
    write(&src.join("a.txt"), "new a");
    write(&src.join("b.txt"), "new b");
    let dest = dir.path().join("dest");
    write(&dest.join("a.txt"), "old a");
    write(&dest.join("b.txt"), "old b");

    let id = queue
        .start(
            vec![
                endpoint(remote(), &src.join("a.txt")),
                endpoint(remote(), &src.join("b.txt")),
            ],
            endpoint(Location::Local, &dest),
            Mode::Copy,
        )
        .unwrap();
    let job = wait(&env, &id).await;
    assert_eq!(job.status, JobStatus::Done, "{job:?}");
    // One question; the answer covered the second file too.
    assert_eq!(env.prompts.lock().unwrap().len(), 1);
    // Files run in parallel, so either one may ask.
    let asked = env.prompts.lock().unwrap()[0].keep_both_name.clone();
    assert!(asked == "a (1).txt" || asked == "b (1).txt", "{asked}");
    assert_eq!(
        std::fs::read_to_string(dest.join("a.txt")).unwrap(),
        "old a"
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("a (1).txt")).unwrap(),
        "new a"
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("b (1).txt")).unwrap(),
        "new b"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overwrite_replaces_and_skip_keeps() {
    for (action, expected) in [
        (ConflictAction::Overwrite, "new"),
        (ConflictAction::Skip, "old"),
    ] {
        let decision = ConflictDecision {
            action,
            apply_to_rest: false,
        };
        let Some((dir, env, queue)) = setup(Some(decision)).await else {
            return;
        };
        write(&dir.path().join("src/f.txt"), "new");
        write(&dir.path().join("dest/f.txt"), "old");
        let id = queue
            .start(
                vec![endpoint(Location::Local, &dir.path().join("src/f.txt"))],
                endpoint(remote(), &dir.path().join("dest")),
                Mode::Copy,
            )
            .unwrap();
        let job = wait(&env, &id).await;
        assert_eq!(job.status, JobStatus::Done);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("dest/f.txt")).unwrap(),
            expected,
            "{action:?}"
        );
        assert!(!dir.path().join("dest/f.txt.cm-part").exists());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn move_removes_the_source_across_and_within_file_systems() {
    let Some((dir, env, queue)) = setup(None).await else {
        return;
    };
    let root = dir.path();
    write(&root.join("remote/logs/a.log"), "a");
    write(&root.join("remote/logs/deep/b.log"), "b");
    std::fs::create_dir(root.join("local")).unwrap();

    // Remote -> local: copy, then delete what was copied.
    let id = queue
        .start(
            vec![endpoint(remote(), &root.join("remote/logs"))],
            endpoint(Location::Local, &root.join("local")),
            Mode::Move,
        )
        .unwrap();
    assert_eq!(wait(&env, &id).await.status, JobStatus::Done);
    assert!(!root.join("remote/logs").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("local/logs/deep/b.log")).unwrap(),
        "b"
    );

    // Local -> local: a rename.
    std::fs::create_dir(root.join("archive")).unwrap();
    let id = queue
        .start(
            vec![endpoint(Location::Local, &root.join("local/logs"))],
            endpoint(Location::Local, &root.join("archive")),
            Mode::Move,
        )
        .unwrap();
    assert_eq!(wait(&env, &id).await.status, JobStatus::Done);
    assert!(root.join("archive/logs/a.log").exists());
    assert!(!root.join("local/logs").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_folder_cannot_be_copied_into_itself() {
    let Some((dir, _env, queue)) = setup(None).await else {
        return;
    };
    std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
    let error = queue
        .start(
            vec![endpoint(Location::Local, &dir.path().join("a"))],
            endpoint(Location::Local, &dir.path().join("a/b")),
            Mode::Copy,
        )
        .unwrap_err();
    assert!(error.to_string().contains("chính nó"), "{error}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_leaves_no_partial_file() {
    let Some((dir, env, queue)) = setup(None).await else {
        return;
    };
    let big: PathBuf = dir.path().join("big.bin");
    std::fs::write(&big, vec![7u8; 64 * 1024 * 1024]).unwrap();
    let dest = dir.path().join("dest");
    std::fs::create_dir(&dest).unwrap();
    let id = queue
        .start(
            vec![endpoint(Location::Local, &big)],
            endpoint(remote(), &dest),
            Mode::Copy,
        )
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    queue.cancel(&id).unwrap();
    let job = wait(&env, &id).await;
    if job.files_done == 1 {
        return; // The disk was faster than the cancel; nothing to check.
    }
    assert_eq!(job.status, JobStatus::Cancelled);
    assert!(!dest.join("big.bin.cm-part").exists());
    assert!(!dest.join("big.bin").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_items_can_be_retried() {
    let Some((dir, env, queue)) = setup(None).await else {
        return;
    };
    write(&dir.path().join("src/ok.txt"), "ok");
    write(&dir.path().join("src/later.txt"), "later");
    let dest = dir.path().join("dest");
    std::fs::create_dir(&dest).unwrap();
    // A folder where the file should go makes that one item fail.
    std::fs::create_dir(dest.join("later.txt.cm-part")).unwrap();

    let sources = vec![
        endpoint(Location::Local, &dir.path().join("src/ok.txt")),
        endpoint(Location::Local, &dir.path().join("src/later.txt")),
    ];
    let id = queue
        .start(sources, endpoint(remote(), &dest), Mode::Copy)
        .unwrap();
    let job = wait(&env, &id).await;
    assert_eq!(job.status, JobStatus::Failed, "{job:?}");
    assert_eq!((job.files_done, job.files_failed), (1, 1));
    // The server was reachable: a plain failure, not a lost connection.
    assert!(!job.connection_lost);

    std::fs::remove_dir(dest.join("later.txt.cm-part")).unwrap();
    env.done.lock().unwrap().clear();
    queue.retry(&id).unwrap();
    let job = wait(&env, &id).await;
    assert_eq!(job.status, JobStatus::Done, "{job:?}");
    assert_eq!(
        std::fs::read_to_string(dest.join("later.txt")).unwrap(),
        "later"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failures_while_the_server_is_gone_are_flagged_for_retry() {
    let Some((dir, env, queue)) = setup(None).await else {
        return;
    };
    write(&dir.path().join("src/a.txt"), "a");
    let dest = dir.path().join("dest");
    std::fs::create_dir(&dest).unwrap();
    std::fs::create_dir(dest.join("a.txt.cm-part")).unwrap();
    env.lost.store(true, std::sync::atomic::Ordering::SeqCst);
    let id = queue
        .start(
            vec![endpoint(Location::Local, &dir.path().join("src/a.txt"))],
            endpoint(remote(), &dest),
            Mode::Copy,
        )
        .unwrap();
    let job = wait(&env, &id).await;
    assert_eq!(job.status, JobStatus::Failed);
    assert!(job.connection_lost);
}
