use crate::error::{Error, Result};
use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

pub const AUTH_METHODS: &[&str] = &["auto", "agent", "key", "password"];
pub const SECRET_STORES: &[&str] = &["none", "keyring", "sqlite"];
pub const KEY_SOURCES: &[&str] = &["file", "content"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshHost {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    pub key_path: Option<String>,
    /// `file`: `key_path` (or ~/.ssh/id_*); `content`: the saved key text.
    pub key_source: String,
    /// Keychain identity: supplies username, password and (maybe) key.
    pub identity_id: Option<String>,
    /// Keychain key; wins over the identity's key and over `key_path`.
    pub key_id: Option<String>,
    pub jump_host_id: Option<String>,
    pub use_system_ssh: bool,
    pub agent_forwarding: bool,
    pub default_dir: Option<String>,
    pub secret_store: String,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub last_connected_at: Option<String>,
}

/// The fields a user edits in MOD-17. Everything else is owned by the app.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SshHostInput {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub username: String,
    #[serde(default = "default_auth")]
    pub auth_method: String,
    #[serde(default)]
    pub key_path: Option<String>,
    #[serde(default = "default_key_source")]
    pub key_source: String,
    #[serde(default)]
    pub identity_id: Option<String>,
    #[serde(default)]
    pub key_id: Option<String>,
    #[serde(default)]
    pub jump_host_id: Option<String>,
    #[serde(default)]
    pub use_system_ssh: bool,
    #[serde(default)]
    pub agent_forwarding: bool,
    #[serde(default)]
    pub default_dir: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_port() -> u16 {
    22
}

fn default_auth() -> String {
    "auto".into()
}

fn default_key_source() -> String {
    "file".into()
}

const COLUMNS: &str = "id, name, host, port, username, auth_method, key_path, jump_host_id, \
     use_system_ssh, agent_forwarding, default_dir, secret_store, color, tags, sort_order, \
     created_at, last_connected_at, key_source, identity_id, key_id";

fn from_row(row: &Row) -> rusqlite::Result<SshHost> {
    let tags: String = row.get(13)?;
    Ok(SshHost {
        id: row.get(0)?,
        name: row.get(1)?,
        host: row.get(2)?,
        port: row.get::<_, i64>(3)?.clamp(1, 65535) as u16,
        username: row.get(4)?,
        auth_method: row.get(5)?,
        key_path: row.get(6)?,
        jump_host_id: row.get(7)?,
        use_system_ssh: row.get::<_, i64>(8)? != 0,
        agent_forwarding: row.get::<_, i64>(9)? != 0,
        default_dir: row.get(10)?,
        secret_store: row.get(11)?,
        color: row.get(12)?,
        tags: split_tags(&tags),
        sort_order: row.get(14)?,
        created_at: row.get(15)?,
        last_connected_at: row.get(16)?,
        key_source: row.get(17)?,
        identity_id: row.get(18)?,
        key_id: row.get(19)?,
    })
}

fn split_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn list(conn: &Connection) -> Result<Vec<SshHost>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM ssh_host ORDER BY sort_order, name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<SshHost>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM ssh_host WHERE id = ?1"))?;
    Ok(stmt.query_row([id], from_row).optional()?)
}

pub fn require(conn: &Connection, id: &str) -> Result<SshHost> {
    get(conn, id)?.ok_or_else(|| Error::msg("Không tìm thấy máy chủ SSH"))
}

/// Trims and checks an edited profile. Rules that need other rows (jump
/// hosts) are checked against `conn`; `id` is the host being edited.
pub fn normalize(conn: &Connection, id: Option<&str>, input: SshHostInput) -> Result<SshHostInput> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(Error::msg("Tên hiển thị không được để trống"));
    }
    if name.chars().count() > 100 {
        return Err(Error::msg("Tên hiển thị dài quá 100 ký tự"));
    }

    let mut host = input.host.trim().to_string();
    if host.starts_with('[') && host.ends_with(']') {
        host = host[1..host.len() - 1].to_string();
    }
    if host.is_empty() {
        return Err(Error::msg("Máy chủ không được để trống"));
    }
    if host.len() > 255 || host.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(Error::msg("Địa chỉ máy chủ không hợp lệ"));
    }
    if input.port == 0 {
        return Err(Error::msg("Cổng phải nằm trong khoảng 1–65535"));
    }

    let identity_id = non_empty(input.identity_id);
    if let Some(identity_id) = &identity_id {
        super::ssh_keychain::require_identity(conn, identity_id)?;
    }
    let key_id = non_empty(input.key_id);
    if let Some(key_id) = &key_id {
        super::ssh_keychain::require_key(conn, key_id)?;
        if input.use_system_ssh {
            return Err(Error::msg(
                "OpenSSH hệ thống không dùng được khóa trong Keychain, hãy chọn tệp khóa",
            ));
        }
    }

    // With an identity the username comes from it.
    let username = input.username.trim().to_string();
    if username.is_empty() && identity_id.is_none() {
        return Err(Error::msg("Người dùng không được để trống"));
    }
    if username.len() > 256
        || username
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(Error::msg("Tên người dùng không hợp lệ"));
    }

    if !AUTH_METHODS.contains(&input.auth_method.as_str()) {
        return Err(Error::msg(format!(
            "Phương thức xác thực không hợp lệ: {}",
            input.auth_method
        )));
    }

    // Empty means "try ~/.ssh/id_*", even with auth_method = "key".
    let key_path = non_empty(input.key_path);
    if !KEY_SOURCES.contains(&input.key_source.as_str()) {
        return Err(Error::msg(format!(
            "Nguồn khóa không hợp lệ: {}",
            input.key_source
        )));
    }
    if input.use_system_ssh && input.key_source == "content" {
        return Err(Error::msg(
            "OpenSSH hệ thống chỉ dùng được tệp khóa, không dùng được nội dung khóa dán vào",
        ));
    }
    let default_dir = non_empty(input.default_dir);
    if default_dir.as_deref().is_some_and(|dir| dir.len() > 4096) {
        return Err(Error::msg("Thư mục mặc định quá dài"));
    }

    let color = non_empty(input.color);
    if let Some(color) = &color {
        let valid = color.len() == 7
            && color.starts_with('#')
            && color[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !valid {
            return Err(Error::msg("Màu nhận diện phải có dạng #rrggbb"));
        }
    }

    let mut tags: Vec<String> = Vec::new();
    for tag in input.tags.iter().flat_map(|tag| tag.split(',')) {
        let tag = tag.trim();
        if tag.is_empty() || tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
            continue;
        }
        if tag.chars().count() > 40 {
            return Err(Error::msg("Tag dài quá 40 ký tự"));
        }
        tags.push(tag.to_string());
    }

    let jump_host_id = non_empty(input.jump_host_id);
    if let Some(jump_id) = &jump_host_id {
        if Some(jump_id.as_str()) == id {
            return Err(Error::msg("Máy chủ không thể đi qua chính nó"));
        }
        let chain = jump_chain(conn, jump_id, id)?;
        if let Some(system) = chain.iter().find(|jump| jump.use_system_ssh) {
            return Err(Error::msg(format!(
                "Jump host {} dùng chế độ OpenSSH hệ thống nên không đi qua được",
                system.name
            )));
        }
        // Hosts already behind this one get the new hops too.
        let below = match id {
            Some(id) => depth_below(conn, id)?,
            None => 0,
        };
        if chain.len() + below > MAX_JUMP_HOPS {
            return Err(Error::msg(format!(
                "Chỉ hỗ trợ đi qua tối đa {MAX_JUMP_HOPS} jump host liên tiếp"
            )));
        }
    }
    if input.use_system_ssh {
        if let Some(id) = id {
            let used_as_jump: i64 = conn.query_row(
                "SELECT COUNT(*) FROM ssh_host WHERE jump_host_id = ?1",
                [id],
                |row| row.get(0),
            )?;
            if used_as_jump > 0 {
                return Err(Error::msg(
                    "Máy chủ đang là jump host nên không thể chuyển sang OpenSSH hệ thống",
                ));
            }
        }
    }

    Ok(SshHostInput {
        name,
        host,
        port: input.port,
        username,
        auth_method: input.auth_method,
        key_path,
        key_source: input.key_source,
        identity_id,
        key_id,
        jump_host_id,
        use_system_ssh: input.use_system_ssh,
        agent_forwarding: input.agent_forwarding,
        default_dir,
        color,
        tags,
    })
}

/// Jump hosts in a row are capped: every hop is one more login and one more
/// channel inside a channel.
pub const MAX_JUMP_HOPS: usize = 5;

/// The jump hosts on the way to a host whose jump host is `first`, nearest
/// first (`first`, its jump host, ...). Fails on a loop back to `editing`
/// (the host being saved) or within the saved rows.
pub fn jump_chain(conn: &Connection, first: &str, editing: Option<&str>) -> Result<Vec<SshHost>> {
    let mut chain: Vec<SshHost> = Vec::new();
    let mut next = Some(first.to_string());
    while let Some(id) = next {
        if Some(id.as_str()) == editing || chain.iter().any(|hop| hop.id == id) {
            return Err(Error::msg(
                "Chuỗi jump host bị vòng lặp: máy chủ này đã nằm trên đường đi",
            ));
        }
        if chain.len() > MAX_JUMP_HOPS {
            return Err(Error::msg(format!(
                "Chỉ hỗ trợ đi qua tối đa {MAX_JUMP_HOPS} jump host liên tiếp"
            )));
        }
        let hop = get(conn, &id)?.ok_or_else(|| Error::msg("Không tìm thấy jump host đã chọn"))?;
        next = hop.jump_host_id.clone();
        chain.push(hop);
    }
    Ok(chain)
}

/// How many hosts deep the hosts reaching the network through `id` go
/// (0 when nothing uses it as a jump host).
fn depth_below(conn: &Connection, id: &str) -> Result<usize> {
    let mut stmt = conn.prepare("SELECT id FROM ssh_host WHERE jump_host_id = ?1")?;
    let mut level = vec![id.to_string()];
    let mut seen = level.clone();
    let mut depth = 0;
    loop {
        let mut next = Vec::new();
        for parent in &level {
            let rows = stmt.query_map([parent], |row| row.get::<_, String>(0))?;
            for child in rows {
                let child = child?;
                // A loop in old rows must not spin forever.
                if !seen.contains(&child) {
                    seen.push(child.clone());
                    next.push(child);
                }
            }
        }
        if next.is_empty() {
            return Ok(depth);
        }
        depth += 1;
        level = next;
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn insert(conn: &Connection, id: &str, input: &SshHostInput, created_at: &str) -> Result<()> {
    let sort_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM ssh_host",
        [],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO ssh_host (id, name, host, port, username, auth_method, key_path, jump_host_id,
            use_system_ssh, agent_forwarding, default_dir, color, tags, sort_order, created_at,
            key_source, identity_id, key_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        rusqlite::params![
            id,
            input.name,
            input.host,
            input.port as i64,
            input.username,
            input.auth_method,
            input.key_path,
            input.jump_host_id,
            input.use_system_ssh as i64,
            input.agent_forwarding as i64,
            input.default_dir,
            input.color,
            input.tags.join(","),
            sort_order,
            created_at,
            input.key_source,
            input.identity_id,
            input.key_id,
        ],
    )?;
    Ok(())
}

pub fn update(conn: &Connection, id: &str, input: &SshHostInput) -> Result<()> {
    let n = conn.execute(
        "UPDATE ssh_host SET name = ?2, host = ?3, port = ?4, username = ?5, auth_method = ?6,
            key_path = ?7, jump_host_id = ?8, use_system_ssh = ?9, agent_forwarding = ?10,
            default_dir = ?11, color = ?12, tags = ?13, key_source = ?14,
            identity_id = ?15, key_id = ?16
         WHERE id = ?1",
        rusqlite::params![
            id,
            input.name,
            input.host,
            input.port as i64,
            input.username,
            input.auth_method,
            input.key_path,
            input.jump_host_id,
            input.use_system_ssh as i64,
            input.agent_forwarding as i64,
            input.default_dir,
            input.color,
            input.tags.join(","),
            input.key_source,
            input.identity_id,
            input.key_id,
        ],
    )?;
    if n == 0 {
        return Err(Error::msg("Không tìm thấy máy chủ SSH"));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM ssh_host WHERE id = ?1", [id])?;
    Ok(())
}

pub fn set_secret_store(conn: &Connection, id: &str, store: &str) -> Result<()> {
    if !SECRET_STORES.contains(&store) {
        return Err(Error::msg(format!(
            "Nơi lưu mật khẩu không hợp lệ: {store}"
        )));
    }
    conn.execute(
        "UPDATE ssh_host SET secret_store = ?2 WHERE id = ?1",
        rusqlite::params![id, store],
    )?;
    Ok(())
}

pub fn touch_connected(conn: &Connection, id: &str, at: &str) -> Result<()> {
    conn.execute(
        "UPDATE ssh_host SET last_connected_at = ?2 WHERE id = ?1",
        rusqlite::params![id, at],
    )?;
    Ok(())
}

pub struct SealedSecret {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub fn secret_get(conn: &Connection, host_id: &str, kind: &str) -> Result<Option<SealedSecret>> {
    Ok(conn
        .query_row(
            "SELECT nonce, ciphertext FROM ssh_secret WHERE host_id = ?1 AND kind = ?2",
            [host_id, kind],
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
    host_id: &str,
    kind: &str,
    sealed: &SealedSecret,
    at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO ssh_secret (host_id, kind, nonce, ciphertext, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(host_id, kind) DO UPDATE SET
           nonce = excluded.nonce, ciphertext = excluded.ciphertext, updated_at = excluded.updated_at",
        rusqlite::params![host_id, kind, sealed.nonce, sealed.ciphertext, at],
    )?;
    Ok(())
}

pub fn secret_delete(conn: &Connection, host_id: &str, kind: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM ssh_secret WHERE host_id = ?1 AND kind = ?2",
        [host_id, kind],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().expect("db");
        conn.pragma_update(None, "foreign_keys", "ON").expect("fk");
        crate::db::pool::migrate(&conn).expect("migrate");
        conn
    }

    fn input(name: &str) -> SshHostInput {
        SshHostInput {
            name: name.into(),
            host: "10.0.0.1".into(),
            port: 22,
            username: "deploy".into(),
            auth_method: "auto".into(),
            key_source: "file".into(),
            ..Default::default()
        }
    }

    fn add(conn: &Connection, id: &str, input: SshHostInput) {
        let input = normalize(conn, None, input).expect("valid");
        insert(conn, id, &input, "0").expect("insert");
    }

    #[test]
    fn normalize_trims_and_dedupes_tags() {
        let conn = conn();
        let mut raw = input("  prod  ");
        raw.host = " [::1] ".into();
        raw.tags = vec!["prod, db".into(), "PROD".into(), " ".into()];
        raw.key_path = Some("  ".into());
        let clean = normalize(&conn, None, raw).expect("valid");
        assert_eq!(clean.name, "prod");
        assert_eq!(clean.host, "::1");
        assert_eq!(clean.tags, vec!["prod", "db"]);
        assert_eq!(clean.key_path, None);
    }

    #[test]
    fn normalize_rejects_bad_fields() {
        let conn = conn();
        type Edit = Box<dyn Fn(&mut SshHostInput)>;
        let cases: Vec<Edit> = vec![
            Box::new(|i| i.name = " ".into()),
            Box::new(|i| i.host = "bad host".into()),
            Box::new(|i| i.port = 0),
            Box::new(|i| i.username = "a b".into()),
            Box::new(|i| i.auth_method = "gssapi".into()),
            Box::new(|i| i.color = Some("red".into())),
            Box::new(|i| i.key_source = "inline".into()),
            Box::new(|i| {
                i.use_system_ssh = true;
                i.key_source = "content".into();
            }),
        ];
        for edit in cases {
            let mut raw = input("x");
            edit(&mut raw);
            assert!(normalize(&conn, None, raw).is_err());
        }
    }

    #[test]
    fn jump_chains_reject_loops_and_long_chains() {
        let conn = conn();
        add(&conn, "j0", input("j0"));
        for n in 1..MAX_JUMP_HOPS {
            let mut hop = input(&format!("j{n}"));
            hop.jump_host_id = Some(format!("j{}", n - 1));
            add(&conn, &format!("j{n}"), hop);
        }
        let last = format!("j{}", MAX_JUMP_HOPS - 1);
        let chain = jump_chain(&conn, &last, None).expect("chain");
        assert_eq!(chain.len(), MAX_JUMP_HOPS);
        assert_eq!(chain[0].id, last);
        assert_eq!(chain[MAX_JUMP_HOPS - 1].id, "j0");

        // MAX_JUMP_HOPS jump hosts in front of a host is fine, one more is not.
        let mut deep = input("deep");
        deep.jump_host_id = Some(last.clone());
        add(&conn, "deep", deep);
        let mut deeper = input("deeper");
        deeper.jump_host_id = Some("deep".into());
        assert!(normalize(&conn, None, deeper).is_err());

        // Giving j0 a jump host would push `deep` one hop too far.
        add(&conn, "outer", input("outer"));
        let mut j0 = input("j0");
        j0.jump_host_id = Some("outer".into());
        assert!(normalize(&conn, Some("j0"), j0).is_err());

        // Loops: through itself, or through a host that is behind it.
        let mut own = input("j1");
        own.jump_host_id = Some("j1".into());
        assert!(normalize(&conn, Some("j1"), own).is_err());
        let mut looped = input("j1");
        looped.jump_host_id = Some("j3".into());
        assert!(normalize(&conn, Some("j1"), looped).is_err());

        // A system-OpenSSH host cannot be a hop.
        let mut system = input("system");
        system.use_system_ssh = true;
        add(&conn, "system", system);
        let mut behind = input("behind");
        behind.jump_host_id = Some("system".into());
        assert!(normalize(&conn, None, behind).is_err());

        // ... but it can reach its server through a chain.
        let mut system_behind = input("system2");
        system_behind.use_system_ssh = true;
        system_behind.jump_host_id = Some("j2".into());
        assert!(normalize(&conn, None, system_behind).is_ok());
    }

    #[test]
    fn crud_round_trip_and_cascade() {
        let conn = conn();
        let mut raw = input("prod");
        raw.tags = vec!["a".into(), "b".into()];
        add(&conn, "h1", raw);
        let host = require(&conn, "h1").expect("host");
        assert_eq!(host.tags, vec!["a", "b"]);
        assert_eq!(host.secret_store, "none");

        let sealed = SealedSecret {
            nonce: vec![1; 12],
            ciphertext: vec![2; 20],
        };
        secret_put(&conn, "h1", "password", &sealed, "1").expect("secret");
        assert!(secret_get(&conn, "h1", "password").expect("get").is_some());

        delete(&conn, "h1").expect("delete");
        assert!(get(&conn, "h1").expect("get").is_none());
        assert!(secret_get(&conn, "h1", "password").expect("get").is_none());
    }
}
