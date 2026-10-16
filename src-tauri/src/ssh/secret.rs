//! Saved SSH passwords, key passphrases and private keys.
//!
//! Secrets belong to a host, a Keychain key or a Keychain identity
//! ([`SecretOwner`]). The user picks where they live (MOD-17 / MOD-19):
//! * `keyring`: the OS credential store. Preferred, but not every machine
//!   has one (a Linux session without Secret Service).
//! * `sqlite`: the `ssh_secret` table, sealed with AES-256-GCM. The key is
//!   `<app_data_dir>/secret.key`, outside the database, so the ciphertext
//!   that travels in a backup is useless on another machine. Anyone who can
//!   read this user's app data directory can still decrypt it.
//!
//! Keyring calls may block on D-Bus: call these functions from a blocking
//! thread, never from inside an async task.

use crate::clock;
use crate::db::repos::ssh_hosts::{self, SealedSecret};
use crate::db::repos::ssh_keychain;
use rusqlite::Connection;
use crate::db::Db;
use crate::error::{Error, Result};
use aes_gcm::aead::{Aead, Generate, Key, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "command-manager";
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretStore {
    None,
    Keyring,
    Sqlite,
}

impl SecretStore {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Keyring => "keyring",
            Self::Sqlite => "sqlite",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "keyring" => Self::Keyring,
            "sqlite" => Self::Sqlite,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKind {
    Password,
    Passphrase,
    /// The private key text pasted in MOD-17 (`key_source = 'content'`).
    PrivateKey,
}

impl SecretKind {
    pub const ALL: [SecretKind; 3] = [
        SecretKind::Password,
        SecretKind::Passphrase,
        SecretKind::PrivateKey,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Passphrase => "passphrase",
            Self::PrivateKey => "private_key",
        }
    }
}

/// One field of the secrets form: leave the saved value, replace it, or drop it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "action", content = "value", rename_all = "lowercase")]
pub enum SecretChange {
    #[default]
    Keep,
    Set(String),
    Clear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsInput {
    pub store: SecretStore,
    #[serde(default)]
    pub password: SecretChange,
    #[serde(default)]
    pub passphrase: SecretChange,
    #[serde(default)]
    pub private_key: SecretChange,
}

impl SecretsInput {
    pub fn change(&self, kind: SecretKind) -> &SecretChange {
        match kind {
            SecretKind::Password => &self.password,
            SecretKind::Passphrase => &self.passphrase,
            SecretKind::PrivateKey => &self.private_key,
        }
    }

    fn change_mut(&mut self, kind: SecretKind) -> &mut SecretChange {
        match kind {
            SecretKind::Password => &mut self.password,
            SecretKind::Passphrase => &mut self.passphrase,
            SecretKind::PrivateKey => &mut self.private_key,
        }
    }

    /// Keep everything except one kind.
    pub fn only(store: SecretStore, kind: SecretKind, change: SecretChange) -> Self {
        let mut input = Self {
            store,
            password: SecretChange::Keep,
            passphrase: SecretChange::Keep,
            private_key: SecretChange::Keep,
        };
        *input.change_mut(kind) = change;
        input
    }
}

/// Whose secret it is. Each owner has its own rows, keyring entries and
/// `secret_store` column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretOwner {
    Host(String),
    Key(String),
    Identity(String),
}

impl SecretOwner {
    pub fn id(&self) -> &str {
        match self {
            Self::Host(id) | Self::Key(id) | Self::Identity(id) => id,
        }
    }

    fn keyring_account(&self, kind: SecretKind) -> String {
        match self {
            Self::Host(id) => format!("ssh:{id}:{}", kind.as_str()),
            Self::Key(id) => format!("ssh-key:{id}:{}", kind.as_str()),
            Self::Identity(id) => format!("ssh-identity:{id}:{}", kind.as_str()),
        }
    }

    /// Binds the ciphertext to its row so it cannot be copied to another
    /// owner. Hosts keep the original format, so existing rows still open.
    fn aad(&self, kind: SecretKind) -> Vec<u8> {
        match self {
            Self::Host(id) => format!("{id}\0{}", kind.as_str()),
            Self::Key(id) => format!("key:{id}\0{}", kind.as_str()),
            Self::Identity(id) => format!("identity:{id}\0{}", kind.as_str()),
        }
        .into_bytes()
    }

    fn sealed_get(&self, c: &Connection, kind: SecretKind) -> Result<Option<SealedSecret>> {
        match self {
            Self::Host(id) => ssh_hosts::secret_get(c, id, kind.as_str()),
            Self::Key(id) => ssh_keychain::secret_get(c, "key", id, kind.as_str()),
            Self::Identity(id) => ssh_keychain::secret_get(c, "identity", id, kind.as_str()),
        }
    }

    fn sealed_put(&self, c: &Connection, kind: SecretKind, sealed: &SealedSecret) -> Result<()> {
        let at = clock::now_unix();
        match self {
            Self::Host(id) => ssh_hosts::secret_put(c, id, kind.as_str(), sealed, &at),
            Self::Key(id) => ssh_keychain::secret_put(c, "key", id, kind.as_str(), sealed, &at),
            Self::Identity(id) => {
                ssh_keychain::secret_put(c, "identity", id, kind.as_str(), sealed, &at)
            }
        }
    }

    fn sealed_delete(&self, c: &Connection, kind: SecretKind) -> Result<()> {
        match self {
            Self::Host(id) => ssh_hosts::secret_delete(c, id, kind.as_str()),
            Self::Key(id) => ssh_keychain::secret_delete(c, "key", id, kind.as_str()),
            Self::Identity(id) => ssh_keychain::secret_delete(c, "identity", id, kind.as_str()),
        }
    }

    /// Where this owner keeps secrets now; `None` when the row does not
    /// exist (an unsaved profile in "Kiểm tra kết nối").
    pub fn current_store(&self, c: &Connection) -> Result<Option<SecretStore>> {
        Ok(match self {
            Self::Host(id) => ssh_hosts::get(c, id)?.map(|host| host.secret_store),
            Self::Key(id) => ssh_keychain::get_key(c, id)?.map(|key| key.secret_store),
            Self::Identity(id) => {
                ssh_keychain::get_identity(c, id)?.map(|identity| identity.secret_store)
            }
        }
        .map(|store| SecretStore::parse(&store)))
    }

    fn set_store(&self, c: &Connection, store: SecretStore) -> Result<()> {
        match self {
            Self::Host(id) => ssh_hosts::set_secret_store(c, id, store.as_str()),
            Self::Identity(id) => ssh_keychain::set_identity_store(c, id, store.as_str()),
            // A key always has its key text; "none" only happens while the
            // row is being deleted.
            Self::Key(_) if store == SecretStore::None => Ok(()),
            Self::Key(id) => ssh_keychain::set_key_store(c, id, store.as_str()),
        }
    }
}

pub fn keyring_available() -> bool {
    keyring::Entry::store_status().is_ok()
}

pub struct Vault {
    key_path: PathBuf,
}

impl Vault {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            key_path: app_data_dir.join("secret.key"),
        }
    }

    pub fn get(
        &self,
        db: &Db,
        store: SecretStore,
        owner: &SecretOwner,
        kind: SecretKind,
    ) -> Result<Option<String>> {
        match store {
            SecretStore::None => Ok(None),
            SecretStore::Keyring => match keyring_entry(owner, kind)?.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(keyring_error(error)),
            },
            SecretStore::Sqlite => {
                let Some(sealed) = db.read(|c| owner.sealed_get(c, kind))?
                else {
                    return Ok(None);
                };
                // A missing key file (backup restored on another machine)
                // reads as "nothing saved", so the user is simply asked again.
                let Some(key) = self.load_key()? else {
                    return Ok(None);
                };
                Ok(open(&key, &owner.aad(kind), &sealed))
            }
        }
    }

    pub fn put(
        &self,
        db: &Db,
        store: SecretStore,
        owner: &SecretOwner,
        kind: SecretKind,
        value: &str,
    ) -> Result<()> {
        match store {
            SecretStore::None => Ok(()),
            SecretStore::Keyring => {
                // Windows Credential Manager caps a secret at 2560 bytes;
                // an RSA 4096 key does not fit.
                if cfg!(windows) && value.len() > 2500 {
                    return Err(Error::msg(
                        "Khóa quá lớn để lưu vào Windows Credential Manager, hãy chọn lưu trong SQLite",
                    ));
                }
                keyring_entry(owner, kind)?
                    .set_password(value)
                    .map_err(keyring_error)
            }
            SecretStore::Sqlite => {
                let key = self.load_or_create_key()?;
                let sealed = seal(&key, &owner.aad(kind), value)?;
                db.write(|c| owner.sealed_put(c, kind, &sealed))
            }
        }
    }

    pub fn delete(
        &self,
        db: &Db,
        store: SecretStore,
        owner: &SecretOwner,
        kind: SecretKind,
    ) -> Result<()> {
        match store {
            SecretStore::None => Ok(()),
            SecretStore::Keyring => match keyring_entry(owner, kind)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(error) => Err(keyring_error(error)),
            },
            SecretStore::Sqlite => {
                db.write(|c| owner.sealed_delete(c, kind))
            }
        }
    }

    /// Removes every secret of an owner from every store. Used on delete and
    /// "forget password"; a store that is unavailable is skipped.
    pub fn forget_all(&self, db: &Db, owner: &SecretOwner) -> Result<()> {
        for kind in SecretKind::ALL {
            if keyring_available() {
                let _ = self.delete(db, SecretStore::Keyring, owner, kind);
            }
            self.delete(db, SecretStore::Sqlite, owner, kind)?;
        }
        db.write(|c| owner.set_store(c, SecretStore::None))
    }

    /// Applies the secrets part of MOD-17 and returns the store now in use.
    ///
    /// Changing the store moves the kept values across, so the user does not
    /// type the password again. Writes to the new store happen before the
    /// old copies are removed, so a failure never loses a saved secret.
    pub fn apply(
        &self,
        db: &Db,
        owner: &SecretOwner,
        current: SecretStore,
        input: &SecretsInput,
    ) -> Result<SecretStore> {
        if input.store == SecretStore::Keyring && !keyring_available() {
            return Err(Error::msg("Không tìm thấy keyring trên máy này"));
        }
        if input.store == SecretStore::None {
            self.forget_all(db, owner)?;
            return Ok(SecretStore::None);
        }

        let mut next = Vec::new();
        for kind in SecretKind::ALL {
            let value = match input.change(kind) {
                SecretChange::Set(value) if !value.is_empty() => Some(value.clone()),
                SecretChange::Set(_) | SecretChange::Clear => None,
                SecretChange::Keep => self.get(db, current, owner, kind).unwrap_or(None),
            };
            next.push((kind, value));
        }

        for (kind, value) in &next {
            match value {
                Some(value) => self.put(db, input.store, owner, *kind, value)?,
                None => self.delete(db, input.store, owner, *kind)?,
            }
        }
        if current != input.store {
            for kind in SecretKind::ALL {
                let _ = self.delete(db, current, owner, kind);
            }
        }

        let store = if next.iter().any(|(_, value)| value.is_some()) {
            input.store
        } else {
            SecretStore::None
        };
        db.write(|c| owner.set_store(c, store))?;
        Ok(store)
    }

    /// Saves one secret after a successful login (MOD-19).
    pub fn remember(
        &self,
        db: &Db,
        owner: &SecretOwner,
        current: SecretStore,
        store: SecretStore,
        kind: SecretKind,
        value: &str,
    ) -> Result<()> {
        let input = if store == SecretStore::None {
            // Unticking "Lưu mật khẩu" forgets only this kind.
            if current == SecretStore::None {
                return Ok(());
            }
            SecretsInput::only(current, kind, SecretChange::Clear)
        } else {
            SecretsInput::only(store, kind, SecretChange::Set(value.to_string()))
        };
        self.apply(db, owner, current, &input).map(|_| ())
    }

    fn load_key(&self) -> Result<Option<[u8; KEY_LEN]>> {
        match std::fs::read(&self.key_path) {
            Ok(bytes) if bytes.len() == KEY_LEN => {
                let mut key = [0u8; KEY_LEN];
                key.copy_from_slice(&bytes);
                Ok(Some(key))
            }
            Ok(_) => Err(Error::msg(format!(
                "Tệp khóa {} bị hỏng",
                self.key_path.display()
            ))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn load_or_create_key(&self) -> Result<[u8; KEY_LEN]> {
        if let Some(key) = self.load_key()? {
            return Ok(key);
        }
        let key = Key::<Aes256Gcm>::generate();
        let mut bytes = [0u8; KEY_LEN];
        bytes.copy_from_slice(key.as_slice());
        write_private(&self.key_path, &bytes)?;
        Ok(bytes)
    }
}

fn keyring_entry(owner: &SecretOwner, kind: SecretKind) -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &owner.keyring_account(kind)).map_err(keyring_error)
}

fn keyring_error(error: keyring::Error) -> Error {
    Error::msg(format!("Keyring: {error}"))
}

fn seal(key: &[u8; KEY_LEN], aad: &[u8], value: &str) -> Result<SealedSecret> {
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let nonce = Nonce::generate();
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: value.as_bytes(),
                aad,
            },
        )
        .map_err(|_| Error::msg("Không mã hóa được mật khẩu"))?;
    Ok(SealedSecret {
        nonce: nonce.as_slice().to_vec(),
        ciphertext,
    })
}

fn open(key: &[u8; KEY_LEN], aad: &[u8], sealed: &SealedSecret) -> Option<String> {
    if sealed.nonce.len() != NONCE_LEN {
        return None;
    }
    let nonce = Nonce::try_from(sealed.nonce.as_slice()).ok()?;
    let cipher = Aes256Gcm::new(&Key::<Aes256Gcm>::from(*key));
    let plain = cipher
        .decrypt(
            &nonce,
            Payload {
                msg: &sealed.ciphertext,
                aad,
            },
        )
        .ok()?;
    String::from_utf8(plain).ok()
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        // The app data directory under %APPDATA% is already private to the
        // user, so the file inherits an ACL that only they can read.
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::ssh_hosts::{insert, SshHostInput};

    fn setup() -> (tempfile::TempDir, Db, Vault) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Db::open(dir.path().join("app.db")).expect("db");
        db.write(|c| {
            let input = SshHostInput {
                name: "h".into(),
                host: "h".into(),
                port: 22,
                username: "u".into(),
                auth_method: "auto".into(),
                key_source: "file".into(),
                ..Default::default()
            };
            insert(c, "h1", &input, "0")?;
            insert(c, "h2", &input, "0")
        })
        .expect("hosts");
        let vault = Vault::new(dir.path());
        (dir, db, vault)
    }

    fn host(id: &str) -> SecretOwner {
        SecretOwner::Host(id.into())
    }

    fn store_of(db: &Db, id: &str) -> String {
        db.read(|c| ssh_hosts::require(c, id))
            .expect("host")
            .secret_store
    }

    #[test]
    fn sqlite_round_trip_is_encrypted_and_bound_to_the_host() {
        let (dir, db, vault) = setup();
        vault
            .put(
                &db,
                SecretStore::Sqlite,
                &host("h1"),
                SecretKind::Password,
                "hunter2",
            )
            .expect("put");
        assert_eq!(
            vault
                .get(&db, SecretStore::Sqlite, &host("h1"), SecretKind::Password)
                .expect("get")
                .as_deref(),
            Some("hunter2")
        );

        let sealed = db
            .read(|c| ssh_hosts::secret_get(c, "h1", "password"))
            .expect("row")
            .expect("sealed");
        assert!(!sealed
            .ciphertext
            .windows(7)
            .any(|window| window == b"hunter2"));

        // The same ciphertext moved to another host must not decrypt.
        db.write(|c| ssh_hosts::secret_put(c, "h2", "password", &sealed, "0"))
            .expect("copy");
        assert_eq!(
            vault
                .get(&db, SecretStore::Sqlite, &host("h2"), SecretKind::Password)
                .expect("get"),
            None
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join("secret.key"))
                .expect("key file")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn a_lost_key_file_reads_as_nothing_saved() {
        let (dir, db, vault) = setup();
        vault
            .put(&db, SecretStore::Sqlite, &host("h1"), SecretKind::Password, "pw")
            .expect("put");
        std::fs::remove_file(dir.path().join("secret.key")).expect("remove key");
        assert_eq!(
            vault
                .get(&db, SecretStore::Sqlite, &host("h1"), SecretKind::Password)
                .expect("get"),
            None
        );
    }

    #[test]
    fn apply_keeps_set_and_clears_values() {
        let (_dir, db, vault) = setup();
        let store = vault
            .apply(
                &db,
                &host("h1"),
                SecretStore::None,
                &SecretsInput {
                    store: SecretStore::Sqlite,
                    password: SecretChange::Set("pw".into()),
                    passphrase: SecretChange::Keep,
                    private_key: SecretChange::Keep,
                },
            )
            .expect("apply");
        assert_eq!(store, SecretStore::Sqlite);
        assert_eq!(store_of(&db, "h1"), "sqlite");

        // Keep leaves the password alone while a passphrase is added.
        vault
            .apply(
                &db,
                &host("h1"),
                SecretStore::Sqlite,
                &SecretsInput {
                    store: SecretStore::Sqlite,
                    password: SecretChange::Keep,
                    passphrase: SecretChange::Set("pp".into()),
                    private_key: SecretChange::Keep,
                },
            )
            .expect("apply");
        let get = |kind| {
            vault
                .get(&db, SecretStore::Sqlite, &host("h1"), kind)
                .expect("get")
        };
        assert_eq!(get(SecretKind::Password).as_deref(), Some("pw"));
        assert_eq!(get(SecretKind::Passphrase).as_deref(), Some("pp"));

        // Clearing both falls back to "none".
        let store = vault
            .apply(
                &db,
                &host("h1"),
                SecretStore::Sqlite,
                &SecretsInput {
                    store: SecretStore::Sqlite,
                    password: SecretChange::Clear,
                    passphrase: SecretChange::Clear,
                    private_key: SecretChange::Clear,
                },
            )
            .expect("apply");
        assert_eq!(store, SecretStore::None);
        assert_eq!(store_of(&db, "h1"), "none");
        assert_eq!(get(SecretKind::Password), None);
    }

    #[test]
    fn remember_without_saving_forgets_only_that_kind() {
        let (_dir, db, vault) = setup();
        vault
            .apply(
                &db,
                &host("h1"),
                SecretStore::None,
                &SecretsInput {
                    store: SecretStore::Sqlite,
                    password: SecretChange::Set("pw".into()),
                    passphrase: SecretChange::Set("pp".into()),
                    private_key: SecretChange::Keep,
                },
            )
            .expect("apply");
        vault
            .remember(
                &db,
                &host("h1"),
                SecretStore::Sqlite,
                SecretStore::None,
                SecretKind::Password,
                "new",
            )
            .expect("remember");
        let get = |kind| {
            vault
                .get(&db, SecretStore::Sqlite, &host("h1"), kind)
                .expect("get")
        };
        assert_eq!(get(SecretKind::Password), None);
        assert_eq!(get(SecretKind::Passphrase).as_deref(), Some("pp"));
        assert_eq!(store_of(&db, "h1"), "sqlite");
    }

    #[test]
    fn private_key_is_a_third_kind_alongside_password() {
        let (_dir, db, vault) = setup();
        let input: SecretsInput = serde_json::from_str(
            r#"{"store":"sqlite","password":{"action":"set","value":"pw"},"private_key":{"action":"set","value":"KEY"}}"#,
        )
        .expect("json");
        vault
            .apply(&db, &host("h1"), SecretStore::None, &input)
            .expect("apply");
        let get = |kind| {
            vault
                .get(&db, SecretStore::Sqlite, &host("h1"), kind)
                .expect("get")
        };
        assert_eq!(get(SecretKind::PrivateKey).as_deref(), Some("KEY"));
        assert_eq!(get(SecretKind::Password).as_deref(), Some("pw"));

        vault
            .apply(
                &db,
                &host("h1"),
                SecretStore::Sqlite,
                &SecretsInput::only(
                    SecretStore::Sqlite,
                    SecretKind::PrivateKey,
                    SecretChange::Clear,
                ),
            )
            .expect("clear key");
        assert_eq!(get(SecretKind::PrivateKey), None);
        assert_eq!(get(SecretKind::Password).as_deref(), Some("pw"));
    }

    #[test]
    fn keychain_owners_are_separate_from_hosts() {
        let (_dir, db, vault) = setup();
        db.write(|c| {
            c.execute(
                "INSERT INTO ssh_key (id, name, algorithm, public_key, fingerprint, secret_store, created_at)
                 VALUES ('h1', 'k', 'ssh-ed25519', 'pk', 'fp', 'sqlite', '0')",
                [],
            )?;
            Ok(())
        })
        .expect("key row");
        let key = SecretOwner::Key("h1".into());
        vault
            .put(&db, SecretStore::Sqlite, &key, SecretKind::PrivateKey, "KEY")
            .expect("put");
        // Same id, different owner: nothing leaks across.
        assert_eq!(
            vault
                .get(&db, SecretStore::Sqlite, &host("h1"), SecretKind::PrivateKey)
                .expect("get"),
            None
        );
        assert_eq!(
            vault
                .get(&db, SecretStore::Sqlite, &key, SecretKind::PrivateKey)
                .expect("get")
                .as_deref(),
            Some("KEY")
        );
        // The host's sealed row cannot be read as the key's.
        vault
            .put(&db, SecretStore::Sqlite, &host("h1"), SecretKind::Password, "pw")
            .expect("put");
        let sealed = db
            .read(|c| ssh_hosts::secret_get(c, "h1", "password"))
            .expect("row")
            .expect("sealed");
        db.write(|c| ssh_keychain::secret_put(c, "key", "h1", "password", &sealed, "0"))
            .expect("copy");
        assert_eq!(
            vault
                .get(&db, SecretStore::Sqlite, &key, SecretKind::Password)
                .expect("get"),
            None
        );
    }

    #[test]
    fn secret_change_json_shape() {
        let input: SecretsInput = serde_json::from_str(
            r#"{"store":"sqlite","password":{"action":"set","value":"x"},"passphrase":{"action":"clear"}}"#,
        )
        .expect("json");
        assert_eq!(input.store, SecretStore::Sqlite);
        assert!(matches!(input.password, SecretChange::Set(ref v) if v == "x"));
        assert!(matches!(input.passphrase, SecretChange::Clear));
        let keep: SecretsInput = serde_json::from_str(r#"{"store":"none"}"#).expect("json");
        assert!(matches!(keep.password, SecretChange::Keep));
    }
}
