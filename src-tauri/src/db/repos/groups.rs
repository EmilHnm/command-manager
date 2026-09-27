use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub group_name: String,
    pub autostart: bool,
    pub execution_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Membership {
    pub group_id: String,
    pub command_id: String,
    pub execution_order: i64,
}

pub fn list(conn: &Connection) -> Result<Vec<Group>> {
    let mut stmt = conn.prepare(
        "SELECT id, group_name, autostart, execution_mode FROM command_group ORDER BY group_name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Group {
            id: row.get(0)?,
            group_name: row.get(1)?,
            autostart: row.get::<_, i64>(2)? != 0,
            execution_mode: row.get(3)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_autostart(conn: &Connection) -> Result<Vec<Group>> {
    let mut stmt = conn.prepare(
        "SELECT id, group_name, autostart, execution_mode FROM command_group WHERE autostart = 1",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Group {
            id: row.get(0)?,
            group_name: row.get(1)?,
            autostart: true,
            execution_mode: row.get(3)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn insert(conn: &Connection, g: &Group) -> Result<()> {
    conn.execute(
        "INSERT INTO command_group (id, group_name, autostart, execution_mode) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![g.id, g.group_name, g.autostart as i64, g.execution_mode],
    )?;
    Ok(())
}

pub fn update(conn: &Connection, g: &Group) -> Result<()> {
    let n = conn.execute(
        "UPDATE command_group SET group_name = ?2, autostart = ?3, execution_mode = ?4 WHERE id = ?1",
        rusqlite::params![g.id, g.group_name, g.autostart as i64, g.execution_mode],
    )?;
    if n == 0 {
        return Err(crate::error::Error::msg("group not found"));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM command_group WHERE id = ?1", [id])?;
    Ok(())
}

pub fn memberships(conn: &Connection, group_id: &str) -> Result<Vec<Membership>> {
    let mut stmt = conn.prepare(
        "SELECT group_id, command_id, execution_order FROM group_membership
         WHERE group_id = ?1 ORDER BY execution_order",
    )?;
    let rows = stmt.query_map([group_id], |row| {
        Ok(Membership {
            group_id: row.get(0)?,
            command_id: row.get(1)?,
            execution_order: row.get(2)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn replace_memberships(
    conn: &Connection,
    group_id: &str,
    members: &[Membership],
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM group_membership WHERE group_id = ?1",
        [group_id],
    )?;
    for m in members {
        tx.execute(
            "INSERT INTO group_membership (group_id, command_id, execution_order) VALUES (?1, ?2, ?3)",
            rusqlite::params![group_id, m.command_id, m.execution_order],
        )?;
    }
    tx.commit()?;
    Ok(())
}
