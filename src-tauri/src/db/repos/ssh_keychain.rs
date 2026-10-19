//! Keychain rows: SSH keys and identities that many hosts can share.
//! Secret values (key text, passphrases, passwords) are never stored here in
//! clear; see `ssh::secret`.

use super::ssh_hosts::SealedSecret;
use crate::error::{Error, Result};
use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshKey {
    pub id: String,
    pub name: String,
    pub algorithm: String,
    /// OpenSSH public key line, ready for `authorized_keys`.
    pub public_key: String,
    pub fingerprint: String,
    /// The stored key text needs a passphrase.
    pub encrypted: bool,
    pub secret_store: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshIdentity {
    pub id: String,
    pub name: String,
    pub username: String,
    pub key_id: Option<String>,
    pub secret_store: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SshIdentityInput {
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub key_id: Option<String>,
}

fn key_from_row(row: &Row) -> rusqlite::Result<SshKey> {
    Ok(SshKey {
        id: row.get(0)?,
        name: row.get(1)?,
        algorithm: row.get(2)?,
        public_key: row.get(3)?,
        fingerprint: row.get(4)?,
        encrypted: row.get::<_, i64>(5)? != 0,
        secret_store: row.get(6)?,
        created_at: row.get(7)?,
    })
}

const KEY_COLUMNS: &str =
    "id, name, algorithm, public_key, fingerprint, encrypted, secret_store, created_at";

pub fn list_keys(conn: &Connection) -> Result<Vec<SshKey>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {KEY_COLUMNS} FROM ssh_key ORDER BY name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([], key_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_key(conn: &Connection, id: &str) -> Result<Option<SshKey>> {
    let mut stmt = conn.prepare(&format!("SELECT {KEY_COLUMNS} FROM ssh_key WHERE id = ?1"))?;
    Ok(stmt.query_row([id], key_from_row).optional()?)
}

pub fn require_key(conn: &Connection, id: &str) -> Result<SshKey> {
    get_key(conn, id)?.ok_or_else(|| Error::msg("Không tìm thấy khóa trong Keychain"))
}

pub fn insert_key(conn: &Connection, key: &SshKey) -> Result<()> {
    conn.execute(
        &format!("INSERT INTO ssh_key ({KEY_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"),
        rusqlite::params![
            key.id,
            key.name,
            key.algorithm,
            key.public_key,
            key.fingerprint,
            key.encrypted as i64,
            key.secret_store,
            key.created_at,
        ],
    )?;
    Ok(())
}

pub fn rename_key(conn: &Connection, id: &str, name: &str) -> Result<()> {
    let n = conn.execute(
        "UPDATE ssh_key SET name = ?2 WHERE id = ?1",
        rusqlite::params![id, name],
    )?;
    if n == 0 {
        return Err(Error::msg("Không tìm thấy khóa trong Keychain"));
    }
    Ok(())
}

pub fn set_key_store(conn: &Connection, id: &str, store: &str) -> Result<()> {
    if !matches!(store, "keyring" | "sqlite") {
        return Err(Error::msg(
            "Khóa trong Keychain phải được lưu (Keyring hoặc SQLite)",
        ));
    }
    conn.execute(
        "UPDATE ssh_key SET secret_store = ?2 WHERE id = ?1",
        rusqlite::params![id, store],
    )?;
    Ok(())
}

pub fn delete_key(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM ssh_key WHERE id = ?1", [id])?;
    Ok(())
}

fn identity_from_row(row: &Row) -> rusqlite::Result<SshIdentity> {
    Ok(SshIdentity {
        id: row.get(0)?,
        name: row.get(1)?,
        username: row.get(2)?,
        key_id: row.get(3)?,
        secret_store: row.get(4)?,
        created_at: row.get(5)?,
    })
}

const IDENTITY_COLUMNS: &str = "id, name, username, key_id, secret_store, created_at";

pub fn list_identities(conn: &Connection) -> Result<Vec<SshIdentity>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {IDENTITY_COLUMNS} FROM ssh_identity ORDER BY name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([], identity_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_identity(conn: &Connection, id: &str) -> Result<Option<SshIdentity>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {IDENTITY_COLUMNS} FROM ssh_identity WHERE id = ?1"
    ))?;
    Ok(stmt.query_row([id], identity_from_row).optional()?)
}

pub fn require_identity(conn: &Connection, id: &str) -> Result<SshIdentity> {
    get_identity(conn, id)?.ok_or_else(|| Error::msg("Không tìm thấy identity trong Keychain"))
}

pub fn normalize_identity(conn: &Connection, input: SshIdentityInput) -> Result<SshIdentityInput> {
    let name = input.name.trim().to_string();
    let username = input.username.trim().to_string();
    if username.is_empty() {
        return Err(Error::msg("Tên đăng nhập không được để trống"));
    }
    if username.len() > 256
        || username
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(Error::msg("Tên đăng nhập không hợp lệ"));
    }
    if name.chars().count() > 100 {
        return Err(Error::msg("Tên identity dài quá 100 ký tự"));
    }
    let key_id = input
        .key_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty());
    if let Some(key_id) = &key_id {
        require_key(conn, key_id)?;
    }
    Ok(SshIdentityInput {
        // Termius shows the username when an identity has no label.
        name: if name.is_empty() {
            username.clone()
        } else {
            name
        },
        username,
        key_id,
    })
}

pub fn insert_identity(
    conn: &Connection,
    id: &str,
    input: &SshIdentityInput,
    created_at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO ssh_identity (id, name, username, key_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, input.name, input.username, input.key_id, created_at],
    )?;
    Ok(())
}

pub fn update_identity(conn: &Connection, id: &str, input: &SshIdentityInput) -> Result<()> {
    let n = conn.execute(
        "UPDATE ssh_identity SET name = ?2, username = ?3, key_id = ?4 WHERE id = ?1",
        rusqlite::params![id, input.name, input.username, input.key_id],
    )?;
    if n == 0 {
        return Err(Error::msg("Không tìm thấy identity trong Keychain"));
    }
    Ok(())
}

pub fn set_identity_store(conn: &Connection, id: &str, store: &str) -> Result<()> {
    conn.execute(
        "UPDATE ssh_identity SET secret_store = ?2 WHERE id = ?1",
        rusqlite::params![id, store],
    )?;
    Ok(())
}

pub fn delete_identity(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM ssh_identity WHERE id = ?1", [id])?;
    Ok(())
}

/// How many hosts and identities point at each key / identity, so the UI can
/// warn before deleting one that is in use.
#[derive(Debug, Default, Clone, Copy, Serialize, PartialEq)]
pub struct Usage {
    pub hosts: usize,
    pub identities: usize,
}

pub fn key_usage(conn: &Connection) -> Result<HashMap<String, Usage>> {
    let mut usage: HashMap<String, Usage> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT key_id, COUNT(*) FROM ssh_host WHERE key_id IS NOT NULL GROUP BY key_id",
    )?;
    for row in stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })? {
        let (id, count) = row?;
        usage.entry(id).or_default().hosts = count as usize;
    }
    let mut stmt = conn.prepare(
        "SELECT key_id, COUNT(*) FROM ssh_identity WHERE key_id IS NOT NULL GROUP BY key_id",
    )?;
    for row in stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })? {
        let (id, count) = row?;
        usage.entry(id).or_default().identities = count as usize;
    }
    Ok(usage)
}

pub fn identity_usage(conn: &Connection) -> Result<HashMap<String, usize>> {
    let mut stmt = conn.prepare(
        "SELECT identity_id, COUNT(*) FROM ssh_host WHERE identity_id IS NOT NULL
         GROUP BY identity_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
    })?;
    Ok(rows.collect::<rusqlite::Result<HashMap<_, _>>>()?)
}

pub fn secret_get(
    conn: &Connection,
    owner_kind: &str,
    owner_id: &str,
    kind: &str,
) -> Result<Option<SealedSecret>> {
    Ok(conn
        .query_row(
            "SELECT nonce, ciphertext FROM ssh_keychain_secret
             WHERE owner_kind = ?1 AND owner_id = ?2 AND kind = ?3",
            [owner_kind, owner_id, kind],
            |row| {
                Ok(SealedSecret {
                    nonce: row.get(0)?,
                    ciphertext: row.get(1)?,
                })
            },
        )
        .optional()?)
}

pub fn secret_put(
    conn: &Connection,
    owner_kind: &str,
    owner_id: &str,
    kind: &str,
    sealed: &SealedSecret,
    at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO ssh_keychain_secret (owner_kind, owner_id, kind, nonce, ciphertext, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(owner_kind, owner_id, kind) DO UPDATE SET
           nonce = excluded.nonce, ciphertext = excluded.ciphertext, updated_at = excluded.updated_at",
        rusqlite::params![owner_kind, owner_id, kind, sealed.nonce, sealed.ciphertext, at],
    )?;
    Ok(())
}

pub fn secret_delete(
    conn: &Connection,
    owner_kind: &str,
    owner_id: &str,
    kind: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM ssh_keychain_secret WHERE owner_kind = ?1 AND owner_id = ?2 AND kind = ?3",
        [owner_kind, owner_id, kind],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::ssh_hosts;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().expect("db");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        crate::db::pool::migrate(&conn).expect("migrate");
        conn
    }

    fn key(id: &str) -> SshKey {
        SshKey {
            id: id.into(),
            name: "laptop".into(),
            algorithm: "ssh-ed25519".into(),
            public_key: "ssh-ed25519 AAAA laptop".into(),
            fingerprint: "SHA256:x".into(),
            encrypted: false,
            secret_store: "sqlite".into(),
            created_at: "0".into(),
        }
    }

    #[test]
    fn deleting_a_key_detaches_users_and_drops_its_secrets() {
        let conn = conn();
        insert_key(&conn, &key("k1")).expect("key");
        let identity = normalize_identity(
            &conn,
            SshIdentityInput {
                name: " ".into(),
                username: " deploy ".into(),
                key_id: Some("k1".into()),
            },
        )
        .expect("identity");
        assert_eq!(identity.name, "deploy");
        insert_identity(&conn, "i1", &identity, "0").expect("insert identity");
        let host = ssh_hosts::SshHostInput {
            name: "prod".into(),
            host: "10.0.0.1".into(),
            port: 22,
            username: "deploy".into(),
            auth_method: "auto".into(),
            key_source: "file".into(),
            ..Default::default()
        };
        ssh_hosts::insert(&conn, "h1", &host, "0").expect("host");
        conn.execute(
            "UPDATE ssh_host SET key_id = 'k1', identity_id = 'i1' WHERE id = 'h1'",
            [],
        )
        .expect("link");
        let sealed = SealedSecret {
            nonce: vec![0; 12],
            ciphertext: vec![1],
        };
        secret_put(&conn, "key", "k1", "private_key", &sealed, "0").expect("secret");

        assert_eq!(
            key_usage(&conn).expect("usage").get("k1"),
            Some(&Usage {
                hosts: 1,
                identities: 1
            })
        );
        assert_eq!(identity_usage(&conn).expect("usage").get("i1"), Some(&1));

        delete_key(&conn, "k1").expect("delete");
        assert!(secret_get(&conn, "key", "k1", "private_key")
            .expect("get")
            .is_none());
        let host = ssh_hosts::require(&conn, "h1").expect("host");
        assert_eq!(host.key_id, None);
        assert_eq!(
            require_identity(&conn, "i1").expect("identity").key_id,
            None
        );

        delete_identity(&conn, "i1").expect("delete identity");
        assert_eq!(
            ssh_hosts::require(&conn, "h1").expect("host").identity_id,
            None
        );
    }

    #[test]
    fn identity_needs_a_username_and_a_real_key() {
        let conn = conn();
        assert!(normalize_identity(
            &conn,
            SshIdentityInput {
                name: "x".into(),
                username: " ".into(),
                key_id: None
            }
        )
        .is_err());
        assert!(normalize_identity(
            &conn,
            SshIdentityInput {
                name: "x".into(),
                username: "root".into(),
                key_id: Some("missing".into())
            }
        )
        .is_err());
    }
}
