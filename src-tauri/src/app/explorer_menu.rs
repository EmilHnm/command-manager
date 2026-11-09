//! "Open in Command Manager" in the right-click menu of Windows File
//! Explorer (folders, folder backgrounds, drives). The entries live under
//! HKCU, so no administrator rights; they start the app with
//! `--open-dir <folder>`, and a running app gets the folder from the
//! second instance. Either way the folder waits in `PendingDirs` until the
//! UI takes it and opens a terminal there.

use std::sync::Mutex;

pub const OPEN_DIR_ARG: &str = "--open-dir";

/// Folders asked for by `--open-dir`, until the UI takes them.
#[derive(Default)]
pub struct PendingDirs(Mutex<Vec<String>>);

impl PendingDirs {
    pub fn push(&self, dir: String) {
        if let Ok(mut dirs) = self.0.lock() {
            dirs.push(dir);
        }
    }

    pub fn take(&self) -> Vec<String> {
        self.0
            .lock()
            .map(|mut dirs| std::mem::take(&mut *dirs))
            .unwrap_or_default()
    }
}

/// The folder after `--open-dir` (or in `--open-dir=<folder>`).
pub fn open_dir_arg<S: AsRef<str>>(args: impl IntoIterator<Item = S>) -> Option<String> {
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg.as_ref();
        let value = if arg == OPEN_DIR_ARG {
            args.next().map(|value| value.as_ref().to_string())
        } else {
            arg.strip_prefix(OPEN_DIR_ARG)
                .and_then(|rest| rest.strip_prefix('='))
                .map(str::to_string)
        };
        if let Some(value) = value {
            return Some(unquote_root(value.trim())).filter(|dir| !dir.is_empty());
        }
    }
    None
}

/// Explorer passes a drive as `"C:\"`, which Windows argv parsing reads as
/// `C:"` (the backslash escapes the quote).
fn unquote_root(value: &str) -> String {
    match value.strip_suffix('"') {
        Some(rest) => format!("{rest}\\"),
        None => value.to_string(),
    }
}

#[cfg(windows)]
mod registry {
    use super::OPEN_DIR_ARG;
    use crate::error::{Error, Result};
    use crate::pty::profiles::registry_string;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegDeleteTreeW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
    };

    const LABEL: &str = "Open in Command Manager";
    /// Folder, folder background (right-click inside a folder), drive.
    const KEYS: [&str; 3] = [
        r"Software\Classes\Directory\shell\CommandManager",
        r"Software\Classes\Directory\Background\shell\CommandManager",
        r"Software\Classes\Drive\shell\CommandManager",
    ];

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }

    fn exe() -> Result<String> {
        Ok(std::env::current_exe()?.to_string_lossy().into_owned())
    }

    fn command(exe: &str) -> String {
        format!("\"{exe}\" {OPEN_DIR_ARG} \"%V\"")
    }

    fn set_string(subkey: &str, name: &str, value: &str) -> Result<()> {
        let (subkey, name, value) = (wide(subkey), wide(name), wide(value));
        // SAFETY: NUL-terminated buffers that outlive the call; the size
        // given is the byte length of `value` with its terminator.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                PCWSTR(name.as_ptr()),
                REG_SZ.0,
                Some(value.as_ptr().cast()),
                (value.len() * 2) as u32,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(Error::msg(format!("Không ghi được registry: {status:?}")))
        }
    }

    /// The command of each entry, None when one is missing.
    fn commands() -> Option<Vec<String>> {
        KEYS.iter()
            .map(|key| registry_string(&format!(r"{key}\command"), ""))
            .collect()
    }

    pub fn is_enabled() -> bool {
        commands().is_some()
    }

    pub fn set(enabled: bool) -> Result<()> {
        if !enabled {
            for key in KEYS {
                let subkey = wide(key);
                // SAFETY: a NUL-terminated buffer that outlives the call.
                let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(subkey.as_ptr())) };
                if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
                    return Err(Error::msg(format!("Không xoá được registry: {status:?}")));
                }
            }
            return Ok(());
        }
        let exe = exe()?;
        let result = KEYS.iter().try_for_each(|key| {
            set_string(key, "", LABEL)?;
            set_string(key, "Icon", &format!("\"{exe}\",0"))?;
            set_string(&format!(r"{key}\command"), "", &command(&exe))
        });
        if result.is_err() {
            // No half-written menu.
            let _ = set(false);
        }
        result
    }

    /// At startup: entries that point at another copy of the app (moved
    /// or reinstalled elsewhere) are pointed at this one.
    pub fn refresh() {
        let Ok(exe) = exe() else { return };
        let wanted = command(&exe);
        if let Some(commands) = commands() {
            if commands.iter().any(|command| *command != wanted) {
                let _ = set(true);
            }
        }
    }
}

/// None: the platform has no such menu.
pub fn status() -> Option<bool> {
    #[cfg(windows)]
    return Some(registry::is_enabled());
    #[cfg(not(windows))]
    None
}

pub fn set(enabled: bool) -> crate::error::Result<()> {
    #[cfg(windows)]
    return registry::set(enabled);
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err(crate::error::Error::msg("Chỉ có trên Windows"))
    }
}

pub fn refresh() {
    #[cfg(windows)]
    registry::refresh();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_comes_from_the_open_dir_argument() {
        let dir = |args: &[&str]| open_dir_arg(args.iter().copied());
        assert_eq!(dir(&["app.exe"]), None);
        assert_eq!(dir(&["app.exe", "--open-dir"]), None);
        assert_eq!(dir(&["app.exe", "--open-dir", "  "]), None);
        assert_eq!(
            dir(&["app.exe", "--open-dir", r"C:\Users\hoa\dự án"]).as_deref(),
            Some(r"C:\Users\hoa\dự án")
        );
        assert_eq!(
            dir(&["app.exe", "--open-dir=/home/hoa"]).as_deref(),
            Some("/home/hoa")
        );
        // `"C:\"` from Explorer arrives as `C:"`.
        assert_eq!(
            dir(&["app.exe", "--open-dir", "C:\""]).as_deref(),
            Some(r"C:\")
        );
        assert_eq!(dir(&["app.exe", "--open-dirx", "a"]), None);
    }

    #[test]
    fn pending_folders_are_taken_once() {
        let pending = PendingDirs::default();
        pending.push("a".into());
        pending.push("b".into());
        assert_eq!(pending.take(), ["a", "b"]);
        assert!(pending.take().is_empty());
    }
}
