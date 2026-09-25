use crate::db;
use crate::error::{Error, Result};
use std::fs;
use std::path::Path;

pub fn export(db: &db::Db, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let dest_s = dest.to_string_lossy().replace('\'', "''");
    db.write(|conn| {
        conn.execute_batch(&format!("VACUUM INTO '{dest_s}'"))?;
        Ok(())
    })?;
    if !db::pool::integrity_ok(dest)? {
        let _ = fs::remove_file(dest);
        return Err(Error::msg("backup failed integrity_check"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vacuum_into_passes_integrity() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("app.db");
        let dest = dir.path().join("snap.db");
        let db = crate::db::Db::open(src).unwrap();
        export(&db, &dest).unwrap();
        assert!(dest.exists());
        assert!(crate::db::pool::integrity_ok(&dest).unwrap());
    }
}
