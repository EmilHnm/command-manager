use crate::error::{Error, Result};
use portable_pty::{Child, CommandBuilder, SlavePty};

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

#[cfg(not(windows))]
fn shell_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .trim_end_matches(".exe")
}

/// Look the executable up on PATH directly; spawning `where.exe` costs a few
/// hundred milliseconds per terminal. `symlink_metadata` also sees the
/// zero-byte App Execution Alias that a Microsoft Store pwsh installs.
#[cfg(windows)]
fn command_exists(command: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|directory| {
        [".exe", ".com", ".cmd", ".bat"].iter().any(|extension| {
            std::fs::symlink_metadata(directory.join(format!("{command}{extension}")))
                .is_ok_and(|metadata| !metadata.is_dir())
        })
    })
}

#[cfg(windows)]
pub fn spawn(
    slave: Box<dyn SlavePty + Send>,
    integration_root: Option<&std::path::Path>,
    preferred_shell: Option<&str>,
    load_powershell_profile: bool,
) -> Result<InteractiveShell> {
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let script = include_str!("powershell.ps1");
    let script_path = write_script("powershell", script, integration_root)?;
    let script_path_string = script_path.to_string_lossy().to_string();
    let preference = preferred_shell.unwrap_or_default().to_ascii_lowercase();
    let mut candidates = Vec::new();
    for candidate in [
        ("pwsh", "pwsh", "Pwsh"),
        ("powershell", "powershell.exe", "WindowsPowerShell"),
    ] {
        if preference.is_empty()
            || (preference == "pwsh" && candidate.2 == "Pwsh")
            || ((preference == "powershell" || preference == "powershell.exe")
                && candidate.2 == "WindowsPowerShell")
        {
            candidates.push(candidate);
        }
    }
    for (probe, executable, kind) in candidates {
        if command_exists(probe) {
            let mut command = CommandBuilder::new(executable);
            command.arg("-NoLogo");
            if !load_powershell_profile {
                // Skipping $PROFILE avoids slow modules and prompt themes; the
                // app provides history suggestions on its own.
                command.arg("-NoProfile");
            }
            if kind != "Pwsh" {
                command.args(["-ExecutionPolicy", "Bypass"]);
            }
            command.args(["-NoExit", "-File", &script_path_string]);
            command.env("CM_NONCE", &nonce);
            if let Ok(child) = slave.spawn_command(command) {
                let shell_kind = if kind == "Pwsh" {
                    ShellKind::Pwsh
                } else {
                    ShellKind::WindowsPowerShell
                };
                let has_psreadline = powershell_has_psreadline(executable, shell_kind);
                let history_level = if has_psreadline { 1 } else { 2 };
                return Ok(InteractiveShell {
                    child,
                    shell_kind,
                    history_level,
                    nonce: (history_level == 1).then_some(nonce),
                });
            }
        }
    }

    let executable = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into());
    let mut command = CommandBuilder::new(executable);
    let esc = '\x1b';
    let bel = '\x07';
    // cmd expands PROMPT left-to-right. Put B after the visible prompt so
    // the frontend's marker range contains the actual editable input line.
    let prompt = format!("{esc}]633;A{bel}{esc}]633;P;Cwd=$P{bel}$P$G{esc}]633;B{bel}");
    command.args(["/D", "/Q", "/K"]);
    command.env("PROMPT", prompt);
    let child = slave
        .spawn_command(command)
        .map_err(|e| Error::msg(e.to_string()))?;
    Ok(InteractiveShell {
        child,
        shell_kind: ShellKind::Cmd,
        history_level: 2,
        nonce: None,
    })
}

#[cfg(not(windows))]
pub fn spawn(
    slave: Box<dyn SlavePty + Send>,
    integration_root: Option<&std::path::Path>,
    preferred_shell: Option<&str>,
    load_powershell_profile: bool,
) -> Result<InteractiveShell> {
    let preferred = preferred_shell
        .filter(|value| !value.trim().is_empty())
        .and_then(resolve_shell_command);
    // `$SHELL` is commonly either an absolute path or a short executable name
    // (for example `zsh` in containers and test runners). Resolve both forms so
    // the documented shell order does not silently fall back to bash.
    let configured = preferred.or_else(|| {
        std::env::var("SHELL")
            .ok()
            .and_then(|value| resolve_shell_command(&value))
    });
    let shell = configured
        .or_else(|| {
            ["/bin/bash", "/bin/sh"]
                .into_iter()
                .find(|path| std::path::Path::new(path).is_file())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "/bin/sh".into());
    let name = shell_name(&shell);
    let kind = match name {
        "bash" => ShellKind::Bash,
        "zsh" => ShellKind::Zsh,
        "sh" | "dash" => ShellKind::Sh,
        "pwsh" => ShellKind::Pwsh,
        "powershell" => ShellKind::WindowsPowerShell,
        _ => ShellKind::Other,
    };
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let mut command = CommandBuilder::new(shell);
    let _script_path = match kind {
        ShellKind::Bash => {
            let path = write_script("bash", include_str!("bash.sh"), integration_root)?;
            command.args(["--rcfile", &path.to_string_lossy(), "-i"]);
            command.env("CM_NONCE", &nonce);
            Some(path)
        }
        ShellKind::Zsh => {
            let directory = integration_root
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(std::env::temp_dir)
                .join("shell-integration")
                .join("1")
                .join(format!("zsh-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&directory).map_err(|e| Error::msg(e.to_string()))?;
            let path = directory.join(".zshrc");
            let contents = format!(
                "if [[ -f $HOME/.zshrc && $HOME/.zshrc != $ZDOTDIR/.zshrc ]]; then source $HOME/.zshrc; fi\n{}\nTRAPEXIT() {{ command rm -rf -- $ZDOTDIR; }}\n",
                include_str!("zsh.zsh")
            );
            std::fs::write(&path, contents).map_err(|e| Error::msg(e.to_string()))?;
            command.args(["-i"]);
            command.env("ZDOTDIR", &directory);
            command.env("CM_NONCE", &nonce);
            Some(path)
        }
        ShellKind::Sh => {
            let path = write_script("sh", include_str!("sh.sh"), integration_root)?;
            command.args(["-i"]);
            command.env("ENV", &path);
            Some(path)
        }
        ShellKind::Pwsh | ShellKind::WindowsPowerShell => {
            let path = write_script("pwsh", include_str!("powershell.ps1"), integration_root)?;
            command.arg("-NoLogo");
            if !load_powershell_profile {
                command.arg("-NoProfile");
            }
            command.args(["-NoExit", "-File", &path.to_string_lossy()]);
            command.env("CM_NONCE", &nonce);
            Some(path)
        }
        ShellKind::Other => {
            command.arg("-i");
            None
        }
    };
    let child = slave
        .spawn_command(command)
        .map_err(|e| Error::msg(e.to_string()))?;
    let history_level = match kind {
        ShellKind::Pwsh | ShellKind::WindowsPowerShell
            if !powershell_has_psreadline(&shell, kind) =>
        {
            2
        }
        _ => kind.history_level(),
    };
    Ok(InteractiveShell {
        child,
        shell_kind: kind,
        history_level,
        nonce: (history_level == 1).then_some(nonce),
    })
}

#[cfg(not(windows))]
fn resolve_shell_command(value: &str) -> Option<String> {
    let candidate = match value {
        "bash" => "/bin/bash",
        "zsh" => "/bin/zsh",
        "sh" | "dash" => "/bin/sh",
        "pwsh" => "pwsh",
        "powershell" => "powershell",
        path => path,
    };
    if std::path::Path::new(candidate).is_file() {
        return Some(candidate.to_string());
    }
    std::process::Command::new("which")
        .arg(candidate)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|path| !path.is_empty())
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

    #[cfg(not(windows))]
    #[test]
    fn resolves_short_shell_names_from_environment_style_values() {
        let resolved = super::resolve_shell_command("sh").expect("sh should be available");
        assert!(std::path::Path::new(&resolved).is_file());
    }
}
