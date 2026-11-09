//! The kinds of terminal a new tab can open, as Windows Terminal lists them:
//! the shells found on this machine and, on Windows, each WSL distribution.
//!
//! A profile's `id` is what `terminal_shell` and `terminal_open` take:
//! `pwsh`, `powershell`, `cmd`, `bash`, `zsh`, `sh`, `wsl` (the default
//! distribution) or `wsl:<name>`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TerminalProfile {
    pub id: String,
    pub name: String,
    /// `powershell`, `cmd`, `wsl` or `unix`: picks the icon.
    pub kind: String,
}

fn profile(id: &str, name: &str, kind: &str) -> TerminalProfile {
    TerminalProfile {
        id: id.to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
    }
}

pub const WSL_PREFIX: &str = "wsl:";

/// The WSL distribution a profile id names: `Some(None)` for the default
/// one, `None` when the id is not a WSL profile.
pub fn wsl_distribution(id: &str) -> Option<Option<&str>> {
    if id.eq_ignore_ascii_case("wsl") {
        return Some(None);
    }
    let name = id.strip_prefix(WSL_PREFIX)?.trim();
    Some((!name.is_empty()).then_some(name))
}

/// Where a WSL tab starts, from the folder the app asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum WslStart {
    /// `--cd <linux path>`: a folder inside the distribution.
    Linux(String),
    /// A Windows folder: wsl.exe started there opens it as /mnt/<drive>/...
    Windows,
    /// `--cd ~`.
    Home,
}

/// `\\wsl.localhost\<distro>\rest` or `\\wsl$\<distro>\rest`, as WSL tabs
/// report their folder: `(distro, /rest)`.
pub fn wsl_unc_path(path: &str) -> Option<(&str, String)> {
    let lower = path.to_ascii_lowercase();
    let prefix = ["\\\\wsl.localhost\\", "\\\\wsl$\\"]
        .into_iter()
        .find(|prefix| lower.starts_with(prefix))?;
    let rest = &path[prefix.len()..];
    let (distro, inner) = rest.split_once('\\').unwrap_or((rest, ""));
    if distro.is_empty() {
        return None;
    }
    let inner = inner.trim_end_matches('\\').replace('\\', "/");
    Some((distro, format!("/{inner}")))
}

/// `wsl.exe --cd` takes no `\\wsl.localhost` paths, so a folder inside a
/// distribution becomes its Linux path. A folder in another distribution
/// cannot be opened: the tab starts at home. `default` is the default
/// distribution's name, for a profile that names none.
pub fn wsl_start(
    distribution: Option<&str>,
    default: Option<&str>,
    cwd: Option<&std::path::Path>,
) -> WslStart {
    let Some(cwd) = cwd else {
        return WslStart::Home;
    };
    let text = cwd.to_string_lossy();
    match wsl_unc_path(&text) {
        Some((owner, linux)) => {
            let target = distribution.or(default);
            if target.is_some_and(|target| target.eq_ignore_ascii_case(owner)) {
                WslStart::Linux(linux)
            } else {
                WslStart::Home
            }
        }
        None => WslStart::Windows,
    }
}

/// The default WSL distribution's name, from the registry wsl.exe keeps.
#[cfg(windows)]
pub fn default_wsl_distribution() -> Option<String> {
    const LXSS: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Lxss";
    let id = registry_string(LXSS, "DefaultDistribution")?;
    registry_string(&format!("{LXSS}\\{id}"), "DistributionName")
}

#[cfg(windows)]
pub(crate) fn registry_string(subkey: &str, value: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};

    let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let subkey = wide(subkey);
    let value = wide(value);
    let mut bytes = 0u32;
    // SAFETY: both strings are NUL-terminated and outlive the calls; the
    // second call writes at most `bytes` bytes into a buffer of that size.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut bytes),
        )
        .ok()
        .ok()?;
        let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
        .ok()
        .ok()?;
        let text = String::from_utf16_lossy(&buffer);
        let text = text.trim_end_matches('\0').trim();
        (!text.is_empty()).then(|| text.to_string())
    }
}

/// Docker Desktop's own distributions: Windows Terminal hides them too, they
/// are not meant to be worked in.
fn is_internal_distribution(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "docker-desktop" | "docker-desktop-data"
    )
}

/// `wsl.exe -l -q` prints one distribution per line in UTF-16LE.
pub fn parse_wsl_list(bytes: &[u8]) -> Vec<String> {
    let utf16 = bytes.len() >= 2
        && bytes.len().is_multiple_of(2)
        && (bytes.starts_with(&[0xff, 0xfe]) || bytes[1] == 0);
    let text = if utf16 {
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    text.lines()
        .map(|line| {
            line.trim_matches(|ch: char| ch.is_whitespace() || ch == '\u{feff}' || ch == '\0')
        })
        .filter(|name| !name.is_empty() && !is_internal_distribution(name))
        .map(str::to_string)
        .collect()
}

#[cfg(windows)]
pub fn command_exists(command: &str) -> bool {
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

/// `wsl.exe` lives in System32; looking there first does not depend on PATH.
#[cfg(windows)]
pub fn wsl_executable() -> Option<std::path::PathBuf> {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let path = std::path::Path::new(&system_root)
        .join("System32")
        .join("wsl.exe");
    if path.is_file() {
        return Some(path);
    }
    command_exists("wsl").then(|| "wsl.exe".into())
}

/// Lists the distributions, giving up after a few seconds: a broken WSL
/// install can leave `wsl.exe` hanging.
#[cfg(windows)]
fn wsl_distributions() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    use std::time::{Duration, Instant};

    let Some(wsl) = wsl_executable() else {
        return Vec::new();
    };
    let Ok(mut child) = std::process::Command::new(wsl)
        .args(["-l", "-q"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .creation_flags(crate::process::platform::CREATE_NO_WINDOW)
        .spawn()
    else {
        return Vec::new();
    };
    let mut stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(stdout) = stdout.as_mut() {
            let _ = std::io::Read::read_to_end(stdout, &mut bytes);
        }
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let bytes = reader.join().unwrap_or_default();
                return if status.success() {
                    parse_wsl_list(&bytes)
                } else {
                    Vec::new()
                };
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
        }
    }
}

#[cfg(windows)]
pub fn list() -> Vec<TerminalProfile> {
    let mut profiles = Vec::new();
    if command_exists("pwsh") {
        profiles.push(profile("pwsh", "PowerShell 7", "powershell"));
    }
    profiles.push(profile("powershell", "Windows PowerShell", "powershell"));
    profiles.push(profile("cmd", "Command Prompt", "cmd"));
    let distributions = wsl_distributions();
    if !distributions.is_empty() {
        profiles.push(profile("wsl", "WSL (bản phân phối mặc định)", "wsl"));
    }
    for name in distributions {
        profiles.push(profile(&format!("{WSL_PREFIX}{name}"), &name, "wsl"));
    }
    profiles
}

#[cfg(not(windows))]
pub fn list() -> Vec<TerminalProfile> {
    let mut profiles = Vec::new();
    for (id, name, path) in [
        ("bash", "Bash", "/bin/bash"),
        ("zsh", "Zsh", "/bin/zsh"),
        ("sh", "POSIX sh", "/bin/sh"),
    ] {
        if std::path::Path::new(path).is_file() {
            profiles.push(profile(id, name, "unix"));
        }
    }
    let has_pwsh = std::process::Command::new("which")
        .arg("pwsh")
        .output()
        .is_ok_and(|output| output.status.success());
    if has_pwsh {
        profiles.push(profile("pwsh", "PowerShell 7", "powershell"));
    }
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn wsl_list_is_read_from_utf16_and_hides_docker_distributions() {
        let bytes = utf16("\u{feff}Ubuntu-22.04\r\ndocker-desktop\r\nDebian\r\n\r\n");
        assert_eq!(parse_wsl_list(&bytes), vec!["Ubuntu-22.04", "Debian"]);
    }

    #[test]
    fn wsl_list_also_accepts_utf8_output() {
        // With WSL_UTF8=1 set, wsl.exe prints UTF-8.
        assert_eq!(
            parse_wsl_list(b"Ubuntu\nAlpine\n"),
            vec!["Ubuntu", "Alpine"]
        );
    }

    #[test]
    fn wsl_unc_paths_become_linux_paths() {
        assert_eq!(
            wsl_unc_path(r"\\wsl.localhost\Ubuntu\home\hoa\src"),
            Some(("Ubuntu", "/home/hoa/src".to_string()))
        );
        assert_eq!(
            wsl_unc_path(r"\\WSL$\Debian\"),
            Some(("Debian", "/".to_string()))
        );
        assert_eq!(
            wsl_unc_path(r"\\wsl.localhost\Ubuntu"),
            Some(("Ubuntu", "/".to_string()))
        );
        assert_eq!(wsl_unc_path(r"C:\Users\hoa"), None);
        assert_eq!(wsl_unc_path(r"\\server\share\x"), None);
    }

    #[test]
    fn wsl_tabs_start_in_their_own_distribution_only() {
        use std::path::Path;
        let ubuntu = Path::new(r"\\wsl.localhost\Ubuntu\home\hoa");
        assert_eq!(
            wsl_start(Some("ubuntu"), None, Some(ubuntu)),
            WslStart::Linux("/home/hoa".into())
        );
        assert_eq!(
            wsl_start(None, Some("Ubuntu"), Some(ubuntu)),
            WslStart::Linux("/home/hoa".into())
        );
        assert_eq!(
            wsl_start(Some("Debian"), None, Some(ubuntu)),
            WslStart::Home
        );
        assert_eq!(wsl_start(None, None, Some(ubuntu)), WslStart::Home);
        assert_eq!(
            wsl_start(Some("Debian"), None, Some(Path::new(r"C:\proj"))),
            WslStart::Windows
        );
        assert_eq!(wsl_start(Some("Debian"), None, None), WslStart::Home);
    }

    #[test]
    fn wsl_profile_ids_name_their_distribution() {
        assert_eq!(wsl_distribution("wsl"), Some(None));
        assert_eq!(wsl_distribution("wsl:Ubuntu"), Some(Some("Ubuntu")));
        assert_eq!(wsl_distribution("wsl:"), Some(None));
        assert_eq!(wsl_distribution("pwsh"), None);
    }
}
