//! Files pane and transfers (SCR-08): one file API over the local disk and
//! SFTP, and the queue that copies between any two of them.

pub mod fs;
pub mod transfer;
#[cfg(test)]
mod transfer_tests;

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Which file system a path belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Location {
    Local,
    Ssh { host_id: String },
}

impl Location {
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Ssh { .. })
    }

    /// Joins a child name onto a directory. Remote paths are always POSIX;
    /// local ones use the OS separator.
    pub fn join(&self, dir: &str, name: &str) -> String {
        match self {
            Self::Ssh { .. } => {
                if dir.ends_with('/') {
                    format!("{dir}{name}")
                } else {
                    format!("{dir}/{name}")
                }
            }
            Self::Local => std::path::Path::new(dir)
                .join(name)
                .to_string_lossy()
                .into_owned(),
        }
    }

    pub fn parent(&self, path: &str) -> Option<String> {
        match self {
            Self::Ssh { .. } => {
                let trimmed = path.trim_end_matches('/');
                let index = trimmed.rfind('/')?;
                Some(if index == 0 {
                    "/".to_string()
                } else {
                    trimmed[..index].to_string()
                })
            }
            Self::Local => std::path::Path::new(path)
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned()),
        }
    }

    pub fn file_name(&self, path: &str) -> String {
        match self {
            Self::Ssh { .. } => path
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string(),
            Self::Local => std::path::Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    /// Paths reaching the IPC layer must be absolute: nothing is resolved
    /// against a working directory the user cannot see.
    pub fn check_absolute(&self, path: &str) -> Result<()> {
        let absolute = match self {
            Self::Ssh { .. } => path.starts_with('/'),
            Self::Local => std::path::Path::new(path).is_absolute(),
        };
        if path.contains('\0') || !absolute {
            return Err(Error::msg(format!("Đường dẫn không hợp lệ: {path}")));
        }
        Ok(())
    }

    /// `child` is `dir` itself or somewhere inside it.
    pub fn is_within(&self, child: &str, dir: &str) -> bool {
        let sep = if self.is_remote() {
            '/'
        } else {
            std::path::MAIN_SEPARATOR
        };
        let dir = dir.trim_end_matches(sep);
        child == dir || child.starts_with(&format!("{dir}{sep}"))
    }
}

/// A new name typed by the user (rename, new folder).
pub fn check_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\0'])
        || (cfg!(windows) && name.contains(['\\', ':', '*', '?', '"', '<', '>', '|']))
    {
        return Err(Error::msg(format!("Tên không hợp lệ: {name}")));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Dir,
    File,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    pub size: u64,
    /// Unix seconds.
    pub mtime: Option<u64>,
    /// Permission bits (`0o755`), when the file system has them.
    pub mode: Option<u32>,
    /// For symlinks: where it points, and whether that is a directory (so
    /// the pane can open it like one).
    pub link_target: Option<String>,
    pub link_is_dir: bool,
    pub hidden: bool,
}

/// Folders first, then names in natural order (`file2` before `file10`).
pub fn sort_entries(entries: &mut [FsEntry]) {
    entries.sort_by(|a, b| {
        let a_dir = a.kind == EntryKind::Dir || a.link_is_dir;
        let b_dir = b.kind == EntryKind::Dir || b.link_is_dir;
        b_dir
            .cmp(&a_dir)
            .then_with(|| natural_cmp(&a.name, &b.name))
    });
}

pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    // Case only breaks ties, so `file1 < File2 < file10`.
    let mut case_tie = Ordering::Equal;
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        match (a.peek(), b.peek()) {
            (None, None) => return case_tie,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        it.next();
                    }
                    digits
                };
                let (x, y) = (take(&mut a), take(&mut b));
                let (xt, yt) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let order = xt
                    .len()
                    .cmp(&yt.len())
                    .then_with(|| xt.cmp(yt))
                    .then_with(|| x.len().cmp(&y.len()));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(&x), Some(&y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                if case_tie == Ordering::Equal {
                    case_tie = x.cmp(&y);
                }
                a.next();
                b.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote() -> Location {
        Location::Ssh {
            host_id: "h".into(),
        }
    }

    #[test]
    fn remote_paths_are_posix() {
        let r = remote();
        assert_eq!(r.join("/var/www", "app"), "/var/www/app");
        assert_eq!(r.join("/", "etc"), "/etc");
        assert_eq!(r.parent("/var/www/app").as_deref(), Some("/var/www"));
        assert_eq!(r.parent("/var").as_deref(), Some("/"));
        assert_eq!(r.parent("/"), None);
        assert_eq!(r.file_name("/var/www/app/"), "app");
        assert!(r.check_absolute("relative/path").is_err());
        assert!(r.is_within("/var/www/app/x", "/var/www/app"));
        assert!(r.is_within("/var/www/app", "/var/www/app/"));
        assert!(!r.is_within("/var/www/application", "/var/www/app"));
    }

    #[test]
    fn names_cannot_escape_their_folder() {
        for bad in ["", ".", "..", "a/b", "nul\0"] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
        assert!(check_name("report (1).pdf").is_ok());
    }

    #[test]
    fn natural_order_and_folders_first() {
        let entry = |name: &str, kind| FsEntry {
            name: name.into(),
            path: name.into(),
            kind,
            size: 0,
            mtime: None,
            mode: None,
            link_target: None,
            link_is_dir: false,
            hidden: false,
        };
        let mut entries = vec![
            entry("file10", EntryKind::File),
            entry("File2", EntryKind::File),
            entry("zeta", EntryKind::Dir),
            entry("file1", EntryKind::File),
            entry("alpha", EntryKind::Dir),
        ];
        sort_entries(&mut entries);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["alpha", "zeta", "file1", "File2", "file10"]);
    }
}
