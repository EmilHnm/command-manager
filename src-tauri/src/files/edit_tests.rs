//! Editing a "server" file (OpenSSH's sftp-server over stdin/stdout, serving
//! a temp folder) through a local copy.

use super::edit::{EditEnv, EditInfo, EditStatus, RemoteEdits};
use super::fs::FsHandle;
use super::Location;
use crate::error::Result;
use crate::ssh::BoxFuture;
use russh_sftp::client::SftpSession;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

const SFTP_SERVER: &str = "/usr/lib/openssh/sftp-server";

struct Env {
    sftp: Arc<SftpSession>,
    events: Mutex<Vec<EditInfo>>,
}

impl EditEnv for Env {
    fn fs(&self, _: &Location) -> BoxFuture<'static, Result<FsHandle>> {
        let handle = FsHandle::Remote(Arc::clone(&self.sftp));
        Box::pin(async move { Ok(handle) })
    }

    fn changed(&self, info: &EditInfo) {
        self.events.lock().unwrap().push(info.clone());
    }
}

async fn setup() -> Option<(tempfile::TempDir, Arc<Env>, Arc<RemoteEdits>)> {
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
    let stream = tokio::io::join(child.stdout.take().unwrap(), child.stdin.take().unwrap());
    std::mem::forget(child);
    let sftp = Arc::new(SftpSession::new(stream).await.expect("sftp"));
    let dir = tempfile::tempdir().unwrap();
    let env = Arc::new(Env {
        sftp,
        events: Mutex::new(Vec::new()),
    });
    let edits = RemoteEdits::new(env.clone(), dir.path().join("copies"));
    Some((dir, env, edits))
}

fn remote() -> Location {
    Location::Ssh {
        host_id: "h".into(),
    }
}

async fn wait_for(edits: &RemoteEdits, status: EditStatus) -> EditInfo {
    for _ in 0..100 {
        if let Some(info) = edits.list().into_iter().find(|info| info.status == status) {
            return info;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("no session reached {status:?}: {:?}", edits.list());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saving_the_copy_uploads_it_and_keeps_permissions() {
    let Some((dir, _env, edits)) = setup().await else {
        return;
    };
    let server_file = dir.path().join("deploy.sh");
    std::fs::write(&server_file, "echo old\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&server_file, std::fs::Permissions::from_mode(0o750)).unwrap();
    }

    let local = edits
        .open(remote(), server_file.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&local).unwrap(), "echo old\n");
    // Opening again returns the same copy.
    let again = edits
        .open(remote(), server_file.to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(local, again);

    std::fs::write(&local, "echo new\n").unwrap();
    let info = wait_for(&edits, EditStatus::Synced).await;
    assert_eq!(info.uploads, 1);
    assert_eq!(std::fs::read_to_string(&server_file).unwrap(), "echo new\n");
    assert!(!dir.path().join("deploy.sh.cm-part").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&server_file)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o750);
    }

    edits.stop(&local.to_string_lossy()).await.unwrap();
    assert!(!local.exists());
    assert!(edits.list().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_side_change_is_not_overwritten_until_forced() {
    let Some((dir, _env, edits)) = setup().await else {
        return;
    };
    let server_file = dir.path().join("config.yml");
    std::fs::write(&server_file, "a: 1\n").unwrap();
    let local = edits
        .open(remote(), server_file.to_string_lossy().into_owned())
        .await
        .unwrap();

    // Someone edits the server copy meanwhile.
    std::fs::write(&server_file, "a: 2\n").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&server_file)
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(60))
        .unwrap();

    std::fs::write(&local, "a: 3\n").unwrap();
    let info = wait_for(&edits, EditStatus::Conflict).await;
    assert!(info.error.is_some());
    assert_eq!(std::fs::read_to_string(&server_file).unwrap(), "a: 2\n");

    edits.push(&local.to_string_lossy(), true).await.unwrap();
    assert_eq!(std::fs::read_to_string(&server_file).unwrap(), "a: 3\n");
    assert_eq!(edits.list()[0].status, EditStatus::Synced);
}
