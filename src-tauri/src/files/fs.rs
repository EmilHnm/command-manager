//! The same file operations on the local disk (tokio::fs) and over SFTP.

use super::{sort_entries, EntryKind, FsEntry, Location};
use crate::error::{Error, Result};
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileAttributes, StatusCode};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncWrite};

pub type Reader = Box<dyn AsyncRead + Send + Unpin>;
pub type Writer = Box<dyn AsyncWrite + Send + Unpin>;

/// What `stat` reports, without following a final symlink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stat {
    pub kind: EntryKind,
    pub size: u64,
    pub mtime: Option<u64>,
    pub mode: Option<u32>,
}

#[derive(Clone)]
pub enum FsHandle {
    Local,
    Remote(Arc<SftpSession>),
}

fn local_error(path: &str, error: std::io::Error) -> Error {
    let reason = match error.kind() {
        std::io::ErrorKind::PermissionDenied => "Permission denied".to_string(),
        std::io::ErrorKind::NotFound => "Không tồn tại".to_string(),
        _ => error.to_string(),
    };
    Error::msg(format!("{reason} ({path})"))
}

fn sftp_error(path: &str, error: SftpError) -> Error {
    let reason = match &error {
        SftpError::Status(status) if !status.error_message.is_empty() => {
            status.error_message.clone()
        }
        SftpError::Status(status) => status.status_code.to_string(),
        other => other.to_string(),
    };
    Error::msg(format!("{reason} ({path})"))
}

fn is_missing(error: &SftpError) -> bool {
    matches!(error, SftpError::Status(status) if status.status_code == StatusCode::NoSuchFile)
}

fn unix_secs(time: std::io::Result<SystemTime>) -> Option<u64> {
    time.ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

fn local_kind(file_type: std::fs::FileType) -> EntryKind {
    if file_type.is_symlink() {
        EntryKind::Symlink
    } else if file_type.is_dir() {
        EntryKind::Dir
    } else if file_type.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

fn local_mode(metadata: &std::fs::Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Some(metadata.permissions().mode() & 0o7777)
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

fn remote_kind(attrs: &FileAttributes) -> EntryKind {
    let file_type = attrs.file_type();
    if file_type.is_symlink() {
        EntryKind::Symlink
    } else if file_type.is_dir() {
        EntryKind::Dir
    } else if file_type.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

fn remote_stat(attrs: &FileAttributes) -> Stat {
    Stat {
        kind: remote_kind(attrs),
        size: attrs.size.unwrap_or(0),
        mtime: attrs.mtime.map(u64::from),
        mode: attrs.permissions.map(|mode| mode & 0o7777),
    }
}

impl FsHandle {
    pub async fn home(&self) -> Result<String> {
        match self {
            Self::Local => crate::ssh::known_hosts::home_dir()
                .map(|home| home.to_string_lossy().into_owned())
                .ok_or_else(|| Error::msg("Không tìm thấy thư mục home")),
            Self::Remote(sftp) => sftp
                .canonicalize(".")
                .await
                .map_err(|error| sftp_error(".", error)),
        }
    }

    /// Resolves `.`/`..` and symlinks of a directory the user typed.
    pub async fn canonical(&self, path: &str) -> Result<String> {
        match self {
            Self::Local => tokio::fs::canonicalize(path)
                .await
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| local_error(path, error)),
            Self::Remote(sftp) => sftp
                .canonicalize(path)
                .await
                .map_err(|error| sftp_error(path, error)),
        }
    }

    /// `None` when nothing is there. A symlink is reported as a symlink.
    pub async fn stat(&self, path: &str) -> Result<Option<Stat>> {
        match self {
            Self::Local => match tokio::fs::symlink_metadata(path).await {
                Ok(metadata) => Ok(Some(Stat {
                    kind: local_kind(metadata.file_type()),
                    size: metadata.len(),
                    mtime: unix_secs(metadata.modified()),
                    mode: local_mode(&metadata),
                })),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(local_error(path, error)),
            },
            Self::Remote(sftp) => match sftp.symlink_metadata(path).await {
                Ok(attrs) => Ok(Some(remote_stat(&attrs))),
                Err(error) if is_missing(&error) => Ok(None),
                Err(error) => Err(sftp_error(path, error)),
            },
        }
    }

    /// Every entry of a directory, hidden ones included, sorted.
    pub async fn read_dir(&self, location: &Location, dir: &str) -> Result<Vec<FsEntry>> {
        let mut entries = match self {
            Self::Local => self.read_local_dir(dir).await?,
            Self::Remote(sftp) => Self::read_remote_dir(sftp, location, dir).await?,
        };
        sort_entries(&mut entries);
        Ok(entries)
    }

    async fn read_local_dir(&self, dir: &str) -> Result<Vec<FsEntry>> {
        let mut reader = tokio::fs::read_dir(dir)
            .await
            .map_err(|error| local_error(dir, error))?;
        let mut entries = Vec::new();
        while let Some(entry) = reader
            .next_entry()
            .await
            .map_err(|error| local_error(dir, error))?
        {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            // An entry that vanished or cannot be read is left out rather
            // than failing the whole listing.
            let Ok(metadata) = tokio::fs::symlink_metadata(&path).await else {
                continue;
            };
            let kind = local_kind(metadata.file_type());
            let (link_target, link_is_dir) = if kind == EntryKind::Symlink {
                let target = tokio::fs::read_link(&path)
                    .await
                    .ok()
                    .map(|target| target.to_string_lossy().into_owned());
                let is_dir = tokio::fs::metadata(&path)
                    .await
                    .map(|meta| meta.is_dir())
                    .unwrap_or(false);
                (target, is_dir)
            } else {
                (None, false)
            };
            entries.push(FsEntry {
                hidden: name.starts_with('.'),
                name,
                path: path.to_string_lossy().into_owned(),
                kind,
                size: metadata.len(),
                mtime: unix_secs(metadata.modified()),
                mode: local_mode(&metadata),
                link_target,
                link_is_dir,
            });
        }
        Ok(entries)
    }

    async fn read_remote_dir(
        sftp: &SftpSession,
        location: &Location,
        dir: &str,
    ) -> Result<Vec<FsEntry>> {
        let listing = sftp
            .read_dir(dir)
            .await
            .map_err(|error| sftp_error(dir, error))?;
        let mut entries = Vec::new();
        for entry in listing {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let path = location.join(dir, &name);
            let attrs = entry.metadata();
            let kind = remote_kind(&attrs);
            let (link_target, link_is_dir) = if kind == EntryKind::Symlink {
                let target = sftp.read_link(path.as_str()).await.ok();
                let is_dir = sftp
                    .metadata(path.as_str())
                    .await
                    .map(|attrs| attrs.file_type().is_dir())
                    .unwrap_or(false);
                (target, is_dir)
            } else {
                (None, false)
            };
            entries.push(FsEntry {
                hidden: name.starts_with('.'),
                name,
                path,
                kind,
                size: attrs.size.unwrap_or(0),
                mtime: attrs.mtime.map(u64::from),
                mode: attrs.permissions.map(|mode| mode & 0o7777),
                link_target,
                link_is_dir,
            });
        }
        Ok(entries)
    }

    pub async fn mkdir(&self, path: &str) -> Result<()> {
        match self {
            Self::Local => tokio::fs::create_dir(path)
                .await
                .map_err(|error| local_error(path, error)),
            Self::Remote(sftp) => sftp
                .create_dir(path)
                .await
                .map_err(|error| sftp_error(path, error)),
        }
    }

    /// Fails when `to` exists (SFTP v3 has no overwrite; locally we check
    /// first so both behave the same).
    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        if self.stat(to).await?.is_some() {
            return Err(Error::msg(format!("Đã có mục cùng tên ({to})")));
        }
        match self {
            Self::Local => tokio::fs::rename(from, to)
                .await
                .map_err(|error| local_error(from, error)),
            Self::Remote(sftp) => sftp
                .rename(from, to)
                .await
                .map_err(|error| sftp_error(from, error)),
        }
    }

    pub async fn remove_file(&self, path: &str) -> Result<()> {
        match self {
            Self::Local => tokio::fs::remove_file(path)
                .await
                .map_err(|error| local_error(path, error)),
            Self::Remote(sftp) => sftp
                .remove_file(path)
                .await
                .map_err(|error| sftp_error(path, error)),
        }
    }

    pub async fn remove_dir(&self, path: &str) -> Result<()> {
        match self {
            Self::Local => tokio::fs::remove_dir(path)
                .await
                .map_err(|error| local_error(path, error)),
            Self::Remote(sftp) => sftp
                .remove_dir(path)
                .await
                .map_err(|error| sftp_error(path, error)),
        }
    }

    /// Deletes a file, a symlink (not its target) or a whole directory tree.
    pub fn remove_all<'a>(
        &'a self,
        location: &'a Location,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            match self.stat(path).await? {
                None => Ok(()),
                Some(stat) if stat.kind == EntryKind::Dir => {
                    for entry in self.read_dir(location, path).await? {
                        self.remove_all(location, &entry.path).await?;
                    }
                    self.remove_dir(path).await
                }
                Some(_) => self.remove_file(path).await,
            }
        })
    }

    pub async fn open_read(&self, path: &str) -> Result<Reader> {
        match self {
            Self::Local => tokio::fs::File::open(path)
                .await
                .map(|file| Box::new(file) as Reader)
                .map_err(|error| local_error(path, error)),
            Self::Remote(sftp) => sftp
                .open(path)
                .await
                .map(|file| Box::new(file) as Reader)
                .map_err(|error| sftp_error(path, error)),
        }
    }

    /// Creates or truncates. Call `shutdown()` on the writer to finish it:
    /// for SFTP that closes the handle and surfaces late write errors.
    pub async fn create(&self, path: &str) -> Result<Writer> {
        match self {
            Self::Local => tokio::fs::File::create(path)
                .await
                .map(|file| Box::new(file) as Writer)
                .map_err(|error| local_error(path, error)),
            Self::Remote(sftp) => sftp
                .create(path)
                .await
                .map(|file| Box::new(file) as Writer)
                .map_err(|error| sftp_error(path, error)),
        }
    }

    /// Copies the modification time and permission bits onto a finished
    /// file. setuid/setgid/sticky bits are never carried over.
    pub async fn set_attributes(
        &self,
        path: &str,
        mtime: Option<u64>,
        mode: Option<u32>,
    ) -> Result<()> {
        let mode = mode.map(|mode| mode & 0o777);
        match self {
            Self::Local => {
                let path = path.to_string();
                tokio::task::spawn_blocking(move || -> Result<()> {
                    if let Some(mtime) = mtime {
                        let file = std::fs::File::options()
                            .write(true)
                            .open(&path)
                            .map_err(|error| local_error(&path, error))?;
                        file.set_modified(UNIX_EPOCH + Duration::from_secs(mtime))
                            .map_err(|error| local_error(&path, error))?;
                    }
                    #[cfg(unix)]
                    if let Some(mode) = mode {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                            .map_err(|error| local_error(&path, error))?;
                    }
                    Ok(())
                })
                .await
                .map_err(|error| Error::msg(error.to_string()))?
            }
            Self::Remote(sftp) => {
                let mut attrs = FileAttributes::empty();
                if let Some(mtime) = mtime.and_then(|mtime| u32::try_from(mtime).ok()) {
                    attrs.mtime = Some(mtime);
                    attrs.atime = Some(mtime);
                }
                attrs.permissions = mode;
                if attrs.mtime.is_none() && attrs.permissions.is_none() {
                    return Ok(());
                }
                sftp.set_metadata(path, attrs)
                    .await
                    .map_err(|error| sftp_error(path, error))
            }
        }
    }
}
