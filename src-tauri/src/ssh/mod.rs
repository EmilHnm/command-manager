//! SSH terminals (SCR-07): connections, authentication, host keys and saved
//! secrets. The Tauri glue lives in `ipc::ssh`; this module only talks to the
//! UI through [`SshEnv`], so it can be tested without a window.

pub mod client;
pub mod config;
pub mod keys;
pub mod known_hosts;
pub mod manager;
pub mod secret;
pub mod terminal;
#[cfg(test)]
mod tests;

use crate::db::repos::ssh_hosts::SshHost;
use crate::db::repos::ssh_keychain;
use crate::error::{Error, Result};
use rusqlite::Connection;
use secret::{SecretKind, SecretOwner, SecretStore};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnState {
    Connecting,
    Verifying,
    Authenticating,
    Ready,
    /// The network or the server dropped the connection.
    Lost,
    /// Closed on purpose (idle, deleted, app exit).
    Closed,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangedKey {
    pub known_hosts_path: String,
    pub line: usize,
    pub old_fingerprint: Option<String>,
}

/// MOD-18. `changed` set means the connection is already refused and the
/// dialog only explains why.
#[derive(Debug, Clone, Serialize)]
pub struct HostKeyPrompt {
    pub host_id: String,
    pub label: String,
    pub algorithm: String,
    pub fingerprint: String,
    pub changed: Option<ChangedKey>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct HostKeyDecision {
    pub trust: bool,
    #[serde(default)]
    pub remember: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthPromptKind {
    Password,
    Passphrase,
    KeyboardInteractive,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptField {
    pub text: String,
    pub echo: bool,
}

/// MOD-19.
#[derive(Debug, Clone, Serialize)]
pub struct AuthPrompt {
    pub host_id: String,
    pub label: String,
    pub kind: AuthPromptKind,
    pub name: String,
    pub instructions: String,
    pub prompts: Vec<PromptField>,
    /// Whether "Lưu mật khẩu" applies (never for OTP prompts).
    pub storable: bool,
    /// Where this host keeps secrets now; the dialog preselects it.
    pub store: SecretStore,
    pub error: Option<String>,
    pub attempt: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthReply {
    pub answers: Vec<String>,
    #[serde(default = "no_store")]
    pub store: SecretStore,
}

fn no_store() -> SecretStore {
    SecretStore::None
}

/// Where one secret of a login is read from and saved to.
#[derive(Debug, Clone, PartialEq)]
pub struct SecretSlot {
    pub owner: SecretOwner,
    pub store: SecretStore,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeyChoice {
    /// A Keychain key: its text and passphrase are the key's secrets.
    Keychain {
        id: String,
        name: String,
        slot: SecretSlot,
    },
    /// Key text saved on the host itself (`key_source = 'content'`).
    HostContent,
    /// `key_path`.
    File(String),
    /// ~/.ssh/id_ed25519, id_ecdsa, id_rsa.
    Defaults,
}

/// A host with its credentials resolved: identity first, then the host's
/// own fields, the way Termius layers them.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub host: SshHost,
    pub username: String,
    pub key: KeyChoice,
    pub password: SecretSlot,
    pub passphrase: SecretSlot,
}

impl Target {
    /// The slot holding key text for `HostContent`.
    pub fn host_slot(&self) -> SecretSlot {
        SecretSlot {
            owner: SecretOwner::Host(self.host.id.clone()),
            store: SecretStore::parse(&self.host.secret_store),
        }
    }
}

/// Builds the [`Target`] for a host:
/// * username and password come from the identity when there is one;
/// * the key is the host's Keychain key, else the identity's, else the
///   host's own key text / file / ~/.ssh defaults;
/// * a Keychain key keeps its own passphrase.
pub fn resolve(conn: &Connection, host: SshHost) -> Result<Target> {
    let host_slot = SecretSlot {
        owner: SecretOwner::Host(host.id.clone()),
        store: SecretStore::parse(&host.secret_store),
    };
    let identity = match host.identity_id.as_deref() {
        Some(id) => Some(ssh_keychain::require_identity(conn, id)?),
        None => None,
    };
    let username = identity
        .as_ref()
        .map(|identity| identity.username.clone())
        .unwrap_or_else(|| host.username.clone());
    if username.trim().is_empty() {
        return Err(Error::msg("Máy chủ chưa có tên đăng nhập"));
    }
    let password = match &identity {
        Some(identity) => SecretSlot {
            owner: SecretOwner::Identity(identity.id.clone()),
            store: SecretStore::parse(&identity.secret_store),
        },
        None => host_slot.clone(),
    };

    let key_id = host.key_id.clone().or_else(|| {
        identity
            .as_ref()
            .and_then(|identity| identity.key_id.clone())
    });
    let key = match key_id {
        Some(id) => {
            let key = ssh_keychain::require_key(conn, &id)?;
            KeyChoice::Keychain {
                slot: SecretSlot {
                    owner: SecretOwner::Key(key.id.clone()),
                    store: SecretStore::parse(&key.secret_store),
                },
                id: key.id,
                name: key.name,
            }
        }
        None if host.key_source == "content" => KeyChoice::HostContent,
        None => match &host.key_path {
            Some(path) => KeyChoice::File(path.clone()),
            None => KeyChoice::Defaults,
        },
    };
    let passphrase = match &key {
        KeyChoice::Keychain { slot, .. } => slot.clone(),
        _ => host_slot,
    };
    Ok(Target {
        host,
        username,
        key,
        password,
        passphrase,
    })
}

/// Everything the SSH code needs from the app: data, secrets and dialogs.
pub trait SshEnv: Send + Sync + 'static {
    fn target(&self, host_id: &str) -> Result<Target>;
    fn known_hosts(&self) -> &known_hosts::KnownHosts;
    fn load_secret(
        &self,
        slot: &SecretSlot,
        kind: SecretKind,
    ) -> BoxFuture<'static, Option<String>>;
    /// Called after a successful login with what the user typed in MOD-19.
    /// `SecretStore::None` means "do not keep it" and forgets that kind.
    fn save_secret(
        &self,
        owner: &SecretOwner,
        store: SecretStore,
        kind: SecretKind,
        value: String,
    ) -> BoxFuture<'static, ()>;
    fn host_key_prompt(&self, prompt: HostKeyPrompt) -> BoxFuture<'static, HostKeyDecision>;
    /// `None` = the user cancelled.
    fn auth_prompt(&self, prompt: AuthPrompt) -> BoxFuture<'static, Option<AuthReply>>;
    fn status(&self, host_id: &str, state: ConnState, error: Option<String>);
    fn connected(&self, host_id: &str);
    /// Runs a background task (idle disconnects).
    fn spawn(&self, task: BoxFuture<'static, ()>);
}

pub fn label(target: &Target) -> String {
    let host = &target.host;
    if host.port == 22 {
        format!("{}@{}", target.username, host.host)
    } else {
        format!("{}@{}:{}", target.username, host.host, host.port)
    }
}
