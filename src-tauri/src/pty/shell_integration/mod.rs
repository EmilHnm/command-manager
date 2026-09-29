mod platform;

use crate::error::{Error, Result};
use portable_pty::{Child, SlavePty};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Pwsh,
    WindowsPowerShell,
    Cmd,
    Bash,
    Zsh,
    Sh,
    Other,
}

impl ShellKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pwsh => "pwsh",
            Self::WindowsPowerShell => "powershell",
            Self::Cmd => "cmd",
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Sh => "sh",
            Self::Other => "other",
        }
    }

    pub fn history_level(self) -> u8 {
        match self {
            Self::Pwsh | Self::WindowsPowerShell | Self::Bash | Self::Zsh => 1,
            Self::Cmd | Self::Sh | Self::Other => 2,
        }
    }
}

pub struct InteractiveShell {
    pub child: Box<dyn Child + Send + Sync>,
    pub shell_kind: ShellKind,
    pub history_level: u8,
    pub nonce: Option<String>,
}

/// PowerShell 7 always ships PSReadLine, so only Windows PowerShell 5.1 is
/// probed, once per app run: the probe starts a whole PowerShell process.
fn powershell_has_psreadline(executable: &str, kind: ShellKind) -> bool {
    if kind == ShellKind::Pwsh {
        return true;
    }
    static PROBED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *PROBED.get_or_init(|| probe_psreadline(executable))
}

fn probe_psreadline(executable: &str) -> bool {
    let mut command = std::process::Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(crate::process::platform::CREATE_NO_WINDOW);
    }
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "if (Get-Module -ListAvailable -Name PSReadLine) { exit 0 } else { exit 1 }",
        ])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn write_script(
    prefix: &str,
    contents: &str,
    root: Option<&std::path::Path>,
) -> Result<std::path::PathBuf> {
    let directory = root
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir)
        .join("shell-integration")
        .join("1");
    std::fs::create_dir_all(&directory).map_err(|e| Error::msg(e.to_string()))?;
    let extension = matches!(prefix, "powershell" | "pwsh");
    if extension {
        return write_shared_script(&directory, prefix, contents);
    }
    let path = directory.join(format!(
        "command-manager-{prefix}-{}.{}",
        uuid::Uuid::new_v4().simple(),
        if extension { "ps1" } else { "script" }
    ));
    std::fs::write(&path, contents).map_err(|e| Error::msg(e.to_string()))?;
    Ok(path)
}

/// PowerShell tabs share one script named after its contents. Creating a new
/// `.ps1` for every terminal made each launch wait about 0.9 s while Windows
/// scanned the fresh file; an unchanged path is only scanned once.
fn write_shared_script(
    directory: &std::path::Path,
    prefix: &str,
    contents: &str,
) -> Result<std::path::PathBuf> {
    // FNV-1a: a stable name across builds, so a new app version with a
    // changed script gets a new file instead of reusing a stale one.
    let hash = contents
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    let path = directory.join(format!("command-manager-{prefix}-{hash:016x}.ps1"));
    if std::fs::read_to_string(&path).is_ok_and(|existing| existing == contents) {
        return Ok(path);
    }
    remove_stale_scripts(directory, prefix, &path);
    // Write beside the target and rename so a tab starting concurrently never
    // runs a half-written script.
    let staging = directory.join(format!(
        "command-manager-{prefix}-{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&staging, contents).map_err(|e| Error::msg(e.to_string()))?;
    if let Err(error) = std::fs::rename(&staging, &path) {
        let _ = std::fs::remove_file(&staging);
        // Another tab may have won the race with identical contents.
        if !std::fs::read_to_string(&path).is_ok_and(|existing| existing == contents) {
            return Err(Error::msg(error.to_string()));
        }
    }
    Ok(path)
}

/// Drop scripts from older builds and the per-terminal copies written before
/// the shared script existed. PowerShell reads the file only at startup, so
/// running tabs are unaffected.
fn remove_stale_scripts(directory: &std::path::Path, prefix: &str, keep: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let stem = format!("command-manager-{prefix}-");
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path != keep && name.starts_with(&stem) && name.ends_with(".ps1") {
            let _ = std::fs::remove_file(&path);
        }
    }
}

pub fn spawn(
    slave: Box<dyn SlavePty + Send>,
    integration_root: Option<&std::path::Path>,
    preferred_shell: Option<&str>,
    load_powershell_profile: bool,
    cwd: Option<&std::path::Path>,
) -> Result<InteractiveShell> {
    platform::spawn(
        slave,
        integration_root,
        preferred_shell,
        load_powershell_profile,
        cwd,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn shell_scripts_disable_shell_owned_suggestions_in_app_sessions() {
        let powershell = include_str!("powershell.ps1");
        let bash = include_str!("bash.sh");
        let zsh = include_str!("zsh.zsh");

        assert!(powershell.contains("PredictionSource None"));
        assert!(bash.contains("bleopt complete_auto_complete=off"));
        assert!(bash.contains("builtin history 1"));
        assert!(zsh.contains("ZSH_AUTOSUGGEST_STRATEGY=()"));
    }
}
