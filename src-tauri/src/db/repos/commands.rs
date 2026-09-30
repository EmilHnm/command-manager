use crate::error::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    pub id: String,
    pub name: String,
    pub execution_string: String,
    pub is_shell: bool,
    pub shell_kind: Option<String>,
    #[serde(default)]
    pub quick_access: bool,
}

pub fn list(conn: &Connection) -> Result<Vec<CommandDef>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, execution_string, is_shell, shell_kind, quick_access FROM command_definition ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(CommandDef {
            id: row.get(0)?,
            name: row.get(1)?,
            execution_string: row.get(2)?,
            is_shell: row.get::<_, i64>(3)? != 0,
            shell_kind: row.get(4)?,
            quick_access: row.get::<_, i64>(5)? != 0,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<CommandDef>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, execution_string, is_shell, shell_kind, quick_access FROM command_definition WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], |row| {
        Ok(CommandDef {
            id: row.get(0)?,
            name: row.get(1)?,
            execution_string: row.get(2)?,
            is_shell: row.get::<_, i64>(3)? != 0,
            shell_kind: row.get(4)?,
            quick_access: row.get::<_, i64>(5)? != 0,
        })
    })?;
    Ok(rows.next().transpose()?)
}

pub fn insert(conn: &Connection, cmd: &CommandDef) -> Result<()> {
    conn.execute(
        "INSERT INTO command_definition (id, name, execution_string, is_shell, shell_kind, quick_access) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![cmd.id, cmd.name, cmd.execution_string, cmd.is_shell as i64, cmd.shell_kind, cmd.quick_access as i64],
    )?;
    Ok(())
}

pub fn update(conn: &Connection, cmd: &CommandDef) -> Result<()> {
    let n = conn.execute(
        "UPDATE command_definition SET name = ?2, execution_string = ?3, is_shell = ?4, shell_kind = ?5, quick_access = ?6 WHERE id = ?1",
        rusqlite::params![cmd.id, cmd.name, cmd.execution_string, cmd.is_shell as i64, cmd.shell_kind, cmd.quick_access as i64],
    )?;
    if n == 0 {
        return Err(crate::error::Error::msg("command not found"));
    }
    Ok(())
}

pub fn set_quick_access(conn: &Connection, id: &str, enabled: bool) -> Result<()> {
    let n = conn.execute(
        "UPDATE command_definition SET quick_access = ?2 WHERE id = ?1",
        rusqlite::params![id, enabled as i64],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::migrate;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys");
        migrate(&conn).expect("schema migration");
        conn
    }

    #[test]
    fn test_set_quick_access_toggles_and_lists() {
        let conn = setup_db();
        let cmd = CommandDef {
            id: "cmd-1".to_string(),
            name: "Git Status".to_string(),
            execution_string: "git status -sb".to_string(),
            is_shell: true,
            shell_kind: Some("bash".to_string()),
            quick_access: false,
        };
        insert(&conn, &cmd).expect("insert");

        let fetched = get(&conn, "cmd-1").expect("get").expect("found");
        assert!(!fetched.quick_access);

        set_quick_access(&conn, "cmd-1", true).expect("set quick access true");
        let fetched2 = get(&conn, "cmd-1").expect("get").expect("found");
        assert!(fetched2.quick_access);

        let list_all = list(&conn).expect("list");
        assert_eq!(list_all.len(), 1);
        assert!(list_all[0].quick_access);

        set_quick_access(&conn, "cmd-1", false).expect("set quick access false");
        let fetched3 = get(&conn, "cmd-1").expect("get").expect("found");
        assert!(!fetched3.quick_access);
    }

    #[test]
    fn test_set_quick_access_not_found_returns_error() {
        let conn = setup_db();
        let res = set_quick_access(&conn, "nonexistent", true);
        assert!(res.is_err());
    }

    #[test]
    fn test_update_preserves_quick_access_when_updated() {
        let conn = setup_db();
        let mut cmd = CommandDef {
            id: "cmd-2".to_string(),
            name: "Docker PS".to_string(),
            execution_string: "docker ps".to_string(),
            is_shell: false,
            shell_kind: None,
            quick_access: true,
        };
        insert(&conn, &cmd).expect("insert");

        cmd.name = "Docker Containers".to_string();
        update(&conn, &cmd).expect("update");

        let fetched = get(&conn, "cmd-2").expect("get").expect("found");
        assert_eq!(fetched.name, "Docker Containers");
        assert!(fetched.quick_access);
    }
}
