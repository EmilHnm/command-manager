use crate::error::Result;
use regex::RegexBuilder;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistory {
    pub id: String,
    pub command_line: String,
    pub shell_kind: String,
    pub cwd: Option<String>,
    pub last_exit_code: Option<i32>,
    pub run_count: i64,
    pub first_used_at: String,
    pub last_used_at: String,
    pub source: String,
}

pub fn list(conn: &Connection, query: Option<&str>, limit: i64) -> Result<Vec<CommandHistory>> {
    let limit = limit.clamp(1, 10_000);
    let query = query.unwrap_or("").trim();
    let like = format!("%{query}%");
    let mut stmt = conn.prepare(
        "SELECT id, command_line, shell_kind, cwd, last_exit_code, run_count,
                first_used_at, last_used_at, source
         FROM command_history
         WHERE ?1 = '' OR command_line LIKE ?2 OR shell_kind LIKE ?2 OR source LIKE ?2
         ORDER BY last_used_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![query, like, limit], |row| {
        Ok(CommandHistory {
            id: row.get(0)?,
            command_line: row.get(1)?,
            shell_kind: row.get(2)?,
            cwd: row.get(3)?,
            last_exit_code: row.get(4)?,
            run_count: row.get(5)?,
            first_used_at: row.get(6)?,
            last_used_at: row.get(7)?,
            source: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn record(
    conn: &Connection,
    command_line: &str,
    shell_kind: &str,
    cwd: Option<&str>,
    source: &str,
    at: &str,
) -> Result<String> {
    let command_line = command_line.trim_end();
    if !history_allowed(conn, command_line)? {
        return Ok(String::new());
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO command_history
         (id, command_line, shell_kind, cwd, last_exit_code, run_count,
          first_used_at, last_used_at, source)
         VALUES (?1, ?2, ?3, ?4, NULL, 1, ?5, ?5, ?6)
         ON CONFLICT(command_line, shell_kind) DO UPDATE SET
           cwd = excluded.cwd,
           run_count = command_history.run_count + 1,
           last_used_at = excluded.last_used_at,
           source = excluded.source",
        params![id, command_line, shell_kind, cwd, at, source],
    )?;
    let stored_id: String = conn.query_row(
        "SELECT id FROM command_history WHERE command_line = ?1 AND shell_kind = ?2",
        params![command_line, shell_kind],
        |row| row.get(0),
    )?;
    let max_entries = settings_value(conn, "history_max_entries")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(5000)
        .clamp(1, 100_000);
    conn.execute(
        "DELETE FROM command_history
         WHERE id IN (
           SELECT id FROM command_history
           ORDER BY last_used_at DESC
           LIMIT -1 OFFSET ?1
         )",
        [max_entries],
    )?;
    Ok(stored_id)
}

pub fn finish(conn: &Connection, id: &str, exit_code: Option<i32>, at: &str) -> Result<()> {
    if id.is_empty() {
        return Ok(());
    }
    conn.execute(
        "UPDATE command_history SET last_exit_code = ?2, last_used_at = ?3 WHERE id = ?1",
        params![id, exit_code, at],
    )?;
    Ok(())
}

fn settings_value(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM app_setting WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .ok()
}

fn history_allowed(conn: &Connection, command_line: &str) -> Result<bool> {
    if command_line.is_empty() || command_line.starts_with(' ') {
        return Ok(false);
    }
    if settings_value(conn, "history_enabled").as_deref() == Some("false") {
        return Ok(false);
    }
    let patterns = settings_value(conn, "history_block_patterns")
        .unwrap_or_else(|| {
            "(?:password|passwd|pwd|secret|token|api[_-]?key)\\w*\\s*[=:]\\s*\\S+\n--(?:password|token|secret|api[_-]?key)(?:=|\\s+)\\S+\n\\b(?:mysql|mysqldump|mysqladmin)\\b.*\\s(?-i:-p)(?:\\S+|\\s+\\S+)\nauthorization:\\s*\\S+\n\\bbearer\\s+\\S+\n://[^/\\s:@]+:[^@\\s]+@"
                .into()
        });
    for pattern in patterns.lines().map(str::trim).filter(|p| !p.is_empty()) {
        let matches = RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()
            .map(|regex| regex.is_match(command_line))
            .unwrap_or_else(|_| {
                command_line
                    .to_ascii_lowercase()
                    .contains(&pattern.to_ascii_lowercase())
            });
        if matches {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM command_history WHERE id = ?1", [id])?;
    Ok(())
}

pub fn clear(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM command_history", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::Db;

    #[test]
    fn record_upserts_and_finish_updates_latest_exit_code() {
        let db = Db::open(tempfile::tempdir().unwrap().path().join("history.db")).unwrap();
        let first = db
            .write(|conn| record(conn, "echo hello", "shell", None, "command", "1"))
            .unwrap();
        let second = db
            .write(|conn| record(conn, "echo hello", "shell", None, "command", "2"))
            .unwrap();
        assert_eq!(first, second);
        db.write(|conn| finish(conn, &first, Some(0), "3")).unwrap();
        let items = db.read(|conn| list(conn, Some("hello"), 10)).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].run_count, 2);
        assert_eq!(items[0].last_exit_code, Some(0));
    }

    #[test]
    fn privacy_filters_and_retention_are_applied_before_persisting() {
        let db = Db::open(tempfile::tempdir().unwrap().path().join("history.db")).unwrap();
        db.write(|conn| {
            crate::db::repos::settings::set(conn, "history_max_entries", "2")?;
            assert!(record(conn, " password=secret", "pwsh", None, "typed", "1")?.is_empty());
            assert!(record(
                conn,
                "curl --password=secret https://example.test",
                "pwsh",
                None,
                "typed",
                "2"
            )?
            .is_empty());
            assert!(record(
                conn,
                "Authorization: Bearer secret",
                "pwsh",
                None,
                "typed",
                "3"
            )?
            .is_empty());
            record(conn, "echo one", "pwsh", None, "typed", "4")?;
            record(conn, "echo two", "pwsh", None, "typed", "5")?;
            record(conn, "echo three", "pwsh", None, "typed", "6")?;
            Ok(())
        })
        .unwrap();
        let rows = db.read(|conn| list(conn, None, 10)).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.command_line.starts_with("echo ")));
        assert!(!rows.iter().any(|row| row.command_line.contains("one")));
    }

    #[test]
    fn privacy_defaults_do_not_block_power_shell_parameters_or_ssh_ports() {
        let db = Db::open(tempfile::tempdir().unwrap().path().join("history.db")).unwrap();
        db.write(|conn| {
            let cases = [
                ("mkdir -p src/app", false),
                ("docker run -p 8080:80 nginx", false),
                ("ssh -p 2222 host", false),
                ("git log -p HEAD", false),
                ("rsync -P a b", false),
                ("grep -P x f", false),
                ("Get-ChildItem -Path C:/x", false),
                ("ssh -p2222 host", false),
                ("mysql -u root -pSecret123", true),
                ("export GITHUB_TOKEN=ghp_abc", true),
                (r#"$env:GITHUB_TOKEN="ghp_abc""#, true),
                ("curl --token abc", true),
                ("git push https://user:ghp_abc@github.com/x", true),
                ("mysql -p secret", true),
                ("Authorization: Bearer x", true),
            ];
            for (index, (command, blocked)) in cases.into_iter().enumerate() {
                let stored = record(
                    conn,
                    command,
                    "pwsh",
                    None,
                    "typed",
                    &format!("privacy-{index}"),
                )?;
                assert_eq!(
                    stored.is_empty(),
                    blocked,
                    "unexpected privacy result for {command:?}"
                );
            }
            Ok(())
        })
        .unwrap();
    }
}
