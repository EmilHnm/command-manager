use super::schema::{INIT_SQL, SCHEMA_VERSION};
use crate::error::{Error, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// One writer + three readers on the same WAL file.
pub struct Db {
    pub path: PathBuf,
    writer: Mutex<Option<Connection>>,
    readers: [Mutex<Option<Connection>>; 3],
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
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
    configure(&conn)?;
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
        })
    }

    pub fn write<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let guard = self.writer.lock().map_err(|_| Error::msg("db writer poisoned"))?;
        let conn = guard.as_ref().ok_or_else(|| Error::msg("db closed"))?;
        f(conn)
    }

    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        // ponytail: round-robin skipped; reader[0] is enough until contention shows up
        let guard = self.readers[0]
            .lock()
            .map_err(|_| Error::msg("db reader poisoned"))?;
        let conn = guard.as_ref().ok_or_else(|| Error::msg("db closed"))?;
        f(conn)
    }

    /// Drop every connection so WAL files can be replaced. Caller must `reopen`.
    pub fn close_all(&self) -> Result<()> {
        let mut w = self.writer.lock().map_err(|_| Error::msg("db writer poisoned"))?;
        *w = None;
        for r in &self.readers {
            let mut g = r.lock().map_err(|_| Error::msg("db reader poisoned"))?;
            *g = None;
        }
        Ok(())
    }

    pub fn reopen(&self) -> Result<()> {
        let mut w = self.writer.lock().map_err(|_| Error::msg("db writer poisoned"))?;
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

fn migrate(conn: &Connection) -> Result<()> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        conn.execute_batch(INIT_SQL)?;
        return Ok(());
    }
    let version: i64 = conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
        row.get(0)
    })?;
    if version > SCHEMA_VERSION {
        return Err(Error::msg(format!(
            "database schema {version} is newer than app {SCHEMA_VERSION}"
        )));
    }
    if version < SCHEMA_VERSION {
        return Err(Error::msg(format!(
            "database schema {version} needs migration to {SCHEMA_VERSION} (not implemented)"
        )));
    }
    Ok(())
}

pub fn integrity_ok(path: &Path) -> Result<bool> {
    let conn = Connection::open(path)?;
    let msg: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    Ok(msg == "ok")
}

pub fn read_schema_version(path: &Path) -> Result<i64> {
    let conn = Connection::open(path)?;
    conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
        .map_err(Into::into)
}
