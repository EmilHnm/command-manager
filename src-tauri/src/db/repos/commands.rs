use crate::error::{Error, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Run target meaning "this machine".
pub const LOCAL: &str = "local";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    pub id: String,
    pub name: String,
    pub execution_string: String,
    pub is_shell: bool,
    pub shell_kind: Option<String>,
    #[serde(default)]
    pub quick_access: bool,
    /// Where it may run: `"local"` and/or SSH host ids, in display order.
    /// Empty means this machine only (every command before targets existed).
    #[serde(default)]
    pub targets: Vec<String>,
}

impl CommandDef {
    /// The targets a run may pick from; never empty.
    pub fn allowed_targets(&self) -> Vec<String> {
        if self.targets.is_empty() {
            vec![LOCAL.to_string()]
        } else {
            self.targets.clone()
        }
    }

    /// Used when nothing was picked (a group member without a pinned
    /// target): this machine if allowed, else the first host.
    pub fn default_target(&self) -> String {
        let allowed = self.allowed_targets();
        if allowed.iter().any(|target| target == LOCAL) {
            LOCAL.to_string()
        } else {
            allowed[0].clone()
        }
    }

    /// Resolves the target of one run. `None` is fine only when there is
    /// exactly one choice; with several the user has to pick.
    pub fn resolve_target(&self, wanted: Option<&str>) -> Result<String> {
        let allowed = self.allowed_targets();
        match wanted.map(str::trim).filter(|target| !target.is_empty()) {
            Some(target) if allowed.iter().any(|allowed| allowed == target) => {
                Ok(target.to_string())
            }
            Some(_) => Err(Error::msg(
                "Lệnh này không được cấu hình để chạy ở nơi đã chọn",
            )),
            None if allowed.len() == 1 => Ok(allowed[0].clone()),
            None => Err(Error::msg("Lệnh có nhiều nơi chạy, hãy chọn một nơi")),
        }
    }
}

/// Checks and tidies a target list: known hosts only, no duplicates, and
/// "local only" stored as no rows.
pub fn normalize_targets(conn: &Connection, targets: &[String]) -> Result<Vec<String>> {
    let mut clean: Vec<String> = Vec::new();
    for target in targets.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        if clean.iter().any(|existing| existing == target) {
            continue;
        }
        if target != LOCAL {
            super::ssh_hosts::require(conn, target)?;
        }
        clean.push(target.to_string());
    }
    if clean.len() == 1 && clean[0] == LOCAL {
        clean.clear();
    }
    Ok(clean)
}

fn load_targets(conn: &Connection) -> Result<HashMap<String, Vec<String>>> {
    let mut stmt = conn
        .prepare("SELECT command_id, target FROM command_target ORDER BY command_id, sort_order")?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for row in stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (command_id, target) = row?;
        map.entry(command_id).or_default().push(target);
    }
    Ok(map)
}

fn save_targets(conn: &Connection, cmd: &CommandDef) -> Result<()> {
    conn.execute(
        "DELETE FROM command_target WHERE command_id = ?1",
        [&cmd.id],
    )?;
    for (order, target) in cmd.targets.iter().enumerate() {
        conn.execute(
            "INSERT INTO command_target (command_id, target, sort_order) VALUES (?1, ?2, ?3)",
            rusqlite::params![cmd.id, target, order as i64],
        )?;
    }
    Ok(())
}

const COLUMNS: &str = "id, name, execution_string, is_shell, shell_kind, quick_access";

fn from_row(row: &rusqlite::Row) -> rusqlite::Result<CommandDef> {
    Ok(CommandDef {
        id: row.get(0)?,
        name: row.get(1)?,
        execution_string: row.get(2)?,
        is_shell: row.get::<_, i64>(3)? != 0,
        shell_kind: row.get(4)?,
        quick_access: row.get::<_, i64>(5)? != 0,
        targets: Vec::new(),
    })
}

pub fn list(conn: &Connection) -> Result<Vec<CommandDef>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM command_definition ORDER BY name"
    ))?;
    let mut targets = load_targets(conn)?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows
        .filter_map(|r| r.ok())
        .map(|mut cmd| {
            cmd.targets = targets.remove(&cmd.id).unwrap_or_default();
            cmd
        })
        .collect())
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<CommandDef>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM command_definition WHERE id = ?1"
    ))?;
    let mut rows = stmt.query_map([id], from_row)?;
    let Some(mut cmd) = rows.next().transpose()? else {
        return Ok(None);
    };
    let mut targets = conn
        .prepare("SELECT target FROM command_target WHERE command_id = ?1 ORDER BY sort_order")?;
    cmd.targets = targets
        .query_map([id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(Some(cmd))
}

pub fn insert(conn: &Connection, cmd: &CommandDef) -> Result<()> {
    conn.execute(
        "INSERT INTO command_definition (id, name, execution_string, is_shell, shell_kind, quick_access) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![cmd.id, cmd.name, cmd.execution_string, cmd.is_shell as i64, cmd.shell_kind, cmd.quick_access as i64],
    )?;
    save_targets(conn, cmd)
}

pub fn update(conn: &Connection, cmd: &CommandDef) -> Result<()> {
    let n = conn.execute(
        "UPDATE command_definition SET name = ?2, execution_string = ?3, is_shell = ?4, shell_kind = ?5, quick_access = ?6 WHERE id = ?1",
        rusqlite::params![cmd.id, cmd.name, cmd.execution_string, cmd.is_shell as i64, cmd.shell_kind, cmd.quick_access as i64],
    )?;
    if n == 0 {
        return Err(Error::msg("command not found"));
    }
    save_targets(conn, cmd)
}

pub fn set_quick_access(conn: &Connection, id: &str, enabled: bool) -> Result<()> {
    let n = conn.execute(
        "UPDATE command_definition SET quick_access = ?2 WHERE id = ?1",
        rusqlite::params![id, enabled as i64],
    )?;
    if n == 0 {
        return Err(Error::msg("command not found"));
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
            targets: Vec::new(),
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
            targets: Vec::new(),
        };
        insert(&conn, &cmd).expect("insert");

        cmd.name = "Docker Containers".to_string();
        update(&conn, &cmd).expect("update");

        let fetched = get(&conn, "cmd-2").expect("get").expect("found");
        assert_eq!(fetched.name, "Docker Containers");
        assert!(fetched.quick_access);
    }

    fn host(conn: &Connection, id: &str) {
        let input = crate::db::repos::ssh_hosts::SshHostInput {
            name: id.into(),
            host: "10.0.0.1".into(),
            port: 22,
            username: "u".into(),
            auth_method: "auto".into(),
            key_source: "file".into(),
            ..Default::default()
        };
        crate::db::repos::ssh_hosts::insert(conn, id, &input, "0").expect("host");
    }

    fn command(targets: &[&str]) -> CommandDef {
        CommandDef {
            id: "c1".into(),
            name: "deploy".into(),
            execution_string: "make deploy".into(),
            is_shell: true,
            shell_kind: None,
            quick_access: false,
            targets: targets.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn targets_round_trip_and_follow_host_deletion() {
        let conn = setup_db();
        host(&conn, "h1");
        host(&conn, "h2");
        let targets = normalize_targets(
            &conn,
            &["h1".into(), " local ".into(), "h1".into(), "h2".into()],
        )
        .expect("valid");
        assert_eq!(targets, ["h1", "local", "h2"]);
        insert(
            &conn,
            &CommandDef {
                targets,
                ..command(&[])
            },
        )
        .expect("insert");
        assert_eq!(
            get(&conn, "c1").unwrap().unwrap().targets,
            ["h1", "local", "h2"]
        );

        conn.execute("DELETE FROM ssh_host WHERE id = 'h1'", [])
            .unwrap();
        assert_eq!(list(&conn).unwrap()[0].targets, ["local", "h2"]);

        assert!(normalize_targets(&conn, &["missing".into()]).is_err());
        // "Local only" is stored as no rows.
        assert!(normalize_targets(&conn, &["local".into()])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn resolving_a_run_target() {
        let local_only = command(&[]);
        assert_eq!(local_only.resolve_target(None).unwrap(), "local");
        assert!(local_only.resolve_target(Some("h1")).is_err());

        let both = command(&["h1", "local"]);
        assert!(
            both.resolve_target(None).is_err(),
            "several targets need a pick"
        );
        assert_eq!(both.resolve_target(Some("h1")).unwrap(), "h1");
        assert_eq!(both.default_target(), "local");

        let remote_only = command(&["h2", "h1"]);
        assert_eq!(remote_only.default_target(), "h2");
    }
}
