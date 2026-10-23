//! The models a CLI's account can use, read from the CLI itself, to suggest
//! in the model fields. Any other name can still be typed.
//!
//! - `agy models`: "Fetching available models..." then `id<TAB>Label`.
//! - `cursor-agent models`: "Available models", then `id - Label`.
//! - `codex debug models`: JSON `{"models": [{slug, display_name,
//!   visibility, priority}]}`; `visibility: "list"` is what its picker shows.
//! - claude has no list: its aliases.

use super::adapter::{child_path, Cli};
use crate::error::{Error, Result};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

const LIST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelInfo {
    /// What `--model` takes.
    pub id: String,
    pub label: String,
}

fn model(id: &str, label: &str) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        label: label.to_string(),
    }
}

/// Asks the CLI at `program` for its models.
pub async fn list(cli: Cli, program: &Path) -> Result<Vec<ModelInfo>> {
    let args: &[&str] = match cli {
        Cli::Claude => {
            return Ok(vec![
                model("opus", "Opus (mới nhất)"),
                model("sonnet", "Sonnet (mới nhất)"),
                model("haiku", "Haiku (mới nhất)"),
            ])
        }
        Cli::Agy => &["models"],
        Cli::Cursor => &["models"],
        Cli::Codex => &["debug", "models"],
    };
    let run = tokio::process::Command::new(program)
        .args(args)
        .env("PATH", child_path(program))
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = match tokio::time::timeout(LIST_TIMEOUT, run).await {
        Err(_) => {
            return Err(Error::msg(format!(
                "{} không trả danh sách model",
                cli.as_str()
            )))
        }
        Ok(result) => result?,
    };
    let text = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        let error = String::from_utf8_lossy(&out.stderr);
        return Err(Error::msg(format!(
            "{} models: {}",
            cli.as_str(),
            error.lines().last().unwrap_or("lỗi")
        )));
    }
    Ok(match cli {
        Cli::Agy => parse_agy(&text),
        Cli::Cursor => parse_cursor(&text),
        Cli::Codex => parse_codex(&text),
        Cli::Claude => Vec::new(),
    })
}

/// Zero-width and other invisible characters some CLIs print.
fn clean(text: &str) -> String {
    text.chars()
        .filter(|ch| !matches!(ch, '\u{200b}'..='\u{200f}' | '\u{2060}' | '\u{feff}'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | ':' | '/'))
}

fn parse_agy(text: &str) -> Vec<ModelInfo> {
    text.lines()
        .filter_map(|line| {
            let (id, label) = line.split_once('\t')?;
            let id = clean(id);
            is_id(&id).then(|| model(&id, &clean(label)))
        })
        .collect()
}

fn parse_cursor(text: &str) -> Vec<ModelInfo> {
    text.lines()
        .filter_map(|line| {
            let (id, label) = line.split_once(" - ")?;
            let id = clean(id);
            is_id(&id).then(|| model(&id, &clean(label)))
        })
        .collect()
}

fn parse_codex(text: &str) -> Vec<ModelInfo> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut listed: Vec<(i64, ModelInfo)> = value
        .get("models")
        .and_then(Value::as_array)
        .map(|models| {
            models
                .iter()
                .filter(|m| m.get("visibility").and_then(Value::as_str) == Some("list"))
                .filter_map(|m| {
                    let id = m.get("slug").and_then(Value::as_str)?;
                    let label = m.get("display_name").and_then(Value::as_str).unwrap_or(id);
                    let priority = m
                        .get("priority")
                        .and_then(Value::as_i64)
                        .unwrap_or(i64::MAX);
                    Some((priority, model(id, label)))
                })
                .collect()
        })
        .unwrap_or_default();
    listed.sort_by_key(|(priority, _)| *priority);
    listed.into_iter().map(|(_, model)| model).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_each_cli_listing() {
        assert_eq!(
            parse_agy(
                "Fetching available models...\ngemini-3.8-flash-medium\tGemini 3.8 Flash (Medium)\nclaude-sonnet-5-5-low\tClaude Sonnet 5.5 (Low)\n"
            ),
            vec![
                model("gemini-3.8-flash-medium", "Gemini 3.8 Flash (Medium)"),
                model("claude-sonnet-5-5-low", "Claude Sonnet 5.5 (Low)"),
            ]
        );
        assert_eq!(
            parse_cursor(
                "Available models\n\nauto - Auto (default)\ngrok-4.7-low-fast - Grok 4.7  Low Fast\u{200b}\u{200b}\nTip: use --model\n"
            ),
            vec![
                model("auto", "Auto (default)"),
                model("grok-4.7-low-fast", "Grok 4.7 Low Fast"),
            ]
        );
        assert_eq!(
            parse_codex(
                r#"{"models":[{"slug":"hidden","display_name":"H","visibility":"hide","priority":1},
                {"slug":"gpt-5.6-luna","display_name":"GPT-5.6-Luna","visibility":"list","priority":9},
                {"slug":"gpt-5.6-terra","display_name":"GPT-5.6-Terra","visibility":"list","priority":8}]}"#
            ),
            vec![
                model("gpt-5.6-terra", "GPT-5.6-Terra"),
                model("gpt-5.6-luna", "GPT-5.6-Luna"),
            ]
        );
        assert!(parse_codex("not json").is_empty());
    }
}
