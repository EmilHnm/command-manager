//! Host key checks (trust on first use).
//!
//! Two files are consulted: the app's own `known_hosts` (read and written)
//! and the user's `~/.ssh/known_hosts` (read only, so OpenSSH's file is never
//! touched). A key that differs from a recorded one blocks the connection;
//! the user has to forget the old key on purpose.

use crate::error::{Error, Result};
use russh::keys::{self, HashAlg, PublicKey};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Verdict {
    Known,
    Unknown,
    Changed {
        path: PathBuf,
        line: usize,
        old_fingerprint: Option<String>,
    },
}

pub struct KnownHosts {
    app_path: PathBuf,
    user_path: Option<PathBuf>,
}

impl KnownHosts {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            app_path: app_data_dir.join("known_hosts"),
            user_path: home_dir().map(|home| home.join(".ssh").join("known_hosts")),
        }
    }

    #[cfg(test)]
    fn with_paths(app_path: PathBuf, user_path: Option<PathBuf>) -> Self {
        Self {
            app_path,
            user_path,
        }
    }

    pub fn check(&self, host: &str, port: u16, key: &PublicKey) -> Verdict {
        let paths = std::iter::once(self.app_path.as_path()).chain(self.user_path.as_deref());
        for path in paths {
            match keys::check_known_hosts_path(host, port, key, path) {
                Ok(true) => return Verdict::Known,
                Ok(false) => {}
                Err(keys::Error::KeyChanged { line }) => {
                    let old_fingerprint = keys::known_hosts::known_host_keys_path(host, port, path)
                        .ok()
                        .and_then(|keys| {
                            keys.into_iter()
                                .find(|(number, _)| *number == line)
                                .map(|(_, old)| fingerprint(&old))
                        });
                    return Verdict::Changed {
                        path: path.to_path_buf(),
                        line,
                        old_fingerprint,
                    };
                }
                // An unreadable or partly unsupported file says nothing about
                // this host; the other file may still know it.
                Err(_) => {}
            }
        }
        Verdict::Unknown
    }

    pub fn learn(&self, host: &str, port: u16, key: &PublicKey) -> Result<()> {
        keys::known_hosts::learn_known_hosts_path(host, port, key, &self.app_path)
            .map_err(|error| Error::msg(format!("Không ghi được known_hosts: {error}")))
    }

    /// Removes this host's keys from the app's file. Returns how many lines
    /// went away. `~/.ssh/known_hosts` belongs to OpenSSH and is left alone.
    pub fn forget(&self, host: &str, port: u16) -> Result<usize> {
        let matches = keys::known_hosts::known_host_keys_path(host, port, &self.app_path)
            .map_err(|error| Error::msg(format!("Không đọc được known_hosts: {error}")))?;
        if matches.is_empty() {
            return Ok(0);
        }
        let text = std::fs::read_to_string(&self.app_path)?;
        // Same numbering as russh: comment lines are not counted.
        let mut number = 0;
        let mut kept = String::with_capacity(text.len());
        for line in text.split_inclusive('\n') {
            if line.starts_with('#') {
                kept.push_str(line);
                continue;
            }
            number += 1;
            if !matches.iter().any(|(matched, _)| *matched == number) {
                kept.push_str(line);
            }
        }
        std::fs::write(&self.app_path, kept)?;
        Ok(matches.len())
    }
}

pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

pub fn home_dir() -> Option<PathBuf> {
    #[allow(deprecated)]
    std::env::home_dir().filter(|path| !path.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIKYUHyqlCzWrWZus90y547r0iCIZj21h4hS/fGgq5Qsj";
    const KEY_B: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIDtr1KkxB4UqH0A0qjvD/cLDjBiu4PxgoclfLy8wNp0z";

    fn key(b64: &str) -> PublicKey {
        keys::parse_public_key_base64(b64).expect("key")
    }

    #[test]
    fn unknown_then_learned_then_changed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let known = KnownHosts::with_paths(dir.path().join("known_hosts"), None);

        assert_eq!(known.check("srv", 2222, &key(KEY_A)), Verdict::Unknown);
        known.learn("srv", 2222, &key(KEY_A)).expect("learn");
        assert_eq!(known.check("srv", 2222, &key(KEY_A)), Verdict::Known);
        // Port is part of the identity.
        assert_eq!(known.check("srv", 22, &key(KEY_A)), Verdict::Unknown);

        match known.check("srv", 2222, &key(KEY_B)) {
            Verdict::Changed {
                old_fingerprint, ..
            } => {
                // russh starts a fresh file with a newline, so the line
                // number is not always 1; the fingerprint is what matters.
                assert_eq!(old_fingerprint, Some(fingerprint(&key(KEY_A))));
            }
            other => panic!("expected Changed, got {other:?}"),
        }
    }

    #[test]
    fn user_file_is_consulted_but_never_written() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user_known_hosts");
        std::fs::write(&user, format!("# comment\nsrv ssh-ed25519 {KEY_A}\n")).expect("write");
        let known = KnownHosts::with_paths(dir.path().join("app_known_hosts"), Some(user.clone()));

        assert_eq!(known.check("srv", 22, &key(KEY_A)), Verdict::Known);
        assert!(matches!(
            known.check("srv", 22, &key(KEY_B)),
            Verdict::Changed { ref path, .. } if *path == user
        ));
        assert_eq!(known.forget("srv", 22).expect("forget"), 0);
        assert!(std::fs::read_to_string(&user)
            .expect("read")
            .contains(KEY_A));
    }

    #[test]
    fn forget_removes_only_that_host() {
        let dir = tempfile::tempdir().expect("tempdir");
        let known = KnownHosts::with_paths(dir.path().join("known_hosts"), None);
        known.learn("a", 22, &key(KEY_A)).expect("learn");
        known.learn("b", 22, &key(KEY_B)).expect("learn");

        assert_eq!(known.forget("a", 22).expect("forget"), 1);
        assert_eq!(known.check("a", 22, &key(KEY_B)), Verdict::Unknown);
        assert_eq!(known.check("b", 22, &key(KEY_B)), Verdict::Known);
    }
}
