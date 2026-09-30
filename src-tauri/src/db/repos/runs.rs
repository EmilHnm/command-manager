use crate::error::Result;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSession {
    pub id: String,
    pub group_id: Option<String>,
    pub template_id: Option<String>,
    pub template_name: Option<String>,
    pub started_at: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEvent {
    pub id: String,
    pub session_id: String,
    pub command_id: String,
    pub command_name: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub pid: Option<i64>,
}

pub fn insert_session(conn: &Connection, s: &RunSession) -> Result<()> {
    conn.execute(
        "INSERT INTO run_session (id, group_id, template_id, started_at, status)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![s.id, s.group_id, s.template_id, s.started_at, s.status],
    )?;
    Ok(())
}

pub fn set_session_status(conn: &Connection, id: &str, status: &str) -> Result<()> {
    conn.execute(
        "UPDATE run_session SET status = ?2 WHERE id = ?1",
        rusqlite::params![id, status],
    )?;
    Ok(())
}

pub fn session_status(conn: &Connection, id: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT status FROM run_session WHERE id = ?1",
        [id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn finalize_session(conn: &Connection, id: &str) -> Result<()> {
    let current_status: String = conn.query_row(
        "SELECT status FROM run_session WHERE id = ?1",
        [id],
        |row| row.get(0),
    )?;
    let (pending, failed): (i64, i64) = conn.query_row(
        "SELECT
           SUM(CASE WHEN status IN ('starting', 'running') THEN 1 ELSE 0 END),
           SUM(CASE WHEN status IN ('failed', 'stopped') THEN 1 ELSE 0 END)
         FROM run_event WHERE session_id = ?1",
        [id],
        |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                row.get::<_, Option<i64>>(1)?.unwrap_or(0),
            ))
        },
    )?;
    if pending > 0 {
        return Ok(());
    }
    if current_status == "stopped" {
        return Ok(());
    }
    set_session_status(
        conn,
        id,
        if current_status == "failed" || failed > 0 {
            "failed"
        } else {
            "completed"
        },
    )
}

pub fn insert_event(conn: &Connection, e: &RunEvent) -> Result<()> {
    conn.execute(
        "INSERT INTO run_event (id, session_id, command_id, started_at, ended_at, status, exit_code, pid)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            e.id,
            e.session_id,
            e.command_id,
            e.started_at,
            e.ended_at,
            e.status,
            e.exit_code,
            e.pid
        ],
    )?;
    Ok(())
}

pub fn mark_event_started(conn: &Connection, id: &str, pid: i64) -> Result<bool> {
    let changed = conn.execute(
        "UPDATE run_event SET status = 'running', pid = ?2
         WHERE id = ?1 AND status = 'starting'",
        rusqlite::params![id, pid],
    )?;
    Ok(changed > 0)
}

pub fn mark_event_ended(
    conn: &Connection,
    id: &str,
    status: &str,
    exit_code: Option<i32>,
    ended_at: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE run_event SET status = ?2, exit_code = ?3, ended_at = ?4 WHERE id = ?1",
        rusqlite::params![id, status, exit_code, ended_at],
    )?;
    Ok(())
}

pub fn list_sessions(conn: &Connection) -> Result<Vec<RunSession>> {
    let mut stmt = conn.prepare(
        "SELECT rs.id, rs.group_id, rs.template_id, ct.name, rs.started_at, rs.status
         FROM run_session rs
         LEFT JOIN command_template ct ON ct.id = rs.template_id
         ORDER BY rs.started_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(RunSession {
            id: row.get(0)?,
            group_id: row.get(1)?,
            template_id: row.get(2)?,
            template_name: row.get(3)?,
            started_at: row.get(4)?,
            status: row.get(5)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_events(conn: &Connection, session_id: &str) -> Result<Vec<RunEvent>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, command_id, started_at, ended_at, status, exit_code, pid
         FROM run_event WHERE session_id = ?1 ORDER BY started_at",
    )?;
    let rows = stmt.query_map([session_id], |row| {
        Ok(RunEvent {
            id: row.get(0)?,
            session_id: row.get(1)?,
            command_id: row.get(2)?,
            command_name: None,
            started_at: row.get(3)?,
            ended_at: row.get(4)?,
            status: row.get(5)?,
            exit_code: row.get(6)?,
            pid: row.get(7)?,
        })
    })?;
    let mut events = Vec::new();
    for row in rows {
        let mut event = row?;
        event.command_name = conn
            .query_row(
                "SELECT name FROM command_definition WHERE id = ?1",
                [&event.command_id],
                |record| record.get(0),
            )
            .optional()?;
        if event.command_name.is_none() && event.command_id.starts_with("template:") {
            if let Some(template_id) = event.command_id.split(':').nth(1) {
                event.command_name = conn
                    .query_row(
                        "SELECT name FROM command_template WHERE id = ?1",
                        [template_id],
                        |record| record.get(0),
                    )
                    .optional()?;
            }
        }
        events.push(event);
    }
    Ok(events)
}
