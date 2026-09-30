use super::schema::{
    INIT_SQL, MIGRATION_002_SQL, MIGRATION_003_SQL, MIGRATION_004_SQL, MIGRATION_005_SQL,
    MIGRATION_006_SQL, MIGRATION_007_SQL, MIGRATION_008_SQL, MIGRATION_009_SQL, MIGRATION_010_SQL,
    SCHEMA_VERSION,
};
use crate::error::{Error, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// One writer + three readers on the same WAL file.
pub struct Db {
    pub path: PathBuf,
    writer: Mutex<Option<Connection>>,
    readers: [Mutex<Option<Connection>>; 3],
    next_reader: AtomicUsize,
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_millis(5_000))?;
    Ok(())
}

fn configure_read(conn: &Connection) -> Result<()> {
    // A read-only SQLite connection cannot change journal_mode. Keep the
    // reader setup limited to pragmas that are connection-local.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_millis(5_000))?;
    Ok(())
}

fn open_write(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

fn open_read(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    configure_read(&conn)?;
    Ok(conn)
}

impl Db {
    pub fn open(path: PathBuf) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let writer = open_write(&path)?;
        migrate(&writer)?;
        let readers = [
            Mutex::new(Some(open_read(&path)?)),
            Mutex::new(Some(open_read(&path)?)),
            Mutex::new(Some(open_read(&path)?)),
        ];
        Ok(Self {
            path,
            writer: Mutex::new(Some(writer)),
            readers,
            next_reader: AtomicUsize::new(0),
        })
    }

    pub fn write<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self
            .writer
            .lock()
            .map_err(|_| Error::msg("db writer poisoned"))?;
        let conn = guard.as_ref().ok_or_else(|| Error::msg("db closed"))?;
        f(conn)
    }

    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let reader_index = self.next_reader.fetch_add(1, Ordering::Relaxed) % self.readers.len();
        let guard = self.readers[reader_index]
            .lock()
            .map_err(|_| Error::msg("db reader poisoned"))?;
        let conn = guard.as_ref().ok_or_else(|| Error::msg("db closed"))?;
        f(conn)
    }

    /// Drop every connection so WAL files can be replaced. Caller must `reopen`.
    pub fn close_all(&self) -> Result<()> {
        let mut w = self
            .writer
            .lock()
            .map_err(|_| Error::msg("db writer poisoned"))?;
        *w = None;
        for r in &self.readers {
            let mut g = r.lock().map_err(|_| Error::msg("db reader poisoned"))?;
            *g = None;
        }
        Ok(())
    }

    pub fn reopen(&self) -> Result<()> {
        let mut w = self
            .writer
            .lock()
            .map_err(|_| Error::msg("db writer poisoned"))?;
        let writer = open_write(&self.path)?;
        migrate(&writer)?;
        *w = Some(writer);
        for r in &self.readers {
            let mut g = r.lock().map_err(|_| Error::msg("db reader poisoned"))?;
            *g = Some(open_read(&self.path)?);
        }
        Ok(())
    }
}

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        conn.execute_batch(INIT_SQL)?;
    }
    let mut version: i64 =
        conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })?;
    if version > SCHEMA_VERSION {
        return Err(Error::msg(format!(
            "database schema {version} is newer than app {SCHEMA_VERSION}"
        )));
    }
    while version < SCHEMA_VERSION {
        let next = version + 1;
        let sql = match next {
            2 => MIGRATION_002_SQL,
            3 => MIGRATION_003_SQL,
            4 => MIGRATION_004_SQL,
            5 => MIGRATION_005_SQL,
            6 => MIGRATION_006_SQL,
            7 => MIGRATION_007_SQL,
            8 => MIGRATION_008_SQL,
            9 => MIGRATION_009_SQL,
            10 => MIGRATION_010_SQL,
            _ => return Err(Error::msg(format!("missing migration for schema {next}"))),
        };
        conn.execute_batch(sql)?;
        version = next;
    }
    if version != SCHEMA_VERSION {
        return Err(Error::msg(format!(
            "database schema {version} could not be migrated to {SCHEMA_VERSION}"
        )));
    }
    Ok(())
}

pub fn migrate_file(path: &Path) -> Result<()> {
    let conn = open_write(path)?;
    migrate(&conn)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    Ok(())
}

pub fn integrity_ok(path: &Path) -> Result<bool> {
    let conn = Connection::open(path)?;
    let msg: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    Ok(msg == "ok")
}

pub fn read_schema_version(path: &Path) -> Result<i64> {
    let conn = Connection::open(path)?;
    conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_schema_supports_standalone_run_sessions() {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys");
        migrate(&conn).expect("schema migration");
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, SCHEMA_VERSION);

        conn.execute(
            "INSERT INTO run_session
             (id, group_id, template_id, started_at, status)
             VALUES ('standalone', NULL, NULL, 'now', 'running')",
            [],
        )
        .expect("standalone command session should be valid");
    }

    #[test]
    fn opens_wal_database_with_read_only_readers() {
        let path = std::env::temp_dir().join(format!(
            "command-manager-db-open-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));

        let db = Db::open(path.clone()).expect("database with read-only readers should open");
        let version: i64 = db
            .read(|conn| {
                conn.query_row("SELECT version FROM schema_version", [], |row| row.get(0))
                    .map_err(Into::into)
            })
            .expect("reader should query the migrated database");
        assert_eq!(version, SCHEMA_VERSION);
        drop(db);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
    }

    #[test]
    fn deleting_group_keeps_history_and_nulls_group_id() {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys");
        migrate(&conn).expect("schema migration");
        conn.execute(
            "INSERT INTO command_group (id, group_name) VALUES ('group-1', 'Group')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO run_session (id, group_id, template_id, started_at, status)
             VALUES ('session-1', 'group-1', NULL, 'now', 'completed')",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM command_group WHERE id = 'group-1'", [])
            .unwrap();
        let group_id: Option<String> = conn
            .query_row(
                "SELECT group_id FROM run_session WHERE id = 'session-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(group_id, None);
    }

    #[test]
    fn shell_history_migration_adds_privacy_settings_and_typed_source() {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys");
        migrate(&conn).expect("schema migration");
        let enabled: String = conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'history_enabled'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(enabled, "true");
        let terminal_shell: String = conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'terminal_shell'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(terminal_shell.is_empty());
        let ghost_text: String = conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'ghost_text_enabled'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ghost_text, "true");
        conn.execute(
            "INSERT INTO command_history
             (id, command_line, shell_kind, first_used_at, last_used_at, source)
             VALUES ('typed-1', 'echo typed', 'cmd', 'now', 'now', 'typed')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn command_definition_migration_preserves_shell_kind() {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys");
        migrate(&conn).expect("schema migration");
        conn.execute(
            "INSERT INTO command_definition
             (id, name, execution_string, is_shell, shell_kind)
             VALUES ('command-1', 'PowerShell command', 'Get-Date', 1, 'pwsh')",
            [],
        )
        .expect("shell kind column should exist");
        let shell_kind: Option<String> = conn
            .query_row(
                "SELECT shell_kind FROM command_definition WHERE id = 'command-1'",
                [],
                |row| row.get(0),
            )
            .expect("shell kind should be readable");
        assert_eq!(shell_kind.as_deref(), Some("pwsh"));
    }

    #[test]
    fn quick_access_migration_defaults_to_off() {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("foreign keys");

        // Set up schema at version 9 (pre-quick_access)
        conn.execute_batch(INIT_SQL).expect("init sql");
        conn.execute_batch(MIGRATION_002_SQL).expect("migration 2");
        conn.execute_batch(MIGRATION_003_SQL).expect("migration 3");
        conn.execute_batch(MIGRATION_004_SQL).expect("migration 4");
        conn.execute_batch(MIGRATION_005_SQL).expect("migration 5");
        conn.execute_batch(MIGRATION_006_SQL).expect("migration 6");
        conn.execute_batch(MIGRATION_007_SQL).expect("migration 7");
        conn.execute_batch(MIGRATION_008_SQL).expect("migration 8");
        conn.execute_batch(MIGRATION_009_SQL).expect("migration 9");

        let initial_version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .expect("schema version should be 9");
        assert_eq!(initial_version, 9);

        // Insert existing commands in version 9 DB
        conn.execute(
            "INSERT INTO command_definition
             (id, name, execution_string, is_shell, shell_kind)
             VALUES ('cmd-existing-1', 'Git Status', 'git status -sb', 1, 'bash')",
            [],
        )
        .expect("command 1 insertion in v9");

        conn.execute(
            "INSERT INTO command_definition
             (id, name, execution_string, is_shell, shell_kind)
             VALUES ('cmd-existing-2', 'Direct Argv Tool', 'cargo test', 0, NULL)",
            [],
        )
        .expect("command 2 insertion in v9");

        // Migrate to version 10 (quick_access added)
        migrate(&conn).expect("schema migration 9 to 10");

        let migrated_version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .expect("schema version should be 10");
        assert_eq!(migrated_version, 10);

        // Verify existing commands preserve all columns and quick_access defaults to 0
        let (name1, exec1, is_shell1, shell_kind1, qa1): (
            String,
            String,
            i64,
            Option<String>,
            i64,
        ) = conn
            .query_row(
                "SELECT name, execution_string, is_shell, shell_kind, quick_access
                 FROM command_definition WHERE id = 'cmd-existing-1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("cmd-existing-1 should be readable");
        assert_eq!(name1, "Git Status");
        assert_eq!(exec1, "git status -sb");
        assert_eq!(is_shell1, 1);
        assert_eq!(shell_kind1.as_deref(), Some("bash"));
        assert_eq!(qa1, 0);

        let (name2, exec2, is_shell2, shell_kind2, qa2): (
            String,
            String,
            i64,
            Option<String>,
            i64,
        ) = conn
            .query_row(
                "SELECT name, execution_string, is_shell, shell_kind, quick_access
                 FROM command_definition WHERE id = 'cmd-existing-2'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("cmd-existing-2 should be readable");
        assert_eq!(name2, "Direct Argv Tool");
        assert_eq!(exec2, "cargo test");
        assert_eq!(is_shell2, 0);
        assert_eq!(shell_kind2, None);
        assert_eq!(qa2, 0);
    }
}
