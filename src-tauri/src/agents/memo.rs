//! The memo a new agent gets of the agents before it on the same task: what
//! they said and the commands they ran, read back from their raw stream-json
//! logs. A task given to another CLI (or model) cannot continue the earlier
//! session; the memo saves the new agent from rediscovering what was done.

use super::adapter::{Cli, LogLine, Parser};

/// The whole memo; the newest lines are kept when it is longer.
pub const MEMO_BYTES: usize = 12 * 1024;
const TEXT_BYTES: usize = 800;
const TOOL_BYTES: usize = 160;

/// One earlier agent process of the task.
pub struct EarlierRun<'a> {
    pub cli: &'a str,
    pub step: &'a str,
    /// How it ended, in words ("stopped before it finished").
    pub ended: String,
    /// Its raw stream-json log.
    pub raw: String,
}

/// None when the earlier runs left nothing worth passing on.
pub fn memo(runs: &[EarlierRun], max_bytes: usize) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for run in runs {
        let Ok(cli) = Cli::parse(run.cli) else {
            continue;
        };
        let mut parser = Parser::new(cli);
        let mut body = Vec::new();
        for raw in run.raw.lines() {
            for line in parser.feed(raw) {
                match line {
                    LogLine::Text(text) => {
                        body.push(format!("{}: {}", run.cli, cut(&text, TEXT_BYTES)))
                    }
                    // Indented under what the agent said.
                    LogLine::Tool(name, detail) => body.push(format!(
                        "  {}",
                        cut(&format!("$ {name} {detail}"), TOOL_BYTES)
                    )),
                    LogLine::ToolError(error) => {
                        body.push(format!("  {}", cut(&format!("! {error}"), TOOL_BYTES)))
                    }
                    LogLine::Error(error) => {
                        body.push(format!("  {}", cut(&format!("error: {error}"), TOOL_BYTES)))
                    }
                    LogLine::Info(_) => {}
                }
            }
        }
        if body.is_empty() {
            continue;
        }
        lines.push(format!("## {} · {} · {}", run.cli, run.step, run.ended));
        lines.extend(body);
    }
    if lines.is_empty() {
        return None;
    }
    // The newest lines matter most: drop from the start.
    let mut total: usize = lines.iter().map(|line| line.len() + 1).sum();
    let mut start = 0;
    while total > max_bytes && start + 1 < lines.len() {
        total -= lines[start].len() + 1;
        start += 1;
    }
    let mut out = String::new();
    if start > 0 {
        out.push_str("(earlier lines left out)\n");
    }
    out.push_str(&lines[start..].join("\n"));
    Some(out)
}

/// At most `max` bytes, on a char boundary, newlines folded.
fn cut(text: &str, max: usize) -> String {
    let text = text.replace(['\r', '\n'], " ");
    let text = text.trim();
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude_log() -> String {
        [
            r#"{"type":"system","subtype":"init","session_id":"s1","model":"sonnet"}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Đọc api/cart.rs trước.\nRồi thêm test."}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test cart"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","is_error":true,"content":"1 test failed"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Sửa giảm giá âm."}]}}"#,
            "not json at all",
        ]
        .join("\n")
    }

    #[test]
    fn the_memo_keeps_what_the_agent_said_and_ran() {
        let runs = [EarlierRun {
            cli: "claude",
            step: "implement",
            ended: "stopped before it finished".into(),
            raw: claude_log(),
        }];
        let memo = memo(&runs, MEMO_BYTES).unwrap();
        assert!(
            memo.starts_with("## claude · implement · stopped before it finished"),
            "{memo}"
        );
        assert!(
            memo.contains("claude: Đọc api/cart.rs trước. Rồi thêm test."),
            "{memo}"
        );
        assert!(memo.contains("  $ Bash"), "{memo}");
        assert!(memo.contains("cargo test cart"), "{memo}");
        assert!(memo.contains("  ! 1 test failed"), "{memo}");
        assert!(memo.contains("claude: Sửa giảm giá âm."), "{memo}");
        assert!(!memo.contains("not json"), "{memo}");
    }

    #[test]
    fn a_long_memo_keeps_the_newest_lines() {
        let raw = (0..400)
            .map(|i| {
                format!(r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"bước {i}"}}]}}}}"#)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let runs = [EarlierRun {
            cli: "claude",
            step: "implement",
            ended: "failed".into(),
            raw,
        }];
        let memo = memo(&runs, 500).unwrap();
        assert!(memo.len() <= 500 + 40, "{}", memo.len());
        assert!(memo.starts_with("(earlier lines left out)"), "{memo}");
        assert!(memo.ends_with("claude: bước 399"), "{memo}");
        assert!(!memo.contains("bước 0\n"), "{memo}");
    }

    #[test]
    fn nothing_said_means_no_memo() {
        let runs = [EarlierRun {
            cli: "claude",
            step: "implement",
            ended: "failed".into(),
            raw: r#"{"type":"system","subtype":"init","session_id":"s1"}"#.into(),
        }];
        assert_eq!(memo(&runs, MEMO_BYTES), None);
        assert_eq!(memo(&[], MEMO_BYTES), None);
    }

    #[test]
    fn long_text_is_cut_on_a_char_boundary() {
        let text = "đ".repeat(1000);
        let out = cut(&text, 801);
        assert!(out.ends_with('…'));
        assert!(out.len() <= 801 + '…'.len_utf8());
    }
}
