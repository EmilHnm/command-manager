//! The transfer queue (SCR-08 §5): every drop becomes a job.
//!
//! A job scans its sources first (so progress has a real total), creates the
//! folders, then copies files a few at a time. Each file is written to
//! `<name>.cm-part` and renamed over the target only when complete, so a
//! cancelled or failed copy never leaves a half-written file under the real
//! name. Moving deletes a source only after its copy succeeded; within one
//! file system a move is a plain rename.

use super::fs::FsHandle;
use super::{EntryKind, Location};
use crate::error::{Error, Result};
use crate::ssh::BoxFuture;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{oneshot, watch, Semaphore};

const CHUNK: usize = 256 * 1024;
const FILES_PER_JOB: usize = 3;
const FILES_TOTAL: usize = 6;
const MAX_ITEMS: usize = 200_000;
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// Items sent with a full snapshot; failed ones are always included.
const SNAPSHOT_ITEMS: usize = 2_000;
pub const PART_SUFFIX: &str = ".cm-part";
/// A retry resumes this far before the end of the `.cm-part`. SFTP keeps
/// up to ~512 KiB of writes in flight, so after a dropped connection the
/// last stretch of the part may have holes; it is written again.
pub const RESUME_MARGIN: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Copy,
    Move,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Endpoint {
    pub location: Location,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictAction {
    Overwrite,
    OverwriteIfNewer,
    Skip,
    KeepBoth,
    CancelJob,
}

/// MOD-20.
#[derive(Debug, Clone, Serialize)]
pub struct ConflictPrompt {
    pub job_id: String,
    pub item_id: u32,
    pub path: String,
    pub source_size: u64,
    pub source_mtime: Option<u64>,
    pub target_size: u64,
    pub target_mtime: Option<u64>,
    /// A folder meets a file (or the other way): only Skip / KeepBoth.
    pub kind_mismatch: bool,
    pub suggested: ConflictAction,
    pub keep_both_name: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ConflictDecision {
    pub action: ConflictAction,
    #[serde(default)]
    pub apply_to_rest: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Scanning,
    Running,
    Done,
    /// Finished with at least one failed item.
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Pending,
    Running,
    Done,
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemSnapshot {
    pub id: u32,
    /// Path relative to the dropped item, e.g. `public/app.js`.
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub done_bytes: u64,
    pub status: ItemStatus,
    pub error: Option<String>,
    /// Bytes kept from an earlier attempt's `.cm-part` (0: from the start).
    pub resumed_from: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobSnapshot {
    pub id: String,
    pub mode: Mode,
    pub sources: Vec<Endpoint>,
    pub dest: Endpoint,
    pub status: JobStatus,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub files_total: usize,
    pub files_done: usize,
    pub files_skipped: usize,
    pub files_failed: usize,
    pub bytes_per_sec: u64,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub items: Vec<ItemSnapshot>,
    pub items_truncated: bool,
    /// Failed while a server it uses was unreachable: worth retrying once
    /// the connection is back (the UI does so with a backoff).
    pub connection_lost: bool,
}

/// What the queue needs from the app.
pub trait TransferEnv: Send + Sync + 'static {
    /// A file system handle (opens the SFTP session when needed).
    fn fs(&self, location: &Location) -> BoxFuture<'static, Result<FsHandle>>;
    fn progress(&self, job: &JobSnapshot);
    /// MOD-20 opens; the answer comes back through [`TransferQueue::resolve`].
    fn conflict(&self, prompt: &ConflictPrompt);
    fn finished(&self, job: &JobSnapshot);
    /// True when there is no live connection to this location right now.
    fn connection_lost(&self, _location: &Location) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    Dir,
    File,
}

struct Item {
    id: u32,
    kind: ItemKind,
    /// Index of the dropped source this item comes from.
    root: usize,
    src: String,
    dst: String,
    rel: String,
    size: u64,
    mtime: Option<u64>,
    mode: Option<u32>,
    status: ItemStatus,
    error: Option<String>,
    done: Arc<AtomicU64>,
    /// A failed copy left its `.cm-part` for a retry to continue.
    partial: Option<Partial>,
    resumed_from: u64,
}

/// Where a failed copy was going, so a retry writes into the same part
/// without asking MOD-20 again.
#[derive(Debug, Clone)]
struct Partial {
    dst: String,
    replace: bool,
}

struct JobState {
    status: JobStatus,
    items: Vec<Item>,
    total_bytes: u64,
    policy: Option<ConflictAction>,
    finished_at: Option<u64>,
    error: Option<String>,
    warnings: Vec<String>,
    connection_lost: bool,
}

pub struct Job {
    id: String,
    mode: Mode,
    sources: Vec<Endpoint>,
    dest: Endpoint,
    started_at: u64,
    state: Mutex<JobState>,
    done_bytes: AtomicU64,
    bytes_per_sec: AtomicU64,
    cancel: watch::Sender<bool>,
    running: AtomicBool,
    /// One MOD-20 at a time per job: files copy in parallel, and "apply to
    /// the rest" must reach the ones waiting behind the open dialog.
    asking: tokio::sync::Mutex<()>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cancelled_error() -> Error {
    Error::msg("Đã hủy")
}

impl Job {
    fn is_cancelled(&self) -> bool {
        *self.cancel.borrow()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, JobState> {
        self.state.lock().expect("transfer job lock")
    }

    fn set_item(&self, index: usize, status: ItemStatus, error: Option<String>) {
        let mut state = self.state();
        let item = &mut state.items[index];
        item.status = status;
        item.error = error;
    }

    pub fn snapshot(&self, full: bool) -> JobSnapshot {
        let state = self.state();
        let files = || {
            state
                .items
                .iter()
                .filter(|item| item.kind == ItemKind::File)
        };
        let count = |status| files().filter(|item| item.status == status).count();
        let item_view = |item: &Item| ItemSnapshot {
            id: item.id,
            name: item.rel.clone(),
            is_dir: item.kind == ItemKind::Dir,
            size: item.size,
            done_bytes: item.done.load(Ordering::Relaxed),
            status: item.status,
            error: item.error.clone(),
            resumed_from: item.resumed_from,
        };
        let wanted = |item: &&Item| {
            if full {
                true
            } else {
                matches!(item.status, ItemStatus::Running)
            }
        };
        let mut items: Vec<ItemSnapshot> = state
            .items
            .iter()
            .filter(wanted)
            .take(SNAPSHOT_ITEMS)
            .map(item_view)
            .collect();
        let mut truncated = false;
        if full && state.items.len() > SNAPSHOT_ITEMS {
            truncated = true;
            items.extend(
                state.items[SNAPSHOT_ITEMS..]
                    .iter()
                    .filter(|item| item.status == ItemStatus::Failed)
                    .map(item_view),
            );
        }
        JobSnapshot {
            id: self.id.clone(),
            mode: self.mode,
            sources: self.sources.clone(),
            dest: self.dest.clone(),
            status: state.status,
            total_bytes: state.total_bytes,
            done_bytes: self.done_bytes.load(Ordering::Relaxed),
            files_total: files().count(),
            files_done: count(ItemStatus::Done),
            files_skipped: count(ItemStatus::Skipped),
            files_failed: count(ItemStatus::Failed),
            bytes_per_sec: self.bytes_per_sec.load(Ordering::Relaxed),
            started_at: self.started_at,
            finished_at: state.finished_at,
            error: state.error.clone(),
            warnings: state.warnings.clone(),
            items,
            items_truncated: truncated,
            connection_lost: state.connection_lost,
        }
    }
}

type PendingConflicts = Mutex<HashMap<(String, u32), oneshot::Sender<ConflictDecision>>>;

pub struct TransferQueue {
    env: Arc<dyn TransferEnv>,
    jobs: Mutex<Vec<Arc<Job>>>,
    pending: PendingConflicts,
    slots: Arc<Semaphore>,
}

impl TransferQueue {
    pub fn new(env: Arc<dyn TransferEnv>) -> Arc<Self> {
        Arc::new(Self {
            env,
            jobs: Mutex::new(Vec::new()),
            pending: Mutex::new(HashMap::new()),
            slots: Arc::new(Semaphore::new(FILES_TOTAL)),
        })
    }

    fn job(&self, id: &str) -> Result<Arc<Job>> {
        self.jobs
            .lock()
            .expect("transfer jobs lock")
            .iter()
            .find(|job| job.id == id)
            .cloned()
            .ok_or_else(|| Error::msg("Không tìm thấy lượt truyền"))
    }

    pub fn list(&self) -> Vec<JobSnapshot> {
        let jobs = self.jobs.lock().expect("transfer jobs lock").clone();
        jobs.iter().map(|job| job.snapshot(false)).collect()
    }

    pub fn get(&self, id: &str) -> Result<JobSnapshot> {
        Ok(self.job(id)?.snapshot(true))
    }

    /// Forgets finished jobs. Returns the `.cm-part` files their failed
    /// items left behind: nothing can resume them any more, so the caller
    /// hands them to [`Self::remove_leftovers`].
    pub fn clear_finished(&self) -> Vec<Endpoint> {
        let mut jobs = self.jobs.lock().expect("transfer jobs lock");
        let (keep, gone): (Vec<_>, Vec<_>) = jobs
            .drain(..)
            .partition(|job| job.running.load(Ordering::SeqCst));
        *jobs = keep;
        gone.iter()
            .flat_map(|job| {
                let state = job.state();
                state
                    .items
                    .iter()
                    .filter_map(|item| item.partial.as_ref())
                    .map(|partial| Endpoint {
                        location: job.dest.location.clone(),
                        path: format!("{}{PART_SUFFIX}", partial.dst),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Deletes leftover parts. Servers that are not connected right now are
    /// left alone: connecting could open a login dialog nobody expects.
    pub async fn remove_leftovers(&self, parts: Vec<Endpoint>) {
        for part in parts {
            if part.location.is_remote() && self.env.connection_lost(&part.location) {
                continue;
            }
            if let Ok(fs) = self.env.fs(&part.location).await {
                let _ = fs.remove_file(&part.path).await;
            }
        }
    }

    /// Queues a transfer and returns its id; the work runs in the background.
    pub fn start(
        self: &Arc<Self>,
        sources: Vec<Endpoint>,
        dest: Endpoint,
        mode: Mode,
    ) -> Result<String> {
        let first = sources
            .first()
            .ok_or_else(|| Error::msg("Chưa chọn mục nào để truyền"))?;
        if sources
            .iter()
            .any(|source| source.location != first.location)
        {
            return Err(Error::msg("Các mục được kéo phải cùng một nơi"));
        }
        dest.location.check_absolute(&dest.path)?;
        for source in &sources {
            source.location.check_absolute(&source.path)?;
            if source.location == dest.location {
                if source.location.is_within(&dest.path, &source.path) {
                    return Err(Error::msg(format!(
                        "Không thể chép thư mục vào chính nó ({})",
                        source.path
                    )));
                }
                let parent = source.location.parent(&source.path);
                let already_there = parent.as_deref() == Some(dest.path.trim_end_matches('/'))
                    || parent.as_deref() == Some(dest.path.as_str());
                if already_there && mode == Mode::Move {
                    return Err(Error::msg("Mục đã nằm sẵn trong thư mục đích"));
                }
            }
        }

        let (cancel, _) = watch::channel(false);
        let job = Arc::new(Job {
            id: uuid::Uuid::new_v4().to_string(),
            mode,
            sources,
            dest,
            started_at: now(),
            state: Mutex::new(JobState {
                status: JobStatus::Scanning,
                items: Vec::new(),
                total_bytes: 0,
                policy: None,
                finished_at: None,
                error: None,
                warnings: Vec::new(),
                connection_lost: false,
            }),
            done_bytes: AtomicU64::new(0),
            bytes_per_sec: AtomicU64::new(0),
            cancel,
            running: AtomicBool::new(true),
            asking: tokio::sync::Mutex::new(()),
        });
        self.jobs
            .lock()
            .expect("transfer jobs lock")
            .push(Arc::clone(&job));
        let id = job.id.clone();
        let queue = Arc::clone(self);
        tokio::spawn(async move { queue.drive(job, false).await });
        Ok(id)
    }

    pub fn cancel(&self, id: &str) -> Result<()> {
        let job = self.job(id)?;
        job.cancel.send_replace(true);
        // Unblock a conflict dialog that is still open for this job.
        let waiting: Vec<_> = {
            let mut pending = self.pending.lock().expect("transfer pending lock");
            let keys: Vec<_> = pending
                .keys()
                .filter(|(job, _)| job == id)
                .cloned()
                .collect();
            keys.into_iter()
                .filter_map(|key| pending.remove(&key))
                .collect()
        };
        for tx in waiting {
            let _ = tx.send(ConflictDecision {
                action: ConflictAction::CancelJob,
                apply_to_rest: false,
            });
        }
        Ok(())
    }

    /// Runs the failed items of a finished job again.
    pub fn retry(self: &Arc<Self>, id: &str) -> Result<()> {
        let job = self.job(id)?;
        if job.running.swap(true, Ordering::SeqCst) {
            return Err(Error::msg("Lượt truyền vẫn đang chạy"));
        }
        {
            let mut state = job.state();
            let mut lost = 0;
            for item in state.items.iter_mut() {
                if matches!(item.status, ItemStatus::Failed | ItemStatus::Cancelled) {
                    lost += item.done.swap(0, Ordering::Relaxed);
                    item.status = ItemStatus::Pending;
                    item.error = None;
                }
            }
            job.done_bytes.fetch_sub(lost, Ordering::Relaxed);
            state.status = JobStatus::Running;
            state.error = None;
            state.finished_at = None;
            state.connection_lost = false;
        }
        job.cancel.send_replace(false);
        let queue = Arc::clone(self);
        tokio::spawn(async move { queue.drive(job, true).await });
        Ok(())
    }

    pub fn resolve(&self, job_id: &str, item_id: u32, decision: ConflictDecision) -> Result<()> {
        let tx = self
            .pending
            .lock()
            .expect("transfer pending lock")
            .remove(&(job_id.to_string(), item_id))
            .ok_or_else(|| Error::msg("Câu hỏi xung đột đã hết hạn"))?;
        let _ = tx.send(decision);
        Ok(())
    }

    async fn drive(self: Arc<Self>, job: Arc<Job>, retry: bool) {
        let ticker = {
            let env = Arc::clone(&self.env);
            let job = Arc::clone(&job);
            tokio::spawn(async move {
                let mut last = (Instant::now(), job.done_bytes.load(Ordering::Relaxed));
                loop {
                    tokio::time::sleep(PROGRESS_EVERY).await;
                    let done = job.done_bytes.load(Ordering::Relaxed);
                    let elapsed = last.0.elapsed().as_secs_f64();
                    if elapsed >= 1.0 {
                        let rate = (done.saturating_sub(last.1)) as f64 / elapsed;
                        job.bytes_per_sec.store(rate as u64, Ordering::Relaxed);
                        last = (Instant::now(), done);
                    }
                    env.progress(&job.snapshot(false));
                }
            })
        };

        let outcome = self.run(&job, retry).await;
        ticker.abort();
        let lost = std::iter::once(&job.dest.location)
            .chain(job.sources.iter().map(|source| &source.location))
            .any(|location| location.is_remote() && self.env.connection_lost(location));

        {
            let mut state = job.state();
            let cancelled = job.is_cancelled();
            if cancelled {
                for item in state.items.iter_mut() {
                    if matches!(item.status, ItemStatus::Pending | ItemStatus::Running) {
                        item.status = ItemStatus::Cancelled;
                    }
                }
            }
            let failed = state
                .items
                .iter()
                .any(|item| item.status == ItemStatus::Failed);
            state.status = match &outcome {
                _ if cancelled => JobStatus::Cancelled,
                Err(error) => {
                    state.error = Some(error.to_string());
                    JobStatus::Failed
                }
                Ok(()) if failed => JobStatus::Failed,
                Ok(()) => JobStatus::Done,
            };
            state.finished_at = Some(now());
            state.connection_lost = state.status == JobStatus::Failed && lost;
        }
        job.bytes_per_sec.store(0, Ordering::Relaxed);
        job.running.store(false, Ordering::SeqCst);
        self.env.finished(&job.snapshot(true));
    }

    async fn run(self: &Arc<Self>, job: &Arc<Job>, retry: bool) -> Result<()> {
        let src_location = job.sources[0].location.clone();
        let dst_location = job.dest.location.clone();
        let src_fs = self.env.fs(&src_location).await?;
        let dst_fs = if src_location == dst_location {
            src_fs.clone()
        } else {
            self.env.fs(&dst_location).await?
        };

        if !retry {
            match dst_fs.stat(&job.dest.path).await? {
                Some(stat) if stat.kind == EntryKind::Dir => {}
                Some(_) => return Err(Error::msg("Đích thả không phải là thư mục")),
                None => return Err(Error::msg("Thư mục đích không còn tồn tại")),
            }
            let mut to_scan = Vec::new();
            for (root, source) in job.sources.iter().enumerate() {
                if job.mode == Mode::Move
                    && src_location == dst_location
                    && self.move_by_rename(job, &src_fs, root, source).await?
                {
                    continue;
                }
                to_scan.push(root);
            }
            self.scan(job, &src_fs, &to_scan).await?;
            job.state().status = JobStatus::Running;
        }

        self.make_dirs(job, &dst_fs).await?;
        self.copy_files(job, &src_fs, &dst_fs).await;
        if job.mode == Mode::Move && !job.is_cancelled() {
            self.remove_moved_dirs(job, &src_fs).await;
        }
        Ok(())
    }

    /// Same file system: a move is a rename. Returns false when the rename
    /// is not possible (another device), so the caller copies instead.
    async fn move_by_rename(
        self: &Arc<Self>,
        job: &Arc<Job>,
        fs: &FsHandle,
        root: usize,
        source: &Endpoint,
    ) -> Result<bool> {
        let location = &source.location;
        let name = location.file_name(&source.path);
        let Some(stat) = fs.stat(&source.path).await? else {
            job.state()
                .warnings
                .push(format!("Không còn tồn tại: {}", source.path));
            return Ok(true);
        };
        let mut target = location.join(&job.dest.path, &name);
        let kind = if stat.kind == EntryKind::Dir {
            ItemKind::Dir
        } else {
            ItemKind::File
        };
        let index = Self::push_item(job, kind, root, &source.path, &target, &name, &stat);
        if let Some(existing) = fs.stat(&target).await? {
            match self
                .decide(job, index, &stat, &existing, fs, &target)
                .await?
            {
                Decision::Skip => {
                    job.set_item(index, ItemStatus::Skipped, None);
                    return Ok(true);
                }
                Decision::Replace => fs.remove_all(location, &target).await?,
                Decision::Rename(free) => {
                    target = free;
                    job.state().items[index].dst = target.clone();
                }
            }
        }
        match fs.rename(&source.path, &target).await {
            Ok(()) => {
                job.state().items[index]
                    .done
                    .store(stat.size, Ordering::Relaxed);
                job.done_bytes.fetch_add(
                    if kind == ItemKind::File { stat.size } else { 0 },
                    Ordering::Relaxed,
                );
                job.set_item(index, ItemStatus::Done, None);
                Ok(true)
            }
            // Another disk: copy it instead. The scan lists it again, so
            // this placeholder goes (nothing runs concurrently yet).
            Err(_) if !location.is_remote() => {
                let mut state = job.state();
                if let Some(item) = state.items.pop() {
                    if item.kind == ItemKind::File {
                        state.total_bytes -= item.size;
                    }
                }
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }

    fn push_item(
        job: &Job,
        kind: ItemKind,
        root: usize,
        src: &str,
        dst: &str,
        rel: &str,
        stat: &super::fs::Stat,
    ) -> usize {
        let mut state = job.state();
        let id = state.items.len() as u32;
        if kind == ItemKind::File {
            state.total_bytes += stat.size;
        }
        state.items.push(Item {
            id,
            kind,
            root,
            src: src.to_string(),
            dst: dst.to_string(),
            rel: rel.to_string(),
            size: if kind == ItemKind::File { stat.size } else { 0 },
            mtime: stat.mtime,
            mode: stat.mode,
            status: ItemStatus::Pending,
            error: None,
            done: Arc::new(AtomicU64::new(0)),
            partial: None,
            resumed_from: 0,
        });
        state.items.len() - 1
    }

    /// Lists everything to copy, parents before children. Symlinks are not
    /// followed: they are skipped with a warning.
    async fn scan(self: &Arc<Self>, job: &Arc<Job>, fs: &FsHandle, roots: &[usize]) -> Result<()> {
        for &root in roots {
            let source = &job.sources[root];
            let location = &source.location;
            let name = location.file_name(&source.path);
            let mut stack = vec![(
                source.path.clone(),
                job.dest.location.join(&job.dest.path, &name),
                name,
            )];
            while let Some((src, dst, rel)) = stack.pop() {
                if job.is_cancelled() {
                    return Err(cancelled_error());
                }
                if job.state().items.len() >= MAX_ITEMS {
                    return Err(Error::msg(format!(
                        "Quá nhiều mục (hơn {MAX_ITEMS}); hãy chia nhỏ lượt truyền"
                    )));
                }
                let Some(stat) = fs.stat(&src).await? else {
                    job.state()
                        .warnings
                        .push(format!("Không còn tồn tại: {src}"));
                    continue;
                };
                match stat.kind {
                    EntryKind::Dir => {
                        Self::push_item(job, ItemKind::Dir, root, &src, &dst, &rel, &stat);
                        let children = fs.read_dir(location, &src).await?;
                        // Reverse so the stack pops them in listing order.
                        for child in children.into_iter().rev() {
                            stack.push((
                                child.path,
                                job.dest.location.join(&dst, &child.name),
                                format!("{rel}/{}", child.name),
                            ));
                        }
                    }
                    EntryKind::File => {
                        Self::push_item(job, ItemKind::File, root, &src, &dst, &rel, &stat);
                    }
                    EntryKind::Symlink => {
                        job.state().warnings.push(format!("Bỏ qua symlink: {rel}"))
                    }
                    EntryKind::Other => job
                        .state()
                        .warnings
                        .push(format!("Bỏ qua mục không phải tệp thường: {rel}")),
                }
            }
        }
        Ok(())
    }

    async fn make_dirs(self: &Arc<Self>, job: &Arc<Job>, fs: &FsHandle) -> Result<()> {
        let dirs: Vec<usize> = {
            let state = job.state();
            (0..state.items.len())
                .filter(|&i| {
                    state.items[i].kind == ItemKind::Dir
                        && state.items[i].status == ItemStatus::Pending
                })
                .collect()
        };
        for index in dirs {
            if job.is_cancelled() {
                return Err(cancelled_error());
            }
            let (status, dst) = {
                let state = job.state();
                (state.items[index].status, state.items[index].dst.clone())
            };
            // A skipped parent already marked this one.
            if status != ItemStatus::Pending {
                continue;
            }
            let result = match fs.stat(&dst).await {
                Ok(None) => fs.mkdir(&dst).await,
                // Folder onto folder: merge, no question.
                Ok(Some(existing)) if existing.kind == EntryKind::Dir => Ok(()),
                Ok(Some(existing)) => {
                    let source = self.item_stat(job, index);
                    match self.decide(job, index, &source, &existing, fs, &dst).await {
                        Ok(Decision::Rename(free)) => {
                            self.retarget(job, index, &dst, &free);
                            fs.mkdir(&free).await
                        }
                        Ok(_) => {
                            self.skip_subtree(job, index, &dst);
                            continue;
                        }
                        Err(error) => Err(error),
                    }
                }
                Err(error) => Err(error),
            };
            match result {
                Ok(()) => job.set_item(index, ItemStatus::Done, None),
                Err(error) if job.is_cancelled() => return Err(error),
                Err(error) => {
                    self.fail_subtree(job, index, &dst, &error.to_string());
                }
            }
        }
        Ok(())
    }

    fn item_stat(&self, job: &Job, index: usize) -> super::fs::Stat {
        let state = job.state();
        let item = &state.items[index];
        super::fs::Stat {
            kind: if item.kind == ItemKind::Dir {
                EntryKind::Dir
            } else {
                EntryKind::File
            },
            size: item.size,
            mtime: item.mtime,
            mode: item.mode,
        }
    }

    /// After "keep both" on a folder, its children follow the new name.
    fn retarget(&self, job: &Job, index: usize, old: &str, new: &str) {
        let location = job.dest.location.clone();
        let mut state = job.state();
        for item in state.items.iter_mut().skip(index) {
            if item.dst == old {
                item.dst = new.to_string();
            } else if location.is_within(&item.dst, old) {
                item.dst = format!("{new}{}", &item.dst[old.len()..]);
            }
        }
    }

    fn skip_subtree(&self, job: &Job, index: usize, dst: &str) {
        let location = job.dest.location.clone();
        let mut state = job.state();
        for item in state.items.iter_mut().skip(index) {
            if location.is_within(&item.dst, dst) && item.status == ItemStatus::Pending {
                item.status = ItemStatus::Skipped;
            }
        }
    }

    fn fail_subtree(&self, job: &Job, index: usize, dst: &str, error: &str) {
        let location = job.dest.location.clone();
        let mut state = job.state();
        for item in state.items.iter_mut().skip(index) {
            if location.is_within(&item.dst, dst) && item.status == ItemStatus::Pending {
                item.status = ItemStatus::Failed;
                item.error = Some(error.to_string());
            }
        }
    }

    async fn copy_files(self: &Arc<Self>, job: &Arc<Job>, src_fs: &FsHandle, dst_fs: &FsHandle) {
        let files: Vec<usize> = {
            let state = job.state();
            (0..state.items.len())
                .filter(|&i| {
                    state.items[i].kind == ItemKind::File
                        && state.items[i].status == ItemStatus::Pending
                })
                .collect()
        };
        let per_job = Arc::new(Semaphore::new(FILES_PER_JOB));
        let mut tasks = tokio::task::JoinSet::new();
        for index in files {
            let queue = Arc::clone(self);
            let job = Arc::clone(job);
            let (src_fs, dst_fs) = (src_fs.clone(), dst_fs.clone());
            let per_job = Arc::clone(&per_job);
            let global = Arc::clone(&self.slots);
            tasks.spawn(async move {
                let Ok(_job_slot) = per_job.acquire_owned().await else {
                    return;
                };
                let Ok(_slot) = global.acquire_owned().await else {
                    return;
                };
                if job.is_cancelled() {
                    return;
                }
                job.set_item(index, ItemStatus::Running, None);
                match queue.copy_one(&job, index, &src_fs, &dst_fs).await {
                    Ok(status) => job.set_item(index, status, None),
                    Err(_) if job.is_cancelled() => {
                        job.set_item(index, ItemStatus::Cancelled, None)
                    }
                    Err(error) => job.set_item(index, ItemStatus::Failed, Some(error.to_string())),
                }
            });
        }
        while tasks.join_next().await.is_some() {}
    }

    async fn copy_one(
        self: &Arc<Self>,
        job: &Arc<Job>,
        index: usize,
        src_fs: &FsHandle,
        dst_fs: &FsHandle,
    ) -> Result<ItemStatus> {
        let (src, mut dst, size, mtime, mode, done, partial) = {
            let mut state = job.state();
            let item = &mut state.items[index];
            (
                item.src.clone(),
                item.dst.clone(),
                item.size,
                item.mtime,
                item.mode,
                Arc::clone(&item.done),
                item.partial.take(),
            )
        };
        let mut replace = false;
        let mut offset = 0;
        let resume = match &partial {
            Some(partial) => resume_point(src_fs, dst_fs, &src, &partial.dst, size, mtime).await,
            None => None,
        };
        match (partial, resume) {
            (Some(partial), Some(at)) => {
                dst = partial.dst;
                replace = partial.replace;
                offset = at;
            }
            (partial, _) => {
                // The source changed or the part is gone: start over.
                if let Some(partial) = partial {
                    let _ = dst_fs
                        .remove_file(&format!("{}{PART_SUFFIX}", partial.dst))
                        .await;
                }
                if let Some(existing) = dst_fs.stat(&dst).await? {
                    let source = self.item_stat(job, index);
                    match self
                        .decide(job, index, &source, &existing, dst_fs, &dst)
                        .await?
                    {
                        Decision::Skip => return Ok(ItemStatus::Skipped),
                        Decision::Replace => replace = true,
                        Decision::Rename(free) => dst = free,
                    }
                }
            }
        }
        job.state().items[index].resumed_from = offset;

        let part = format!("{dst}{PART_SUFFIX}");
        let finished = async {
            Self::stream(job, src_fs, dst_fs, &src, &part, offset, &done).await?;
            // Times and permissions are a nicety: a server that refuses
            // them still gets the file.
            let keep_mode = mode.filter(|_| dst_fs_supports_mode(dst_fs));
            let _ = dst_fs.set_attributes(&part, mtime, keep_mode).await;
            // An earlier attempt may have removed it already.
            if replace && dst_fs.stat(&dst).await?.is_some() {
                dst_fs.remove_file(&dst).await?;
            }
            dst_fs.rename(&part, &dst).await
        }
        .await;
        if let Err(error) = finished {
            job.done_bytes
                .fetch_sub(done.swap(0, Ordering::Relaxed), Ordering::Relaxed);
            if job.is_cancelled() {
                let _ = dst_fs.remove_file(&part).await;
            } else {
                // Kept for a retry (the UI retries by itself when the
                // connection dropped); the source is checked again then.
                job.state().items[index].partial = Some(Partial { dst, replace });
            }
            return Err(error);
        }

        if job.mode == Mode::Move {
            src_fs.remove_file(&src).await?;
        }
        Ok(ItemStatus::Done)
    }

    async fn stream(
        job: &Job,
        src_fs: &FsHandle,
        dst_fs: &FsHandle,
        src: &str,
        part: &str,
        offset: u64,
        done: &AtomicU64,
    ) -> Result<()> {
        let (mut reader, mut writer) = if offset > 0 {
            (
                src_fs.open_read_at(src, offset).await?,
                dst_fs.open_write_at(part, offset).await?,
            )
        } else {
            (src_fs.open_read(src).await?, dst_fs.create(part).await?)
        };
        done.store(offset, Ordering::Relaxed);
        job.done_bytes.fetch_add(offset, Ordering::Relaxed);
        let mut buffer = vec![0u8; CHUNK];
        let mut cancel = job.cancel.subscribe();
        loop {
            if job.is_cancelled() {
                return Err(cancelled_error());
            }
            let read = tokio::select! {
                read = reader.read(&mut buffer) => read,
                _ = cancel.changed() => return Err(cancelled_error()),
            }
            .map_err(|error| Error::msg(format!("Lỗi đọc {src}: {error}")))?;
            if read == 0 {
                break;
            }
            writer
                .write_all(&buffer[..read])
                .await
                .map_err(|error| Error::msg(format!("Lỗi ghi {part}: {error}")))?;
            done.fetch_add(read as u64, Ordering::Relaxed);
            job.done_bytes.fetch_add(read as u64, Ordering::Relaxed);
        }
        writer
            .shutdown()
            .await
            .map_err(|error| Error::msg(format!("Lỗi ghi {part}: {error}")))?;
        Ok(())
    }

    /// Asks MOD-20 (or applies "apply to the rest") for one existing target.
    async fn decide(
        self: &Arc<Self>,
        job: &Arc<Job>,
        index: usize,
        source: &super::fs::Stat,
        existing: &super::fs::Stat,
        fs: &FsHandle,
        target: &str,
    ) -> Result<Decision> {
        let location = &job.dest.location;
        let source_dir = source.kind == EntryKind::Dir;
        let target_dir = existing.kind == EntryKind::Dir;
        let kind_mismatch = source_dir != target_dir;
        let newer = match (source.mtime, existing.mtime) {
            (Some(a), Some(b)) => a > b,
            _ => false,
        };
        let _one_question = job.asking.lock().await;
        if job.is_cancelled() {
            return Err(cancelled_error());
        }
        let free = free_name(fs, location, target).await?;

        let policy = job.state().policy;
        let action = match policy {
            Some(action) => action,
            None => {
                let (item_id, path) = {
                    let state = job.state();
                    (state.items[index].id, target.to_string())
                };
                let prompt = ConflictPrompt {
                    job_id: job.id.clone(),
                    item_id,
                    path,
                    source_size: source.size,
                    source_mtime: source.mtime,
                    target_size: existing.size,
                    target_mtime: existing.mtime,
                    kind_mismatch,
                    suggested: if kind_mismatch || !newer {
                        ConflictAction::Skip
                    } else {
                        ConflictAction::OverwriteIfNewer
                    },
                    keep_both_name: location.file_name(&free),
                };
                let (tx, rx) = oneshot::channel();
                self.pending
                    .lock()
                    .expect("transfer pending lock")
                    .insert((job.id.clone(), item_id), tx);
                self.env.conflict(&prompt);
                // A closed dialog (no answer) skips this one file.
                let decision = rx.await.unwrap_or(ConflictDecision {
                    action: ConflictAction::Skip,
                    apply_to_rest: false,
                });
                if decision.apply_to_rest {
                    job.state().policy = Some(decision.action);
                }
                decision.action
            }
        };

        Ok(match action {
            ConflictAction::CancelJob => {
                self.cancel(&job.id)?;
                return Err(cancelled_error());
            }
            ConflictAction::Skip => Decision::Skip,
            ConflictAction::KeepBoth => Decision::Rename(free),
            // Never overwrite across kinds: a folder is not replaced by a file.
            _ if kind_mismatch => Decision::Skip,
            ConflictAction::Overwrite => Decision::Replace,
            ConflictAction::OverwriteIfNewer if newer => Decision::Replace,
            ConflictAction::OverwriteIfNewer => Decision::Skip,
        })
    }

    /// After a move, removes source folders whose content all moved.
    async fn remove_moved_dirs(&self, job: &Job, fs: &FsHandle) {
        let dirs: Vec<String> = {
            let state = job.state();
            state
                .items
                .iter()
                .filter(|item| item.kind == ItemKind::Dir && item.status == ItemStatus::Done)
                .filter(|dir| {
                    state.items.iter().all(|item| {
                        !job.sources[dir.root]
                            .location
                            .is_within(&item.src, &dir.src)
                            || item.status == ItemStatus::Done
                    })
                })
                .map(|dir| dir.src.clone())
                .collect()
        };
        // Deepest first, so each folder is empty by the time it goes.
        for dir in dirs.iter().rev() {
            let _ = fs.remove_dir(dir).await;
        }
    }
}

enum Decision {
    Skip,
    Replace,
    Rename(String),
}

/// Where a retry can continue the part a failed copy of `src` left at
/// `{dst}.cm-part`, or None to start over: the source must still have the
/// size and time it had when scanned, and the part must not be longer.
async fn resume_point(
    src_fs: &FsHandle,
    dst_fs: &FsHandle,
    src: &str,
    dst: &str,
    size: u64,
    mtime: Option<u64>,
) -> Option<u64> {
    mtime?;
    let source = src_fs.stat(src).await.ok()??;
    if source.kind != EntryKind::File || source.size != size || source.mtime != mtime {
        return None;
    }
    let part = dst_fs.stat(&format!("{dst}{PART_SUFFIX}")).await.ok()??;
    if part.kind != EntryKind::File || part.size > size {
        return None;
    }
    Some(part.size.saturating_sub(RESUME_MARGIN))
}

fn dst_fs_supports_mode(fs: &FsHandle) -> bool {
    match fs {
        FsHandle::Remote(_) => true,
        FsHandle::Local => cfg!(unix),
    }
}

/// `name (1).ext`, `name (2).ext`, ... next to `target`.
async fn free_name(fs: &FsHandle, location: &Location, target: &str) -> Result<String> {
    let dir = location.parent(target).unwrap_or_default();
    let name = location.file_name(target);
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name.as_str(), ""),
    };
    for n in 1..1000 {
        let candidate = location.join(&dir, &format!("{stem} ({n}){ext}"));
        if fs.stat(&candidate).await?.is_none() {
            return Ok(candidate);
        }
    }
    Err(Error::msg(format!("Không tìm được tên trống cho {name}")))
}
