use crate::db;
use crate::db::schema::SCHEMA_VERSION;
use crate::error::{Error, Result};
use crate::process::manager::ProcessManager;
use crate::process::shutdown;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

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

    let staged =
        std::env::temp_dir().join(format!("command-manager-restore-{}.sqlite", Uuid::new_v4()));
    fs::copy(src, &staged)?;
    let migration_result = db::pool::migrate_file(&staged);
    if let Err(error) = migration_result {
        let _ = fs::remove_file(&staged);
        return Err(Error::msg(format!("backup migration failed: {error}")));
    }
    if db::pool::read_schema_version(&staged)? != SCHEMA_VERSION
        || !db::pool::integrity_ok(&staged)?
    {
        let _ = fs::remove_file(&staged);
        return Err(Error::msg("backup schema or integrity is incompatible"));
    }

    shutdown::shutdown_all(processes, Duration::from_secs(8));

    let db_path = db.path.clone();
    let rollback = rollback_path(&db_path);
    db.write(|conn| {
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        Ok(())
    })?;

    db.close_all()?;
    if let Err(error) = snapshot_live_db(&db_path, &rollback) {
        let _ = fs::remove_file(&staged);
        return match db.reopen() {
            Ok(()) => Err(error),
            Err(reopen_error) => Err(Error::msg(format!(
                "rollback snapshot failed: {error}; reopen failed: {reopen_error}"
            ))),
        };
    }

    let replacement = (|| {
        drop_wal_set(&db_path)?;
        fs::copy(&staged, &db_path)?;
        Ok::<(), Error>(())
    })();

    if let Err(replace_error) = replacement {
        let _ = fs::remove_file(&staged);
        let rollback_result = restore_rollback(&db_path, &rollback);
        let reopen_result = db.reopen();
        return match (rollback_result, reopen_result) {
            (Ok(()), Ok(())) => Err(Error::msg(format!(
                "import failed, rolled back: {replace_error}"
            ))),
            (rollback_error, reopen_error) => Err(Error::msg(format!(
                "import failed: {replace_error}; rollback: {rollback_error:?}; reopen: {reopen_error:?}"
            ))),
        };
    }
    let _ = fs::remove_file(&staged);

    match db.reopen() {
        Ok(()) => {
            let _ = fs::remove_file(&rollback);
            Ok(())
        }
        Err(e) => {
            let _ = restore_rollback(&db_path, &rollback);
            match db.reopen() {
                Ok(()) => Err(Error::msg(format!("import failed, rolled back: {e}"))),
                Err(reopen_error) => Err(Error::msg(format!(
                    "import failed and rollback reopen failed: {e}; {reopen_error}"
                ))),
            }
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
    for path in [db_path.to_path_buf(), wal_path(db_path), shm_path(db_path)] {
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::export;
    use crate::db::repos::history;
    use crate::db::Db;
    use crate::process::manager::ProcessManager;

    #[test]
    fn restores_snapshot_with_command_history() {
        let dir = tempfile::tempdir().unwrap();
        let live = Db::open(dir.path().join("live.db")).unwrap();
        let source = Db::open(dir.path().join("source.db")).unwrap();
        source
            .write(|conn| history::record(conn, "echo restored", "shell", None, "command", "now"))
            .unwrap();
        let backup = dir.path().join("backup.sqlite");
        export::export(&source, &backup).unwrap();

        import(&live, &ProcessManager::new(), &backup, true).unwrap();

        let restored = live
            .read(|conn| history::list(conn, Some("restored"), 10))
            .unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].command_line, "echo restored");
    }
}
