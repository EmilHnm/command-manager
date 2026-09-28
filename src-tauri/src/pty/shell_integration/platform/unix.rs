use super::super::{powershell_has_psreadline, write_script, InteractiveShell, ShellKind};
use crate::error::{Error, Result};
use portable_pty::{CommandBuilder, SlavePty};

fn shell_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .trim_end_matches(".exe")
}

pub(super) fn resolve_shell_command(value: &str) -> Option<String> {
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

pub(super) fn spawn(
    slave: Box<dyn SlavePty + Send>,
    integration_root: Option<&std::path::Path>,
    preferred_shell: Option<&str>,
    load_powershell_profile: bool,
) -> Result<InteractiveShell> {
    let preferred = preferred_shell
        .filter(|value| !value.trim().is_empty())
        .and_then(resolve_shell_command);
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
    let mut command = CommandBuilder::new(&shell);
    match kind {
        ShellKind::Bash => {
            let path = write_script("bash", include_str!("../bash.sh"), integration_root)?;
            command.args(["--rcfile", &path.to_string_lossy(), "-i"]);
            command.env("CM_NONCE", &nonce);
        }
        ShellKind::Zsh => {
            let directory = integration_root
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(std::env::temp_dir)
                .join("shell-integration")
                .join("1")
                .join(format!("zsh-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&directory).map_err(|e| Error::msg(e.to_string()))?;
            // zsh reads .zshenv and .zshrc from ZDOTDIR, so pointing it at our
            // directory would skip the user's ~/.zshenv (often where PATH is
            // set) and make oh-my-zsh rebuild .zcompdump in every new tab.
            // Each file switches ZDOTDIR back to the user's while their file
            // runs, as VS Code's shell integration does.
            let zshenv = "CM_APP_ZDOTDIR=$ZDOTDIR\n\
                ZDOTDIR=${CM_USER_ZDOTDIR:-$HOME}\n\
                if [[ -f $ZDOTDIR/.zshenv ]]; then source $ZDOTDIR/.zshenv; fi\n\
                CM_USER_ZDOTDIR=$ZDOTDIR\n\
                ZDOTDIR=$CM_APP_ZDOTDIR\n";
            std::fs::write(directory.join(".zshenv"), zshenv)
                .map_err(|e| Error::msg(e.to_string()))?;
            let contents = format!(
                "ZDOTDIR=$CM_USER_ZDOTDIR\n\
                 if [[ -f $ZDOTDIR/.zshrc ]]; then source $ZDOTDIR/.zshrc; fi\n\
                 {}\n\
                 TRAPEXIT() {{ command rm -rf -- $CM_APP_ZDOTDIR; }}\n",
                include_str!("../zsh.zsh")
            );
            std::fs::write(directory.join(".zshrc"), contents)
                .map_err(|e| Error::msg(e.to_string()))?;
            command.args(["-i"]);
            if let Ok(user_zdotdir) = std::env::var("ZDOTDIR") {
                command.env("CM_USER_ZDOTDIR", user_zdotdir);
            }
            command.env("ZDOTDIR", &directory);
            command.env("CM_NONCE", &nonce);
        }
        ShellKind::Sh => {
            let path = write_script("sh", include_str!("../sh.sh"), integration_root)?;
            command.args(["-i"]);
            command.env("ENV", &path);
        }
        ShellKind::Pwsh | ShellKind::WindowsPowerShell => {
            let path = write_script("pwsh", include_str!("../powershell.ps1"), integration_root)?;
            command.arg("-NoLogo");
            if !load_powershell_profile {
                command.arg("-NoProfile");
            }
            command.args(["-NoExit", "-File", &path.to_string_lossy()]);
            command.env("CM_NONCE", &nonce);
        }
        ShellKind::Cmd => {
            return Err(Error::msg(
                "cmd.exe shell integration is only available on Windows",
            ));
        }
        ShellKind::Other => {
            command.arg("-i");
        }
    }
    crate::pty::session::set_terminal_env(&mut command);
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

#[cfg(test)]
mod tests {
    #[test]
    fn resolves_short_shell_names_from_environment_style_values() {
        let resolved = super::resolve_shell_command("sh").expect("sh should be available");
        assert!(std::path::Path::new(&resolved).is_file());
    }
}
