//! User keyboard-shortcut overrides (docs/screens.md, SCR-05 tab Phím Tắt).
//!
//! The command registry and its default bindings live in the frontend; the
//! backend only stores what the user changed, VS Code `keybindings.json`
//! style, under the `keybindings` app setting. Every entry is normalized and
//! checked here so a hand-edited import or a stale restore cannot leave a
//! binding that steals plain typing from a terminal.

use crate::db::repos::settings;
use crate::error::{Error, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub const SETTING_KEY: &str = "keybindings";
const MAX_ENTRIES: usize = 500;

/// Scopes a user binding may target. `modal` holds fixed keys.
const SCOPES: &[&str] = &["global", "workspace", "explorer", "terminal", "suggest"];
/// Canonical modifier order. `mod` is Ctrl on Linux/Windows and Cmd on macOS.
const MODIFIERS: &[&str] = &["mod", "ctrl", "shift", "alt", "meta"];
const NAMED_KEYS: &[&str] = &[
    "space",
    "enter",
    "escape",
    "tab",
    "backspace",
    "delete",
    "insert",
    "home",
    "end",
    "pageup",
    "pagedown",
    "up",
    "down",
    "left",
    "right",
    "contextmenu",
    "numpadadd",
    "numpadsubtract",
    "numpadmultiply",
    "numpaddivide",
    "numpaddecimal",
    "numpadenter",
];
const PUNCTUATION: &[&str] = &["`", "-", "=", "[", "]", "\\", ";", "'", ",", ".", "/"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeybindingOverride {
    pub key: String,
    /// A command id, or `-<id>` to remove that command's default binding.
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

impl KeybindingOverride {
    pub fn is_removal(&self) -> bool {
        self.command.starts_with('-')
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedEntry {
    pub index: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedKeybindings {
    pub entries: Vec<KeybindingOverride>,
    pub skipped: Vec<SkippedEntry>,
}

/// Normalizes one chord, e.g. `" Shift+CTRL+n "` → `"ctrl+shift+n"`.
pub fn normalize_key(raw: &str) -> Result<String> {
    let lowered = raw.trim().to_lowercase();
    if lowered.is_empty() {
        return Err(Error::msg("Phím tắt trống"));
    }
    if lowered.split_whitespace().count() > 1 {
        return Err(Error::msg(format!(
            "Chưa hỗ trợ phím tắt nhiều bước: {raw}"
        )));
    }
    let parts: Vec<&str> = lowered.split('+').map(str::trim).collect();
    let (main, modifiers) = parts.split_last().expect("split yields one part");
    if !is_main_key(main) {
        return Err(Error::msg(format!("Phím không hợp lệ: {raw}")));
    }
    let mut present = [false; MODIFIERS.len()];
    for modifier in modifiers {
        let slot = MODIFIERS
            .iter()
            .position(|known| known == modifier)
            .ok_or_else(|| Error::msg(format!("Phím bổ trợ không hợp lệ: {raw}")))?;
        if present[slot] {
            return Err(Error::msg(format!("Phím bổ trợ bị lặp: {raw}")));
        }
        present[slot] = true;
    }
    // `mod` already means Ctrl or Cmd; mixing it with either is ambiguous.
    if present[0] && (present[1] || present[4]) {
        return Err(Error::msg(format!(
            "Không dùng `mod` cùng ctrl/meta: {raw}"
        )));
    }
    let mut chord: Vec<&str> = MODIFIERS
        .iter()
        .zip(present)
        .filter_map(|(name, on)| on.then_some(*name))
        .collect();
    chord.push(main);
    Ok(chord.join("+"))
}

fn is_main_key(key: &str) -> bool {
    let single_alnum = key.len() == 1
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let function_key = key
        .strip_prefix('f')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=24).contains(&n));
    let numpad_digit = key
        .strip_prefix("numpad")
        .is_some_and(|n| n.len() == 1 && n.chars().all(|c| c.is_ascii_digit()));
    single_alnum
        || function_key
        || numpad_digit
        || NAMED_KEYS.contains(&key)
        || PUNCTUATION.contains(&key)
}

fn split_chord(chord: &str) -> (Vec<&str>, &str) {
    let mut parts: Vec<&str> = chord.split('+').collect();
    let main = parts.pop().unwrap_or_default();
    (parts, main)
}

/// Keys the app never receives or that would break the UI if bound.
fn locked_reason(chord: &str) -> Option<&'static str> {
    let (modifiers, main) = split_chord(chord);
    let ctrl = modifiers.iter().any(|m| *m == "ctrl" || *m == "mod");
    let alt = modifiers.contains(&"alt");
    let only_shift = modifiers.iter().all(|m| *m == "shift");
    if main == "tab" && only_shift {
        return Some("Tab và Shift+Tab dùng để di chuyển focus");
    }
    if main == "escape" && modifiers.is_empty() {
        return Some("Esc dùng để đóng modal và popup");
    }
    if ctrl && alt && main == "delete" {
        return Some("Ctrl+Alt+Delete do hệ điều hành giữ");
    }
    let tty_switch = main
        .strip_prefix('f')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=12).contains(&n));
    if ctrl && alt && tty_switch {
        return Some("Ctrl+Alt+F1..F12 dùng để đổi TTY trên Linux");
    }
    None
}

/// Plain keys and Shift+key are typing; a binding must use Ctrl/Alt/Cmd,
/// except the function keys.
fn steals_typing(chord: &str) -> bool {
    let (modifiers, main) = split_chord(chord);
    let has_command_modifier = modifiers.iter().any(|m| *m != "shift");
    let function_key = main.len() > 1
        && main
            .strip_prefix('f')
            .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()));
    !has_command_modifier && !function_key
}

fn is_command_id(id: &str) -> bool {
    let mut segments = id.split('.');
    let valid_segment = |segment: &str| {
        let mut chars = segment.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_alphanumeric())
    };
    let first = segments.next().is_some_and(valid_segment);
    let rest: Vec<&str> = segments.collect();
    first && !rest.is_empty() && rest.into_iter().all(valid_segment) && id.len() <= 100
}

/// Validates and canonicalizes one override.
pub fn normalize(entry: &KeybindingOverride) -> Result<KeybindingOverride> {
    let command = entry.command.trim();
    let id = command.strip_prefix('-').unwrap_or(command);
    if !is_command_id(id) {
        return Err(Error::msg(format!("ID tác vụ không hợp lệ: {command}")));
    }
    let key = normalize_key(&entry.key)?;
    let when = entry
        .when
        .as_deref()
        .map(str::trim)
        .filter(|when| !when.is_empty());
    let removal = command.starts_with('-');
    match when {
        Some(scope) if !SCOPES.contains(&scope) => {
            return Err(Error::msg(format!("Phạm vi không hợp lệ: {scope}")));
        }
        None if !removal => {
            return Err(Error::msg(format!("Thiếu phạm vi cho {command}")));
        }
        _ => {}
    }
    // A removal only switches a default off, so it may name any key.
    if !removal {
        if let Some(reason) = locked_reason(&key) {
            return Err(Error::msg(format!("Không thể gán {key}: {reason}")));
        }
        if steals_typing(&key) {
            return Err(Error::msg(format!(
                "Phím {key} cần Ctrl, Alt hoặc Cmd (trừ F1..F24)"
            )));
        }
    }
    Ok(KeybindingOverride {
        key,
        command: command.to_string(),
        when: when.map(str::to_string),
    })
}

/// Normalizes a full list, rejecting it on the first bad entry. Exact
/// duplicates keep their last position, since later user entries win.
pub fn normalize_all(entries: &[KeybindingOverride]) -> Result<Vec<KeybindingOverride>> {
    if entries.len() > MAX_ENTRIES {
        return Err(Error::msg(format!(
            "Tối đa {MAX_ENTRIES} phím tắt tùy chỉnh"
        )));
    }
    let mut normalized = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let entry =
            normalize(entry).map_err(|error| Error::msg(format!("Mục #{}: {error}", index + 1)))?;
        normalized.retain(|existing| existing != &entry);
        normalized.push(entry);
    }
    Ok(normalized)
}

/// Lenient parse for imports and stored values: keeps every valid entry and
/// reports the rest instead of failing the whole file.
pub fn parse_lenient(json: &str) -> Result<ParsedKeybindings> {
    let items: Vec<serde_json::Value> = serde_json::from_str(json)
        .map_err(|error| Error::msg(format!("JSON phím tắt không hợp lệ: {error}")))?;
    let mut entries: Vec<KeybindingOverride> = Vec::new();
    let mut skipped = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        let parsed = serde_json::from_value::<KeybindingOverride>(item)
            .map_err(|error| Error::msg(error.to_string()))
            .and_then(|entry| normalize(&entry));
        match parsed {
            Ok(_) if entries.len() >= MAX_ENTRIES => skipped.push(SkippedEntry {
                index,
                reason: format!("Vượt quá {MAX_ENTRIES} mục"),
            }),
            Ok(entry) => {
                entries.retain(|existing| existing != &entry);
                entries.push(entry);
            }
            Err(error) => skipped.push(SkippedEntry {
                index,
                reason: error.to_string(),
            }),
        }
    }
    Ok(ParsedKeybindings { entries, skipped })
}

/// Stored overrides. A corrupt value (old backup, manual DB edit) yields the
/// entries that are still valid rather than an error, so shortcuts never
/// stop loading.
pub fn load(conn: &Connection) -> Result<Vec<KeybindingOverride>> {
    let Some(raw) = settings::get(conn, SETTING_KEY)? else {
        return Ok(Vec::new());
    };
    Ok(parse_lenient(&raw)
        .map(|parsed| parsed.entries)
        .unwrap_or_default())
}

pub fn save(conn: &Connection, entries: &[KeybindingOverride]) -> Result<Vec<KeybindingOverride>> {
    let normalized = normalize_all(entries)?;
    let json = serde_json::to_string(&normalized).map_err(|error| Error::msg(error.to_string()))?;
    settings::set(conn, SETTING_KEY, &json)?;
    Ok(normalized)
}

pub fn reset(conn: &Connection) -> Result<()> {
    settings::delete(conn, SETTING_KEY)
}

pub fn to_export_json(entries: &[KeybindingOverride]) -> Result<String> {
    serde_json::to_string_pretty(entries).map_err(|error| Error::msg(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, command: &str, when: Option<&str>) -> KeybindingOverride {
        KeybindingOverride {
            key: key.into(),
            command: command.into(),
            when: when.map(Into::into),
        }
    }

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("db");
        conn.execute_batch("CREATE TABLE app_setting (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .expect("schema");
        conn
    }

    #[test]
    fn keys_are_lowercased_and_modifiers_ordered() {
        assert_eq!(normalize_key(" Shift+CTRL+n ").unwrap(), "ctrl+shift+n");
        assert_eq!(normalize_key("alt+mod+0").unwrap(), "mod+alt+0");
        assert_eq!(normalize_key("ctrl+\\").unwrap(), "ctrl+\\");
        assert_eq!(normalize_key("F12").unwrap(), "f12");
    }

    #[test]
    fn malformed_keys_are_rejected() {
        for bad in [
            "",
            "ctrl+",
            "ctrl",
            "ctrl+ctrl+n",
            "hyper+n",
            "ctrl+f25",
            "ctrl+é",
            "mod+ctrl+n",
            "ctrl+k ctrl+s",
        ] {
            assert!(normalize_key(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn bindings_must_not_steal_typing() {
        assert!(normalize(&entry("n", "workspace.newTerminal", Some("workspace"))).is_err());
        assert!(normalize(&entry("shift+n", "workspace.newTerminal", Some("terminal"))).is_err());
        assert!(normalize(&entry("f5", "workspace.newTerminal", Some("workspace"))).is_ok());
        assert!(normalize(&entry("alt+n", "workspace.newTerminal", Some("workspace"))).is_ok());
    }

    #[test]
    fn locked_keys_cannot_be_bound_but_can_be_removed() {
        for locked in [
            "tab",
            "shift+tab",
            "escape",
            "ctrl+alt+delete",
            "ctrl+alt+f3",
        ] {
            assert!(normalize(&entry(locked, "app.quit", Some("global"))).is_err());
        }
        assert!(normalize(&entry("end", "-terminal.acceptSuggest", None)).is_ok());
        assert!(normalize(&entry("ctrl+tab", "app.quit", Some("global"))).is_ok());
    }

    #[test]
    fn scope_and_command_id_are_checked() {
        assert!(normalize(&entry("ctrl+t", "workspace.newTerminal", None)).is_err());
        assert!(normalize(&entry("ctrl+t", "workspace.newTerminal", Some("modal"))).is_err());
        assert!(normalize(&entry("ctrl+t", "newTerminal", Some("workspace"))).is_err());
        assert!(normalize(&entry("ctrl+t", "Workspace.new", Some("workspace"))).is_err());
        assert!(normalize(&entry("ctrl+t", "--workspace.new", None)).is_err());
        let removal = normalize(&entry("CTRL+N", " -workspace.newTerminal ", Some(" "))).unwrap();
        assert_eq!(removal, entry("ctrl+n", "-workspace.newTerminal", None));
    }

    #[test]
    fn duplicates_keep_their_last_position() {
        let list = normalize_all(&[
            entry("ctrl+t", "workspace.newTerminal", Some("workspace")),
            entry("ctrl+n", "-workspace.newTerminal", None),
            entry("Ctrl+T", "workspace.newTerminal", Some("workspace")),
        ])
        .unwrap();
        assert_eq!(
            list,
            vec![
                entry("ctrl+n", "-workspace.newTerminal", None),
                entry("ctrl+t", "workspace.newTerminal", Some("workspace")),
            ]
        );
    }

    #[test]
    fn strict_save_names_the_bad_entry() {
        let error = normalize_all(&[
            entry("ctrl+t", "workspace.newTerminal", Some("workspace")),
            entry("x", "workspace.newTerminal", Some("workspace")),
        ])
        .unwrap_err();
        assert!(error.to_string().starts_with("Mục #2:"), "{error}");
    }

    #[test]
    fn lenient_parse_keeps_valid_entries_and_reports_the_rest() {
        let parsed = parse_lenient(
            r#"[
                {"key": "ctrl+t", "command": "workspace.newTerminal", "when": "workspace"},
                {"key": "x", "command": "workspace.newTerminal", "when": "workspace"},
                {"command": "app.quit"},
                {"key": "ctrl+n", "command": "-workspace.newTerminal"}
            ]"#,
        )
        .unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(
            parsed.skipped.iter().map(|s| s.index).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(parse_lenient("{}").is_err());
    }

    #[test]
    fn save_load_and_reset_round_trip() {
        let conn = memory_db();
        assert!(load(&conn).unwrap().is_empty());
        let saved = save(
            &conn,
            &[entry(
                "Shift+Ctrl+T",
                "workspace.newTerminal",
                Some("workspace"),
            )],
        )
        .unwrap();
        assert_eq!(saved[0].key, "ctrl+shift+t");
        assert_eq!(load(&conn).unwrap(), saved);
        reset(&conn).unwrap();
        assert!(load(&conn).unwrap().is_empty());
    }

    #[test]
    fn corrupt_stored_value_loads_what_is_still_valid() {
        let conn = memory_db();
        settings::set(&conn, SETTING_KEY, "not json").unwrap();
        assert!(load(&conn).unwrap().is_empty());
        settings::set(
            &conn,
            SETTING_KEY,
            r#"[{"key":"q","command":"app.quit","when":"global"},{"key":"ctrl+t","command":"app.quit","when":"global"}]"#,
        )
        .unwrap();
        assert_eq!(
            load(&conn).unwrap(),
            vec![entry("ctrl+t", "app.quit", Some("global"))]
        );
    }
}
