//! Files pane and transfers (SCR-08): listing, small file operations and
//! the transfer queue, over the local disk or a host's SFTP subsystem.

use super::events;
use super::ssh::SshState;
use crate::db::repos::ssh_hosts;
use crate::error::{Error, Result};
use crate::files::edit::{EditEnv, EditInfo, RemoteEdits};
use crate::files::fs::FsHandle;
use crate::files::transfer::{
    ConflictAction, ConflictDecision, ConflictPrompt, Endpoint, JobSnapshot, Mode, TransferEnv,
    TransferQueue,
};
use crate::files::{check_name, FsEntry, Location};
use crate::ssh::manager::Lease;
use crate::ssh::BoxFuture;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// An SFTP session not used for this long is closed, releasing its lease
/// so the SSH connection can idle out too.
const SFTP_IDLE: Duration = Duration::from_secs(300);

struct SftpEntry {
    session: Arc<SftpSession>,
    lease: Lease,
    last_used: Instant,
}

pub struct FilesState {
    queue: Arc<TransferQueue>,
    edits: Arc<RemoteEdits>,
    sftp: Arc<tokio::sync::Mutex<HashMap<String, SftpEntry>>>,
}

impl FilesState {
    pub fn new(app: &AppHandle) -> Self {
        let sftp: Arc<tokio::sync::Mutex<HashMap<String, SftpEntry>>> = Arc::default();
        let env = Arc::new(TauriTransferEnv {
            app: app.clone(),
            sftp: Arc::clone(&sftp),
        });
        let sweep = Arc::clone(&sftp);
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                // A session still held by a transfer has other Arc owners.
                sweep.lock().await.retain(|_, entry| {
                    entry.last_used.elapsed() < SFTP_IDLE || Arc::strong_count(&entry.session) > 1
                });
            }
        });
        Self {
            queue: TransferQueue::new(env.clone()),
            edits: RemoteEdits::new(env, std::env::temp_dir().join("command-manager-edit")),
            sftp,
        }
    }
}

struct TauriTransferEnv {
    app: AppHandle,
    sftp: Arc<tokio::sync::Mutex<HashMap<String, SftpEntry>>>,
}

async fn open_sftp(app: &AppHandle, host_id: &str) -> Result<SftpEntry> {
    let host = app
        .state::<super::AppState>()
        .db
        .read(|c| ssh_hosts::require(c, host_id))?;
    if host.use_system_ssh {
        return Err(Error::msg(
            "Máy chủ dùng OpenSSH hệ thống nên không có trình quản lý file",
        ));
    }
    let lease = app.state::<SshState>().manager.lease(host_id).await?;
    let channel = lease
        .handle()
        .channel_open_session()
        .await
        .map_err(|error| Error::msg(format!("Không mở được kênh SFTP: {error}")))?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|error| Error::msg(format!("Máy chủ không hỗ trợ SFTP: {error}")))?;
    let session = SftpSession::new(channel.into_stream())
        .await
        .map_err(|error| Error::msg(format!("Không khởi tạo được SFTP: {error}")))?;
    Ok(SftpEntry {
        session: Arc::new(session),
        lease,
        last_used: Instant::now(),
    })
}

async fn fs_for(
    app: &AppHandle,
    cache: &tokio::sync::Mutex<HashMap<String, SftpEntry>>,
    location: &Location,
) -> Result<FsHandle> {
    let Location::Ssh { host_id } = location else {
        return Ok(FsHandle::Local);
    };
    // Held while opening, so a burst of requests opens one session.
    let mut sessions = cache.lock().await;
    if let Some(entry) = sessions.get_mut(host_id) {
        if !entry.lease.handle().is_closed() {
            entry.last_used = Instant::now();
            return Ok(FsHandle::Remote(Arc::clone(&entry.session)));
        }
        sessions.remove(host_id);
    }
    let entry = open_sftp(app, host_id).await?;
    let handle = FsHandle::Remote(Arc::clone(&entry.session));
    sessions.insert(host_id.clone(), entry);
    Ok(handle)
}

impl EditEnv for TauriTransferEnv {
    fn fs(&self, location: &Location) -> BoxFuture<'static, Result<FsHandle>> {
        TransferEnv::fs(self, location)
    }

    fn changed(&self, info: &EditInfo) {
        let _ = self.app.emit(events::REMOTE_EDIT, info);
    }
}

impl TransferEnv for TauriTransferEnv {
    fn fs(&self, location: &Location) -> BoxFuture<'static, Result<FsHandle>> {
        let app = self.app.clone();
        let cache = Arc::clone(&self.sftp);
        let location = location.clone();
        Box::pin(async move { fs_for(&app, &cache, &location).await })
    }

    fn progress(&self, job: &JobSnapshot) {
        let _ = self.app.emit(events::TRANSFER_PROGRESS, job);
    }

    fn conflict(&self, prompt: &ConflictPrompt) {
        let _ = self.app.emit(events::TRANSFER_CONFLICT, prompt);
    }

    fn finished(&self, job: &JobSnapshot) {
        let _ = self.app.emit(events::TRANSFER_DONE, job);
    }

    fn connection_lost(&self, location: &Location) -> bool {
        let Location::Ssh { host_id } = location else {
            return false;
        };
        // Busy cache: assume the connection is fine rather than wait.
        let Ok(sessions) = self.sftp.try_lock() else {
            return false;
        };
        sessions
            .get(host_id)
            .is_none_or(|entry| entry.lease.handle().is_closed())
    }
}

async fn handle(app: &AppHandle, location: &Location) -> Result<FsHandle> {
    let files = app.state::<FilesState>();
    fs_for(app, &files.sftp, location).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn fs_home(app: AppHandle, location: Location) -> Result<String> {
    handle(&app, &location).await?.home().await
}

#[derive(Serialize)]
pub struct Listing {
    /// The folder as the file system names it (symlinks and `..` resolved).
    path: String,
    parent: Option<String>,
    entries: Vec<FsEntry>,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn fs_list(
    app: AppHandle,
    location: Location,
    path: String,
    show_hidden: Option<bool>,
) -> Result<Listing> {
    location.check_absolute(&path)?;
    let fs = handle(&app, &location).await?;
    let path = fs.canonical(&path).await?;
    let mut entries = fs.read_dir(&location, &path).await?;
    if !show_hidden.unwrap_or(false) {
        entries.retain(|entry| !entry.hidden);
    }
    Ok(Listing {
        parent: location.parent(&path),
        path,
        entries,
    })
}

/// Creates `name` inside `dir` and returns its path.
#[tauri::command(rename_all = "snake_case")]
pub async fn fs_mkdir(
    app: AppHandle,
    location: Location,
    dir: String,
    name: String,
) -> Result<String> {
    location.check_absolute(&dir)?;
    check_name(&name)?;
    let path = location.join(&dir, &name);
    let fs = handle(&app, &location).await?;
    if fs.stat(&path).await?.is_some() {
        return Err(Error::msg(format!("Đã có mục tên \"{name}\"")));
    }
    fs.mkdir(&path).await?;
    Ok(path)
}

/// Renames in place (same folder) and returns the new path.
#[tauri::command(rename_all = "snake_case")]
pub async fn fs_rename(
    app: AppHandle,
    location: Location,
    path: String,
    new_name: String,
) -> Result<String> {
    location.check_absolute(&path)?;
    check_name(&new_name)?;
    let dir = location
        .parent(&path)
        .ok_or_else(|| Error::msg("Không đổi tên được thư mục gốc"))?;
    let target = location.join(&dir, &new_name);
    handle(&app, &location)
        .await?
        .rename(&path, &target)
        .await?;
    Ok(target)
}

/// Deletes files and whole folders (no trash: the UI asks first).
#[tauri::command(rename_all = "snake_case")]
pub async fn fs_delete(
    app: AppHandle,
    location: Location,
    paths: Vec<String>,
    confirmed: bool,
) -> Result<()> {
    if !confirmed {
        return Err(Error::ConfirmationRequired);
    }
    for path in &paths {
        location.check_absolute(path)?;
        if location.parent(path).is_none() {
            return Err(Error::msg("Không xóa thư mục gốc"));
        }
    }
    let fs = handle(&app, &location).await?;
    for path in &paths {
        fs.remove_all(&location, path).await?;
    }
    Ok(())
}

/// Opens a server file with the local default app: downloads a copy and
/// uploads it back on every save (see `files::edit`).
#[tauri::command(rename_all = "snake_case")]
pub async fn fs_open_remote(app: AppHandle, location: Location, path: String) -> Result<String> {
    let edits = Arc::clone(&app.state::<FilesState>().edits);
    let local = edits.open(location, path).await?;
    let local = local.to_string_lossy().into_owned();
    super::commands::open_with_os(&local)?;
    Ok(local)
}

#[tauri::command(rename_all = "snake_case")]
pub fn fs_edit_list(app: AppHandle) -> Vec<EditInfo> {
    app.state::<FilesState>().edits.list()
}

/// "Đẩy lên ngay", or "Ghi đè" after a conflict (`force`).
#[tauri::command(rename_all = "snake_case")]
pub async fn fs_edit_push(app: AppHandle, local_path: String, force: Option<bool>) -> Result<()> {
    let edits = Arc::clone(&app.state::<FilesState>().edits);
    edits.push(&local_path, force.unwrap_or(false)).await
}

/// Stops syncing and deletes the local copy.
#[tauri::command(rename_all = "snake_case")]
pub async fn fs_edit_stop(app: AppHandle, local_path: String) -> Result<()> {
    let edits = Arc::clone(&app.state::<FilesState>().edits);
    edits.stop(&local_path).await
}

#[tauri::command(rename_all = "snake_case")]
pub fn fs_open_local(path: String) -> Result<()> {
    Location::Local.check_absolute(&path)?;
    if !std::path::Path::new(&path).exists() {
        return Err(Error::msg(format!("Không tồn tại: {path}")));
    }
    super::commands::open_with_os(&path)
}

// Async so the job is spawned on the tokio runtime (sync commands run on
// the main thread).
#[tauri::command(rename_all = "snake_case")]
pub async fn transfer_start(
    app: AppHandle,
    sources: Vec<Endpoint>,
    dest: Endpoint,
    mode: Mode,
) -> Result<String> {
    app.state::<FilesState>().queue.start(sources, dest, mode)
}

#[tauri::command(rename_all = "snake_case")]
pub fn transfer_resolve(
    app: AppHandle,
    job_id: String,
    item_id: u32,
    action: ConflictAction,
    apply_to_rest: Option<bool>,
) -> Result<()> {
    app.state::<FilesState>().queue.resolve(
        &job_id,
        item_id,
        ConflictDecision {
            action,
            apply_to_rest: apply_to_rest.unwrap_or(false),
        },
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn transfer_cancel(app: AppHandle, job_id: String) -> Result<()> {
    app.state::<FilesState>().queue.cancel(&job_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn transfer_retry(app: AppHandle, job_id: String) -> Result<()> {
    app.state::<FilesState>().queue.retry(&job_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn transfer_list(app: AppHandle) -> Vec<JobSnapshot> {
    app.state::<FilesState>().queue.list()
}

#[tauri::command(rename_all = "snake_case")]
pub fn transfer_get(app: AppHandle, job_id: String) -> Result<JobSnapshot> {
    app.state::<FilesState>().queue.get(&job_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn transfer_clear_finished(app: AppHandle) {
    app.state::<FilesState>().queue.clear_finished();
}
