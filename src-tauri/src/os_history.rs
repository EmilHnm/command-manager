//! Read the history files kept by the user's own shells so they can be
//! imported once into `command_history`. Nothing here writes to those files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Commands longer than this are almost always pasted scripts, not something
/// worth suggesting, and match the OSC frame limit used for live history.
const MAX_COMMAND_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zsh,
    Bash,
    PsReadLine,
}

#[derive(Debug, Clone)]
pub struct Source {
    /// `shell_kind` values stored for this file. PSReadLine keeps one file
    /// for both PowerShell 7 and Windows PowerShell 5.1.
    pub shell_kinds: Vec<&'static str>,
    pub path: PathBuf,
    pub format: Format,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub command_line: String,
    pub run_count: i64,
    pub first_used_at: i64,
    pub last_used_at: i64,
}

/// History files that exist for the current user.
pub fn detect() -> Vec<Source> {
    candidates()
        .into_iter()
        .filter_map(|(shell_kinds, paths, format)| {
            paths
                .into_iter()
                .find(|path| path.is_file())
                .map(|path| Source {
                    shell_kinds,
                    path,
                    format,
                })
        })
        .collect()
}

type Candidate = (Vec<&'static str>, Vec<PathBuf>, Format);

#[cfg(not(windows))]
fn candidates() -> Vec<Candidate> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let zdotdir = std::env::var_os("ZDOTDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.clone());
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local").join("share"));
    vec![
        (
            vec!["zsh"],
            vec![
                zdotdir.join(".zsh_history"),
                home.join(".zsh_history"),
                home.join(".zhistory"),
            ],
            Format::Zsh,
        ),
        (vec!["bash"], vec![home.join(".bash_history")], Format::Bash),
        (
            vec!["pwsh"],
            vec![data_home
                .join("powershell")
                .join("PSReadLine")
                .join("ConsoleHost_history.txt")],
            Format::PsReadLine,
        ),
    ]
}

#[cfg(windows)]
fn candidates() -> Vec<Candidate> {
    let Some(app_data) = std::env::var_os("APPDATA").map(PathBuf::from) else {
        return Vec::new();
    };
    vec![(
        vec!["pwsh", "powershell"],
        vec![app_data
            .join("Microsoft")
            .join("Windows")
            .join("PowerShell")
            .join("PSReadLine")
            .join("ConsoleHost_history.txt")],
        Format::PsReadLine,
    )]
}

/// Parse a history file into one entry per distinct command line.
pub fn read(source: &Source) -> std::io::Result<Vec<Entry>> {
    let bytes = std::fs::read(&source.path)?;
    Ok(parse(&bytes, source.format, modified_at(&source.path)))
}

fn modified_at(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_else(|| crate::clock::now_unix().parse().unwrap_or_default())
}

/// `newest_at` stands in for missing timestamps: entries without one are
/// spaced a second apart ending at it, so file order becomes recency order.
pub fn parse(bytes: &[u8], format: Format, newest_at: i64) -> Vec<Entry> {
    let raw = match format {
        Format::Zsh => parse_zsh(bytes),
        Format::Bash => parse_bash(bytes),
        Format::PsReadLine => parse_psreadline(bytes),
    };
    aggregate(raw, newest_at)
}

fn aggregate(raw: Vec<(String, Option<i64>)>, newest_at: i64) -> Vec<Entry> {
    let total = raw.len() as i64;
    let mut order = Vec::new();
    let mut entries: HashMap<String, Entry> = HashMap::new();
    for (index, (command_line, at)) in raw.into_iter().enumerate() {
        let command_line = command_line.trim_end().to_string();
        if !importable(&command_line) {
            continue;
        }
        let at = at.unwrap_or(newest_at - (total - 1 - index as i64));
        match entries.get_mut(&command_line) {
            Some(entry) => {
                entry.run_count += 1;
                entry.first_used_at = entry.first_used_at.min(at);
                entry.last_used_at = entry.last_used_at.max(at);
            }
            None => {
                order.push(command_line.clone());
                entries.insert(
                    command_line.clone(),
                    Entry {
                        command_line,
                        run_count: 1,
                        first_used_at: at,
                        last_used_at: at,
                    },
                );
            }
        }
    }
    order
        .into_iter()
        .filter_map(|command_line| entries.remove(&command_line))
        .collect()
}

/// Multi-line commands are skipped: accepting one as a suggestion would send
/// its first line to the shell and run it on its own.
fn importable(command_line: &str) -> bool {
    !command_line.trim().is_empty()
        && command_line.len() <= MAX_COMMAND_BYTES
        && !command_line.chars().any(|c| c.is_control() && c != '\t')
}

/// zsh stores bytes 0x83..=0x9f and a few others as Meta (0x83) followed by
/// the byte XOR 32, so UTF-8 text must be unmetafied before decoding.
fn unmetafy(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut iter = bytes.iter();
    while let Some(&byte) = iter.next() {
        if byte == 0x83 {
            if let Some(&next) = iter.next() {
                out.push(next ^ 32);
            }
        } else {
            out.push(byte);
        }
    }
    out
}

/// Lines ending in a backslash continue the same entry. EXTENDED_HISTORY
/// lines start with `: <start>:<elapsed>;`.
fn parse_zsh(bytes: &[u8]) -> Vec<(String, Option<i64>)> {
    let text = String::from_utf8_lossy(&unmetafy(bytes)).into_owned();
    let mut out = Vec::new();
    let mut pending: Option<(String, Option<i64>)> = None;
    for line in text.lines() {
        let (mut command, at) = match pending.take() {
            Some((mut command, at)) => {
                command.push('\n');
                command.push_str(line);
                (command, at)
            }
            None => match parse_zsh_extended(line) {
                Some((at, command)) => (command.to_string(), Some(at)),
                None => (line.to_string(), None),
            },
        };
        if command.ends_with('\\') {
            command.pop();
            pending = Some((command, at));
        } else {
            out.push((command, at));
        }
    }
    out.extend(pending);
    out
}

fn parse_zsh_extended(line: &str) -> Option<(i64, &str)> {
    let rest = line.strip_prefix(": ")?;
    let (meta, command) = rest.split_once(';')?;
    let (start, elapsed) = meta.split_once(':')?;
    if !elapsed.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((start.trim().parse().ok()?, command))
}

/// With HISTTIMEFORMAT set, bash writes `#<unix time>` before each entry.
fn parse_bash(bytes: &[u8]) -> Vec<(String, Option<i64>)> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::new();
    let mut at = None;
    for line in text.lines() {
        if let Some(stamp) = line.strip_prefix('#') {
            if let Ok(value) = stamp.parse::<i64>() {
                at = Some(value);
                continue;
            }
        }
        out.push((line.to_string(), at.take()));
    }
    out
}

/// PSReadLine ends each line of a multi-line entry except the last with a
/// backtick.
fn parse_psreadline(bytes: &[u8]) -> Vec<(String, Option<i64>)> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut out = Vec::new();
    let mut pending: Option<String> = None;
    for line in text.lines() {
        let mut command = match pending.take() {
            Some(mut command) => {
                command.push('\n');
                command.push_str(line);
                command
            }
            None => line.to_string(),
        };
        if command.ends_with('`') {
            command.pop();
            pending = Some(command);
        } else {
            out.push((command, None));
        }
    }
    out.extend(pending.map(|command| (command, None)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.command_line.as_str()).collect()
    }

    #[test]
    fn zsh_extended_history_keeps_timestamps_and_counts_repeats() {
        let file = b": 1700000000:0;git status\n: 1700000100:2;ls -la\n: 1700000200:0;git status\n";
        let entries = parse(file, Format::Zsh, 1_800_000_000);
        assert_eq!(lines(&entries), ["git status", "ls -la"]);
        assert_eq!(entries[0].run_count, 2);
        assert_eq!(entries[0].first_used_at, 1_700_000_000);
        assert_eq!(entries[0].last_used_at, 1_700_000_200);
        assert_eq!(entries[1].last_used_at, 1_700_000_100);
    }

    #[test]
    fn zsh_unmetafies_utf8_and_skips_multiline_entries() {
        // "echo ở" — 'ở' is E1 BB 9F; 0x9F is stored as Meta + (0x9F ^ 32).
        let mut file = b": 1700000000:0;echo \xe1\xbb".to_vec();
        file.extend([0x83, 0x9f ^ 32]);
        file.extend(b"\n: 1700000001:0;for x in a b; do\\\necho $x\\\ndone\nplain\n");
        let entries = parse(&file, Format::Zsh, 1_800_000_000);
        assert_eq!(lines(&entries), ["echo ở", "plain"]);
    }

    #[test]
    fn bash_uses_optional_timestamps_and_file_order_otherwise() {
        let file = b"#1700000000\nmake build\nnpm test\n\nnpm test\n";
        let entries = parse(file, Format::Bash, 1_800_000_000);
        assert_eq!(lines(&entries), ["make build", "npm test"]);
        assert_eq!(entries[0].last_used_at, 1_700_000_000);
        assert_eq!(entries[1].run_count, 2);
        assert_eq!(entries[1].first_used_at, 1_800_000_000 - 2);
        assert_eq!(entries[1].last_used_at, 1_800_000_000);
    }

    #[test]
    fn psreadline_joins_backtick_continuations_and_strips_bom() {
        let file = "\u{feff}Get-ChildItem\r\nWrite-Host `\r\n  hi\r\ncd C:\\Work\r\n".as_bytes();
        let entries = parse(file, Format::PsReadLine, 1_800_000_000);
        assert_eq!(lines(&entries), ["Get-ChildItem", "cd C:\\Work"]);
        assert_eq!(entries[1].last_used_at, 1_800_000_000);
    }
}
