//! Incremental OSC 633 shell-integration parsing.
//!
//! The PTY is a byte stream: an OSC frame may be split between any two reads.
//! This module deliberately keeps parsing state separate from the terminal
//! renderer so malformed terminal output can never become a history record.

use std::mem;

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
const MAX_FRAME_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Osc633Event {
    PromptStart,
    PromptEnd,
    CommandLine {
        command: String,
        nonce: Option<String>,
    },
    CommandStart,
    CommandFinished {
        exit_code: Option<i32>,
    },
    Property {
        key: String,
        value: String,
    },
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Ground,
    Escape,
    Osc,
    OscData,
    OscEsc,
}

#[derive(Debug, Default)]
pub struct OscScanner {
    state: State,
    frame: Vec<u8>,
}

impl OscScanner {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Osc633Event> {
        let mut events = Vec::new();
        for &byte in bytes {
            match self.state {
                State::Ground => {
                    if byte == ESC {
                        self.state = State::Escape;
                    }
                }
                State::Escape => {
                    if byte == b']' {
                        self.state = State::Osc;
                        self.frame.clear();
                    } else if byte == ESC {
                        self.state = State::Escape;
                    } else {
                        self.state = State::Ground;
                    }
                }
                State::Osc => {
                    if byte == b'6' {
                        self.frame.push(byte);
                        self.state = State::OscData;
                    } else {
                        self.reset();
                    }
                }
                State::OscData => {
                    self.frame.push(byte);
                    if self.frame.len() > MAX_FRAME_BYTES {
                        self.reset();
                    } else if byte == BEL {
                        self.frame.pop();
                        self.finish_frame(&mut events, false);
                    } else if byte == ESC {
                        self.frame.pop();
                        self.state = State::OscEsc;
                    }
                }
                State::OscEsc => {
                    if byte == b'\\' {
                        self.finish_frame(&mut events, true);
                    } else {
                        self.reset();
                    }
                }
            }
        }
        events
    }

    fn finish_frame(&mut self, events: &mut Vec<Osc633Event>, terminated_by_st: bool) {
        let frame = mem::take(&mut self.frame);
        self.state = State::Ground;
        // The frame includes the first `6` from OSC 633. Require the complete
        // selector so OSC 633.1 or another OSC cannot be misinterpreted.
        if frame.len() < 4 || &frame[..4] != b"633;" {
            return;
        }
        let payload = &frame[4..];
        let payload = String::from_utf8_lossy(payload);
        let _ = terminated_by_st;
        parse_payload(&payload, events);
    }

    fn reset(&mut self) {
        self.state = State::Ground;
        self.frame.clear();
    }
}

fn parse_payload(payload: &str, events: &mut Vec<Osc633Event>) {
    let (kind, rest) = payload.split_once(';').unwrap_or((payload, ""));
    match kind {
        "A" => events.push(Osc633Event::PromptStart),
        "B" => events.push(Osc633Event::PromptEnd),
        "C" => events.push(Osc633Event::CommandStart),
        "D" => {
            let code = rest.trim().parse::<i32>().ok();
            // VS Code uses D;<exit-code>; accepting D without a code is useful
            // for shells that only signal completion.
            events.push(Osc633Event::CommandFinished { exit_code: code });
        }
        "E" => {
            let (command, nonce) = split_escaped_field(rest)
                .map(|(command, nonce)| {
                    let nonce = (!nonce.is_empty()).then(|| unescape(nonce));
                    (unescape(command), nonce)
                })
                .unwrap_or_else(|| (unescape(rest), None));
            events.push(Osc633Event::CommandLine { command, nonce });
        }
        "P" => {
            let property = rest.strip_prefix(';').unwrap_or(rest);
            if let Some((key, value)) = property.split_once('=') {
                events.push(Osc633Event::Property {
                    key: key.to_string(),
                    value: unescape(value),
                });
            }
        }
        _ => {}
    }
}

fn split_escaped_field(value: &str) -> Option<(&str, &str)> {
    let mut escaped = false;
    for (index, byte) in value.as_bytes().iter().enumerate() {
        if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == b';' {
            return Some((&value[..index], &value[index + 1..]));
        }
    }
    None
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('x') => {
                    let hi = chars.next();
                    let lo = chars.next();
                    if let (Some(hi), Some(lo)) = (hi, lo) {
                        if let (Some(hi), Some(lo)) = (hi.to_digit(16), lo.to_digit(16)) {
                            if let Some(decoded) = char::from_u32(hi * 16 + lo) {
                                out.push(decoded);
                                continue;
                            }
                        }
                    }
                    out.push('\\');
                    out.push('x');
                }
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub struct ShellHistoryRecord {
    pub command_line: String,
    pub shell_kind: String,
    pub cwd: Option<String>,
    pub exit_code: Option<i32>,
}

pub struct ShellTracker {
    scanner: OscScanner,
    nonce: String,
    shell_kind: String,
    pending_command: Option<String>,
    command_started: bool,
    cwd: Option<String>,
    on_record: Box<dyn FnMut(ShellHistoryRecord) + Send>,
}

impl ShellTracker {
    pub fn new(
        nonce: String,
        shell_kind: String,
        on_record: impl FnMut(ShellHistoryRecord) + Send + 'static,
    ) -> Self {
        Self {
            scanner: OscScanner::default(),
            nonce,
            shell_kind,
            pending_command: None,
            command_started: false,
            cwd: None,
            on_record: Box::new(on_record),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        for event in self.scanner.feed(bytes) {
            match event {
                Osc633Event::CommandLine { command, nonce }
                    if nonce.as_deref() == Some(self.nonce.as_str()) =>
                {
                    self.pending_command = Some(command);
                    self.command_started = false;
                }
                Osc633Event::CommandStart => self.command_started = true,
                Osc633Event::CommandFinished { exit_code } if self.command_started => {
                    if let Some(command_line) = self.pending_command.take() {
                        (self.on_record)(ShellHistoryRecord {
                            command_line,
                            shell_kind: self.shell_kind.clone(),
                            cwd: self.cwd.clone(),
                            exit_code,
                        });
                    }
                    self.command_started = false;
                }
                Osc633Event::Property { key, value } if key == "Cwd" => self.cwd = Some(value),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(payload: &str) -> Vec<u8> {
        format!("\x1b]633;{payload}\x1b\\").into_bytes()
    }

    #[test]
    fn parses_frames_split_at_every_byte() {
        let bytes = frame("E;echo\\x20hi;n1");
        for split in 0..=bytes.len() {
            let mut scanner = OscScanner::default();
            let mut events = scanner.feed(&bytes[..split]);
            events.extend(scanner.feed(&bytes[split..]));
            assert_eq!(
                events,
                vec![Osc633Event::CommandLine {
                    command: "echo hi".into(),
                    nonce: Some("n1".into()),
                }]
            );
        }
    }

    #[test]
    fn parses_bel_and_properties() {
        let mut scanner = OscScanner::default();
        let events = scanner.feed(b"\x1b]633;P;Cwd=C:\\\\Users\\\\Hoa\x07");
        assert_eq!(
            events,
            vec![Osc633Event::Property {
                key: "Cwd".into(),
                value: "C:\\Users\\Hoa".into(),
            }]
        );
    }

    #[test]
    fn tracker_requires_nonce_and_records_completed_commands() {
        let records = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let output = records.clone();
        let mut tracker = ShellTracker::new("nonce".into(), "pwsh".into(), move |record| {
            output.lock().unwrap().push(record);
        });
        tracker.feed(&frame("E;echo\\x20bad;other"));
        tracker.feed(&frame("C"));
        tracker.feed(&frame("D;0"));
        assert!(records.lock().unwrap().is_empty());
        tracker.feed(&frame("P;Cwd=C:\\\\tmp"));
        tracker.feed(&frame("E;echo\\x20ok;nonce"));
        tracker.feed(&frame("C"));
        tracker.feed(&frame("D;0"));
        let records = records.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].command_line, "echo ok");
        assert_eq!(records[0].cwd.as_deref(), Some("C:\\tmp"));
        assert_eq!(records[0].exit_code, Some(0));
    }

    #[test]
    fn keeps_unicode_and_ignores_completion_without_command_start() {
        let mut scanner = OscScanner::default();
        assert_eq!(
            scanner.feed(&frame("E;echo\\x20xin chào;nonce")),
            vec![Osc633Event::CommandLine {
                command: "echo xin chào".into(),
                nonce: Some("nonce".into()),
            }]
        );
        let records = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let output = records.clone();
        let mut tracker = ShellTracker::new("nonce".into(), "pwsh".into(), move |record| {
            output.lock().unwrap().push(record);
        });
        tracker.feed(&frame("D;0"));
        assert!(records.lock().unwrap().is_empty());
    }

    #[test]
    fn replaces_intermediate_multiline_command_until_completion() {
        let records = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let output = records.clone();
        let mut tracker = ShellTracker::new("nonce".into(), "pwsh".into(), move |record| {
            output.lock().unwrap().push(record);
        });
        tracker.feed(&frame("E;&\\x20{;nonce"));
        tracker.feed(&frame("C"));
        tracker.feed(&frame(
            "E;&\\x20{\\x0a\\x20\\x20Write-Output\\x20ok\\x0a};nonce",
        ));
        tracker.feed(&frame("C"));
        tracker.feed(&frame("D;0"));

        let records = records.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].command_line, "& {\n  Write-Output ok\n}");
        assert_eq!(records[0].exit_code, Some(0));
    }

    #[test]
    fn ignores_oversized_frames() {
        let mut scanner = OscScanner::default();
        let mut bytes = b"\x1b]633;E;".to_vec();
        bytes.extend(std::iter::repeat_n(b'a', MAX_FRAME_BYTES + 1));
        bytes.extend_from_slice(b"\x07");
        assert!(scanner.feed(&bytes).is_empty());
    }
}
