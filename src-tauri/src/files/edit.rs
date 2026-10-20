//! Editing a server file with a local app (WinSCP / Termius style).
//!
//! `open` downloads a copy to a temp folder and watches it. When the editor
//! saves (the file stops changing for one tick, since many editors write in
//! several steps), the copy is uploaded back through `<name>.cm-part` and a
//! rename, like a transfer. If the server file changed since it was opened,
//! nothing is overwritten: the session reports a conflict until the user
//! pushes with `force`.

use super::fs::FsHandle;
use super::transfer::PART_SUFFIX;
use super::{check_name, EntryKind, Location};
use crate::error::{Error, Result};
use crate::ssh::BoxFuture;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

const POLL: Duration = Duration::from_millis(1000);
/// A session nobody touched for this long stops and removes its copy.
const IDLE_STOP: Duration = Duration::from_secs(12 * 3600);
const MAX_SIZE: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EditStatus {
    /// Downloaded, nothing saved yet.
    Opened,
    Uploading,
    Synced,
    /// The server copy changed since it was opened; `push(force)` overwrites.
    Conflict,
    Error,
    Stopped,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditInfo {
    pub local_path: String,
    pub location: Location,
    pub remote_path: String,
    pub status: EditStatus,
    pub error: Option<String>,
    /// Unix seconds of the last successful upload.
    pub synced_at: Option<u64>,
    pub uploads: u32,
}

pub trait EditEnv: Send + Sync + 'static {
    fn fs(&self, location: &Location) -> BoxFuture<'static, Result<FsHandle>>;
    fn changed(&self, info: &EditInfo);
}

/// (mtime, size): what tells a saved file apart.
type Signature = (Option<SystemTime>, u64);

struct State {
    status: EditStatus,
    error: Option<String>,
    /// The local file as last uploaded (or downloaded).
    synced_local: Signature,
    /// A change seen on the previous tick, uploaded once it is stable.
    pending: Option<Signature>,
    /// Server mtime when last downloaded / uploaded.
    remote_mtime: Option<u64>,
    synced_at: Option<u64>,
    uploads: u32,
    last_activity: Instant,
}

struct Session {
    location: Location,
    remote_path: String,
    local_path: PathBuf,
    dir: PathBuf,
    state: Mutex<State>,
    stop: watch::Sender<bool>,
    /// One upload at a time per file.
    uploading: tokio::sync::Mutex<()>,
}

impl Session {
    fn info(&self) -> EditInfo {
        let state = self.state.lock().expect("edit state lock");
        EditInfo {
            local_path: self.local_path.to_string_lossy().into_owned(),
            location: self.location.clone(),
            remote_path: self.remote_path.clone(),
            status: state.status,
            error: state.error.clone(),
            synced_at: state.synced_at,
            uploads: state.uploads,
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn local_signature(path: &Path) -> Option<Signature> {
    let meta = tokio::fs::metadata(path).await.ok()?;
    Some((meta.modified().ok(), meta.len()))
}

pub struct RemoteEdits {
    env: Arc<dyn EditEnv>,
    root: PathBuf,
    sessions: Mutex<HashMap<(Location, String), Arc<Session>>>,
}

impl RemoteEdits {
    pub fn new(env: Arc<dyn EditEnv>, root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            env,
            root,
            sessions: Mutex::new(HashMap::new()),
        })
    }

    fn sessions(&self) -> std::sync::MutexGuard<'_, HashMap<(Location, String), Arc<Session>>> {
        self.sessions.lock().expect("edit sessions lock")
    }

    pub fn list(&self) -> Vec<EditInfo> {
        let sessions: Vec<_> = self.sessions().values().cloned().collect();
        sessions.iter().map(|session| session.info()).collect()
    }

    /// Downloads the file and starts watching the copy. Opening a file that
    /// is already being edited returns the same copy (unsaved edits in it
    /// are not thrown away).
    pub async fn open(
        self: &Arc<Self>,
        location: Location,
        remote_path: String,
    ) -> Result<PathBuf> {
        if !location.is_remote() {
            return Err(Error::msg("Chỉ dùng cho tệp trên máy chủ"));
        }
        location.check_absolute(&remote_path)?;
        let key = (location.clone(), remote_path.clone());
        // Bound first: the lock must not be held across the await below.
        let existing = self.sessions().get(&key).cloned();
        if let Some(existing) = existing {
            if tokio::fs::metadata(&existing.local_path).await.is_ok() {
                existing
                    .state
                    .lock()
                    .expect("edit state lock")
                    .last_activity = Instant::now();
                return Ok(existing.local_path.clone());
            }
        }

        let fs = self.env.fs(&location).await?;
        let stat = fs
            .stat(&remote_path)
            .await?
            .ok_or_else(|| Error::msg(format!("Không tồn tại: {remote_path}")))?;
        if stat.kind == EntryKind::Dir {
            return Err(Error::msg("Đây là thư mục"));
        }
        if stat.size > MAX_SIZE {
            return Err(Error::msg(
                "Tệp quá lớn để mở trực tiếp (trên 256 MB); hãy kéo về Files pane local",
            ));
        }
        let name = location.file_name(&remote_path);
        check_name(&name)?;
        let dir = self.root.join(uuid::Uuid::new_v4().to_string());
        tokio::fs::create_dir_all(&dir).await?;
        let local_path = dir.join(&name);
        if let Err(error) = download(&fs, &remote_path, &local_path).await {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return Err(error);
        }
        let signature = local_signature(&local_path)
            .await
            .ok_or_else(|| Error::msg("Không đọc được bản sao vừa tải"))?;

        let (stop, stopped) = watch::channel(false);
        let session = Arc::new(Session {
            location,
            remote_path,
            local_path: local_path.clone(),
            dir,
            state: Mutex::new(State {
                status: EditStatus::Opened,
                error: None,
                synced_local: signature,
                pending: None,
                remote_mtime: stat.mtime,
                synced_at: None,
                uploads: 0,
                last_activity: Instant::now(),
            }),
            stop,
            uploading: tokio::sync::Mutex::new(()),
        });
        if let Some(old) = self.sessions().insert(key, Arc::clone(&session)) {
            old.stop.send_replace(true);
        }
        self.env.changed(&session.info());
        let edits = Arc::clone(self);
        tokio::spawn(async move { edits.watch(session, stopped).await });
        Ok(local_path)
    }

    async fn watch(self: Arc<Self>, session: Arc<Session>, mut stopped: watch::Receiver<bool>) {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(POLL) => {}
                _ = stopped.changed() => break,
            }
            if *stopped.borrow() {
                break;
            }
            let idle = session
                .state
                .lock()
                .expect("edit state lock")
                .last_activity
                .elapsed();
            if idle > IDLE_STOP {
                let _ = self.stop(&session.local_path.to_string_lossy()).await;
                break;
            }
            let Some(now) = local_signature(&session.local_path).await else {
                // Mid-save (some editors delete then recreate): next tick.
                continue;
            };
            let ready = {
                let mut state = session.state.lock().expect("edit state lock");
                if now == state.synced_local {
                    state.pending = None;
                    false
                } else if state.pending == Some(now) {
                    true
                } else {
                    state.pending = Some(now);
                    state.last_activity = Instant::now();
                    false
                }
            };
            if ready {
                let _ = self.upload(&session, false).await;
            }
        }
    }

    /// Uploads the local copy now ("Đẩy lên ngay" / "Ghi đè").
    pub async fn push(&self, local_path: &str, force: bool) -> Result<()> {
        let session = self
            .find(local_path)
            .ok_or_else(|| Error::msg("Tệp này không còn được theo dõi"))?;
        self.upload(&session, force).await
    }

    fn find(&self, local_path: &str) -> Option<Arc<Session>> {
        self.sessions()
            .values()
            .find(|session| session.local_path.to_string_lossy() == local_path)
            .cloned()
    }

    async fn upload(&self, session: &Session, force: bool) -> Result<()> {
        let _one = session.uploading.lock().await;
        let signature = local_signature(&session.local_path)
            .await
            .ok_or_else(|| Error::msg("Bản sao local không còn"))?;
        let set = |status: EditStatus, error: Option<String>| {
            let mut state = session.state.lock().expect("edit state lock");
            state.status = status;
            state.error = error;
        };
        set(EditStatus::Uploading, None);
        self.env.changed(&session.info());

        let result = async {
            let fs = self.env.fs(&session.location).await?;
            let remote = fs.stat(&session.remote_path).await?;
            let known = session.state.lock().expect("edit state lock").remote_mtime;
            if !force {
                if let (Some(remote), Some(known)) = (remote, known) {
                    if remote.mtime != Some(known) {
                        return Ok(false);
                    }
                }
            }
            upload(&fs, &session.local_path, &session.remote_path).await?;
            let after = fs.stat(&session.remote_path).await?;
            let mut state = session.state.lock().expect("edit state lock");
            state.remote_mtime = after.and_then(|stat| stat.mtime);
            Ok::<bool, Error>(true)
        }
        .await;

        {
            let mut state = session.state.lock().expect("edit state lock");
            match &result {
                Ok(true) => {
                    state.status = EditStatus::Synced;
                    state.error = None;
                    state.synced_local = signature;
                    state.pending = None;
                    state.synced_at = Some(now_secs());
                    state.uploads += 1;
                }
                Ok(false) => {
                    state.status = EditStatus::Conflict;
                    state.error =
                        Some("Tệp trên máy chủ đã thay đổi từ lúc mở; chưa ghi đè".into());
                    // Do not ask again until the user saves again or forces.
                    state.synced_local = signature;
                    state.pending = None;
                }
                Err(error) => {
                    state.status = EditStatus::Error;
                    state.error = Some(error.to_string());
                    // Wait for the next save (or "Đẩy lên ngay") instead of
                    // retrying a failing upload every tick.
                    state.synced_local = signature;
                    state.pending = None;
                }
            }
            state.last_activity = Instant::now();
        }
        self.env.changed(&session.info());
        result.map(|_| ())
    }

    /// Stops watching and deletes the local copy.
    pub async fn stop(&self, local_path: &str) -> Result<()> {
        let session = {
            let mut sessions = self.sessions();
            let key = sessions
                .iter()
                .find(|(_, session)| session.local_path.to_string_lossy() == local_path)
                .map(|(key, _)| key.clone());
            key.and_then(|key| sessions.remove(&key))
        };
        let Some(session) = session else {
            return Ok(());
        };
        session.stop.send_replace(true);
        session.state.lock().expect("edit state lock").status = EditStatus::Stopped;
        self.env.changed(&session.info());
        let _ = tokio::fs::remove_dir_all(&session.dir).await;
        Ok(())
    }

    /// Removes every copy (app exit).
    pub async fn stop_all(&self) {
        let paths: Vec<String> = self
            .sessions()
            .values()
            .map(|session| session.local_path.to_string_lossy().into_owned())
            .collect();
        for path in paths {
            let _ = self.stop(&path).await;
        }
    }
}

async fn download(fs: &FsHandle, remote: &str, local: &Path) -> Result<()> {
    let mut reader = fs.open_read(remote).await?;
    let mut writer = tokio::fs::File::create(local).await?;
    tokio::io::copy(&mut reader, &mut writer)
        .await
        .map_err(|error| Error::msg(format!("Lỗi tải {remote}: {error}")))?;
    writer.sync_all().await?;
    Ok(())
}

/// Same safety as a transfer: write `.cm-part`, then swap it in.
async fn upload(fs: &FsHandle, local: &Path, remote: &str) -> Result<()> {
    let part = format!("{remote}{PART_SUFFIX}");
    let result = async {
        let mut reader = tokio::fs::File::open(local).await?;
        let mut writer = fs.create(&part).await?;
        tokio::io::copy(&mut reader, &mut writer)
            .await
            .map_err(|error| Error::msg(format!("Lỗi ghi {remote}: {error}")))?;
        writer
            .shutdown()
            .await
            .map_err(|error| Error::msg(format!("Lỗi ghi {remote}: {error}")))?;
        // Keep the permissions the server file had (scripts stay executable).
        if let Some(existing) = fs.stat(remote).await? {
            let _ = fs.set_attributes(&part, None, existing.mode).await;
            fs.remove_file(remote).await?;
        }
        fs.rename(&part, remote).await
    }
    .await;
    if result.is_err() {
        let _ = fs.remove_file(&part).await;
    }
    result
}
