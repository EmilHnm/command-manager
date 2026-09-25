use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    pub id: String,
    pub name: String,
    pub execution_string: String,
    pub is_shell: bool,
}

pub fn list(conn: &Connection) -> Result<Vec<CommandDef>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, execution_string, is_shell FROM command_definition ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(CommandDef {
            id: row.get(0)?,
            name: row.get(1)?,
            execution_string: row.get(2)?,
            is_shell: row.get::<_, i64>(3)? != 0,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<CommandDef>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, execution_string, is_shell FROM command_definition WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], |row| {
        Ok(CommandDef {
            id: row.get(0)?,
            name: row.get(1)?,
            execution_string: row.get(2)?,
            is_shell: row.get::<_, i64>(3)? != 0,
        })
    })?;
    Ok(rows.next().transpose()?)
}

pub fn insert(conn: &Connection, cmd: &CommandDef) -> Result<()> {
    conn.execute(
        "INSERT INTO command_definition (id, name, execution_string, is_shell) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![cmd.id, cmd.name, cmd.execution_string, cmd.is_shell as i64],
    )?;
    Ok(())
}

pub fn update(conn: &Connection, cmd: &CommandDef) -> Result<()> {
    let n = conn.execute(
        "UPDATE command_definition SET name = ?2, execution_string = ?3, is_shell = ?4 WHERE id = ?1",
        rusqlite::params![cmd.id, cmd.name, cmd.execution_string, cmd.is_shell as i64],
    )?;
    if n == 0 {
        return Err(crate::error::Error::msg("command not found"));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM command_definition WHERE id = ?1", [id])?;
    Ok(())
}
