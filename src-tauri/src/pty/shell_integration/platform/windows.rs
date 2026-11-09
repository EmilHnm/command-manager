use super::super::{
    powershell_has_psreadline, write_script, write_zsh_dir, InteractiveShell, ShellKind,
};
use crate::error::{Error, Result};
use portable_pty::{CommandBuilder, SlavePty};

use crate::pty::profiles::{self, command_exists};

/// A WSL distribution in the user's login shell, with the bash, zsh or sh
/// integration (`wsl.sh` picks it inside the distribution). The folder is
/// reported as a Windows path, so new tabs, Files and drops can use it.
/// `None` when `wsl.exe` cannot be started, so the caller falls back to the
/// Windows shells.
fn spawn_wsl(
    slave: &(dyn SlavePty + Send),
    integration_root: Option<&std::path::Path>,
    distribution: Option<&str>,
    cwd: Option<&std::path::Path>,
) -> Option<InteractiveShell> {
    let wsl = profiles::wsl_executable()?;
    let mut command = CommandBuilder::new(wsl);
    if let Some(distribution) = distribution {
        command.args(["-d", distribution]);
    }
    let default = distribution
        .is_none()
        .then(profiles::default_wsl_distribution)
        .flatten();
    match profiles::wsl_start(distribution, default.as_deref(), cwd) {
        profiles::WslStart::Linux(path) => command.args(["--cd", &path]),
        // wsl.exe starts in its Windows working directory, as /mnt/<drive>/...
        profiles::WslStart::Windows => command.cwd(cwd?),
        profiles::WslStart::Home => command.args(["--cd", "~"]),
    }

    // Without the scripts the tab still opens, only without integration.
    let scripts = write_wsl_scripts(integration_root).ok();
    if let Some(scripts) = &scripts {
        command.args(["-e", "sh", "-c", include_str!("../wsl.sh")]);
        command.env("CM_WSL", "1");
        command.env("CM_BASH_RC", &scripts.bash);
        command.env("CM_SH_RC", &scripts.sh);
        command.env("CM_ZDOTDIR", &scripts.zdotdir);
        // /p: wsl.exe hands these over translated to /mnt/<drive>/... paths.
        let ours = "CM_WSL:CM_BASH_RC/p:CM_SH_RC/p:CM_ZDOTDIR/p";
        let wslenv = match std::env::var("WSLENV") {
            Ok(existing) if !existing.is_empty() => format!("{existing}:{ours}"),
            _ => ours.to_string(),
        };
        command.env("WSLENV", wslenv);
    }
    crate::pty::session::set_terminal_env(&mut command);
    let child = match slave.spawn_command(command) {
        Ok(child) => child,
        Err(_) => {
            if let Some(scripts) = scripts {
                scripts.remove();
            }
            return None;
        }
    };
    Some(InteractiveShell {
        child,
        shell_kind: ShellKind::Wsl,
        // Typed lines are recorded by the app (level 2): which shell runs is
        // only known inside the distribution.
        history_level: ShellKind::Wsl.history_level(),
        nonce: None,
    })
}

/// One tab's integration scripts; `wsl.sh` removes the ones its shell does
/// not use, and the used one removes itself once loaded.
struct WslScripts {
    bash: std::path::PathBuf,
    sh: std::path::PathBuf,
    zdotdir: std::path::PathBuf,
}

impl WslScripts {
    fn remove(self) {
        let _ = std::fs::remove_file(self.bash);
        let _ = std::fs::remove_file(self.sh);
        let _ = std::fs::remove_dir_all(self.zdotdir);
    }
}

fn write_wsl_scripts(integration_root: Option<&std::path::Path>) -> Result<WslScripts> {
    let bash = write_script("bash", include_str!("../bash.sh"), integration_root)?;
    let sh = write_script("sh", include_str!("../sh.sh"), integration_root).inspect_err(|_| {
        let _ = std::fs::remove_file(&bash);
    })?;
    let zdotdir = write_zsh_dir(integration_root).inspect_err(|_| {
        let _ = std::fs::remove_file(&bash);
        let _ = std::fs::remove_file(&sh);
    })?;
    Ok(WslScripts { bash, sh, zdotdir })
}

pub(super) fn spawn(
    slave: Box<dyn SlavePty + Send>,
    integration_root: Option<&std::path::Path>,
    preferred_shell: Option<&str>,
    load_powershell_profile: bool,
    cwd: Option<&std::path::Path>,
) -> Result<InteractiveShell> {
    if let Some(distribution) = preferred_shell.and_then(profiles::wsl_distribution) {
        if let Some(shell) = spawn_wsl(slave.as_ref(), integration_root, distribution, cwd) {
            return Ok(shell);
        }
    }
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let script = include_str!("../powershell.ps1");
    let script_path = write_script("powershell", script, integration_root)?;
    let script_path_string = script_path.to_string_lossy().to_string();
    let preference = preferred_shell
        .filter(|value| profiles::wsl_distribution(value).is_none())
        .unwrap_or_default()
        .to_ascii_lowercase();
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
                command.arg("-NoProfile");
            }
            if kind != "Pwsh" {
                command.args(["-ExecutionPolicy", "Bypass"]);
            }
            command.args(["-NoExit", "-File", &script_path_string]);
            command.env("CM_NONCE", &nonce);
            crate::pty::session::set_terminal_env(&mut command);
            if let Some(cwd) = cwd {
                command.cwd(cwd);
            }
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
    let prompt = format!("{esc}]633;A{bel}{esc}]633;P;Cwd=$P{bel}$P$G{esc}]633;B{bel}");
    command.args(["/D", "/Q", "/K"]);
    command.env("PROMPT", prompt);
    crate::pty::session::set_terminal_env(&mut command);
    if let Some(cwd) = cwd {
        command.cwd(cwd);
    }
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
