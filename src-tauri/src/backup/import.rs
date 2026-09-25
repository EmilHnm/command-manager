use crate::db;
use crate::db::schema::SCHEMA_VERSION;
use crate::error::{Error, Result};
use crate::process::manager::ProcessManager;
use crate::process::shutdown;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn import(db: &db::Db, processes: &ProcessManager, src: &Path, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(Error::ConfirmationRequired);
    }
    if !src.exists() {
        return Err(Error::msg("backup file not found"));
    }
    if !db::pool::integrity_ok(src)? {
        return Err(Error::msg("backup failed integrity_check"));
    }
    let version = db::pool::read_schema_version(src)?;
    if version != SCHEMA_VERSION {
        return Err(Error::msg(format!(
            "backup schema {version} incompatible with app {SCHEMA_VERSION}"
        )));
    }

    shutdown::shutdown_all(processes, Duration::from_secs(8));

    let db_path = db.path.clone();
    let rollback = rollback_path(&db_path);
    snapshot_live_db(&db_path, &rollback)?;

    db.close_all()?;
    drop_wal_set(&db_path)?;
    fs::copy(src, &db_path)?;

    match db.reopen() {
        Ok(()) => {
            let _ = fs::remove_file(&rollback);
            Ok(())
        }
        Err(e) => {
            let _ = restore_rollback(&db_path, &rollback);
            let _ = db.reopen();
            Err(Error::msg(format!("import failed, rolled back: {e}")))
        }
    }
}

fn rollback_path(db_path: &Path) -> PathBuf {
    db_path.with_extension("db.rollback")
}

fn wal_path(db_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", db_path.display()))
}

fn shm_path(db_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-shm", db_path.display()))
}

fn drop_wal_set(db_path: &Path) -> Result<()> {
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(wal_path(db_path));
    let _ = fs::remove_file(shm_path(db_path));
    Ok(())
}

fn snapshot_live_db(db_path: &Path, rollback: &Path) -> Result<()> {
    if db_path.exists() {
        fs::copy(db_path, rollback)?;
    }
    Ok(())
}

fn restore_rollback(db_path: &Path, rollback: &Path) -> Result<()> {
    drop_wal_set(db_path)?;
    if rollback.exists() {
        fs::copy(rollback, db_path)?;
    }
    Ok(())
}
