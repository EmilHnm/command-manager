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
            let path = directory.join(".zshrc");
            let contents = format!(
                "if [[ -f $HOME/.zshrc && $HOME/.zshrc != $ZDOTDIR/.zshrc ]]; then source $HOME/.zshrc; fi\n{}\nTRAPEXIT() {{ command rm -rf -- $ZDOTDIR; }}\n",
                include_str!("../zsh.zsh")
            );
            std::fs::write(&path, contents).map_err(|e| Error::msg(e.to_string()))?;
            command.args(["-i"]);
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
