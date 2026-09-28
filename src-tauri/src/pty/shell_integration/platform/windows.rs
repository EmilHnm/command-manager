use super::super::{powershell_has_psreadline, write_script, InteractiveShell, ShellKind};
use crate::error::{Error, Result};
use portable_pty::{CommandBuilder, SlavePty};

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

pub(super) fn spawn(
    slave: Box<dyn SlavePty + Send>,
    integration_root: Option<&std::path::Path>,
    preferred_shell: Option<&str>,
    load_powershell_profile: bool,
) -> Result<InteractiveShell> {
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let script = include_str!("../powershell.ps1");
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
