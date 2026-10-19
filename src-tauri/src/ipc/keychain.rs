//! Keychain IPC (Termius style): keys and identities saved once and picked
//! by many hosts. Secret values go straight to the vault and never come back
//! to the frontend; only public facts (public key, fingerprint) do.

use super::ssh::{blocking, SshState};
use super::AppState;
use crate::clock;
use crate::db::repos::ssh_keychain::{self, SshIdentity, SshIdentityInput, SshKey};
use crate::error::{Error, Result};
use crate::ssh::keys::{self, ParsedKey};
use crate::ssh::secret::{SecretChange, SecretOwner, SecretStore, SecretsInput};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[derive(Serialize)]
pub struct KeyView {
    #[serde(flatten)]
    key: SshKey,
    used_by_hosts: usize,
    used_by_identities: usize,
}

#[derive(Serialize)]
pub struct IdentityView {
    #[serde(flatten)]
    identity: SshIdentity,
    used_by_hosts: usize,
}

#[derive(Serialize)]
pub struct KeychainList {
    keys: Vec<KeyView>,
    identities: Vec<IdentityView>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn ssh_keychain_list(app: AppHandle) -> Result<KeychainList> {
    app.state::<AppState>().db.read(|c| {
        let key_usage = ssh_keychain::key_usage(c)?;
        let identity_usage = ssh_keychain::identity_usage(c)?;
        let keys = ssh_keychain::list_keys(c)?
            .into_iter()
            .map(|key| {
                let usage = key_usage.get(&key.id).copied().unwrap_or_default();
                KeyView {
                    key,
                    used_by_hosts: usage.hosts,
                    used_by_identities: usage.identities,
                }
            })
            .collect();
        let identities = ssh_keychain::list_identities(c)?
            .into_iter()
            .map(|identity| IdentityView {
                used_by_hosts: identity_usage.get(&identity.id).copied().unwrap_or(0),
                identity,
            })
            .collect();
        Ok(KeychainList { keys, identities })
    })
}

fn key_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::msg("Hãy đặt tên cho khóa"));
    }
    if name.chars().count() > 100 {
        return Err(Error::msg("Tên khóa dài quá 100 ký tự"));
    }
    Ok(name.to_string())
}

fn key_store(store: SecretStore) -> Result<SecretStore> {
    if store == SecretStore::None {
        return Err(Error::msg(
            "Khóa trong Keychain phải được lưu (Keyring hoặc SQLite)",
        ));
    }
    Ok(store)
}

/// Inserts the row, then hands the key text (and passphrase) to the vault.
/// A vault failure removes the row again, so nothing is half-saved.
fn save_key(
    app: &AppHandle,
    name: String,
    parsed: ParsedKey,
    passphrase: Option<String>,
    store: SecretStore,
) -> Result<SshKey> {
    let state = app.state::<AppState>();
    let key = SshKey {
        id: Uuid::new_v4().to_string(),
        name,
        algorithm: parsed.algorithm,
        public_key: parsed.public_key,
        fingerprint: parsed.fingerprint,
        encrypted: parsed.encrypted,
        secret_store: store.as_str().to_string(),
        created_at: clock::now_unix(),
    };
    state.db.write(|c| ssh_keychain::insert_key(c, &key))?;

    let owner = SecretOwner::Key(key.id.clone());
    let secrets = SecretsInput {
        store,
        password: SecretChange::Keep,
        // A passphrase only matters for key text that is still encrypted.
        passphrase: match passphrase.filter(|value| !value.is_empty()) {
            Some(value) if parsed.encrypted => SecretChange::Set(value),
            _ => SecretChange::Keep,
        },
        private_key: SecretChange::Set(parsed.text),
    };
    let vault = &app.state::<SshState>().vault;
    if let Err(error) = vault.apply(&state.db, &owner, SecretStore::None, &secrets) {
        let _ = vault.forget_all(&state.db, &owner);
        let _ = state.db.write(|c| ssh_keychain::delete_key(c, &key.id));
        return Err(error);
    }
    Ok(key)
}

/// "Thêm khóa → Dán": OpenSSH, PEM or PuTTY .ppk text.
#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_key_import(
    app: AppHandle,
    name: String,
    private_key: String,
    passphrase: Option<String>,
    store: SecretStore,
) -> Result<SshKey> {
    blocking(move || {
        let name = key_name(&name)?;
        let store = key_store(store)?;
        let parsed = keys::import(&private_key, passphrase.as_deref(), &name)?;
        save_key(&app, name, parsed, passphrase, store)
    })
    .await
}

/// "Thêm khóa → Sinh khóa mới": ed25519. The public key comes back in the
/// result so the UI can offer to copy it into `authorized_keys`.
#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_key_generate(app: AppHandle, name: String, store: SecretStore) -> Result<SshKey> {
    blocking(move || {
        let name = key_name(&name)?;
        let store = key_store(store)?;
        let parsed = keys::generate_ed25519(&name)?;
        save_key(&app, name, parsed, None, store)
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub fn ssh_key_rename(app: AppHandle, id: String, name: String) -> Result<SshKey> {
    let name = key_name(&name)?;
    app.state::<AppState>().db.write(|c| {
        ssh_keychain::rename_key(c, &id, &name)?;
        ssh_keychain::require_key(c, &id)
    })
}

/// Hosts and identities using the key fall back to their other settings
/// (`ON DELETE SET NULL`); the UI warns with the usage counts first.
#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_key_delete(app: AppHandle, id: String, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(Error::ConfirmationRequired);
    }
    blocking(move || {
        let state = app.state::<AppState>();
        state.db.read(|c| ssh_keychain::require_key(c, &id))?;
        app.state::<SshState>()
            .vault
            .forget_all(&state.db, &SecretOwner::Key(id.clone()))?;
        state.db.write(|c| ssh_keychain::delete_key(c, &id))
    })
    .await
}

fn apply_identity_password(
    app: &AppHandle,
    id: &str,
    current: SecretStore,
    store: SecretStore,
    password: SecretChange,
) -> Result<()> {
    let secrets = SecretsInput {
        store,
        password,
        passphrase: SecretChange::Clear,
        private_key: SecretChange::Clear,
    };
    app.state::<SshState>().vault.apply(
        &app.state::<AppState>().db,
        &SecretOwner::Identity(id.to_string()),
        current,
        &secrets,
    )?;
    Ok(())
}

/// `store = none` keeps no password: the login dialog asks each time.
#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_identity_create(
    app: AppHandle,
    identity: SshIdentityInput,
    password: Option<SecretChange>,
    store: SecretStore,
) -> Result<SshIdentity> {
    blocking(move || {
        let state = app.state::<AppState>();
        let id = Uuid::new_v4().to_string();
        let at = clock::now_unix();
        state.db.write(|c| {
            let input = ssh_keychain::normalize_identity(c, identity)?;
            ssh_keychain::insert_identity(c, &id, &input, &at)
        })?;
        let password = password.unwrap_or(SecretChange::Keep);
        if store != SecretStore::None && matches!(password, SecretChange::Set(_)) {
            if let Err(error) =
                apply_identity_password(&app, &id, SecretStore::None, store, password)
            {
                let _ = state.db.write(|c| ssh_keychain::delete_identity(c, &id));
                return Err(error);
            }
        }
        state.db.read(|c| ssh_keychain::require_identity(c, &id))
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_identity_update(
    app: AppHandle,
    id: String,
    identity: SshIdentityInput,
    password: Option<SecretChange>,
    store: SecretStore,
) -> Result<SshIdentity> {
    blocking(move || {
        let state = app.state::<AppState>();
        let current = state.db.write(|c| {
            let current = ssh_keychain::require_identity(c, &id)?;
            let input = ssh_keychain::normalize_identity(c, identity)?;
            ssh_keychain::update_identity(c, &id, &input)?;
            Ok(current)
        })?;
        apply_identity_password(
            &app,
            &id,
            SecretStore::parse(&current.secret_store),
            store,
            password.unwrap_or(SecretChange::Keep),
        )?;
        state.db.read(|c| ssh_keychain::require_identity(c, &id))
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_identity_delete(app: AppHandle, id: String, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(Error::ConfirmationRequired);
    }
    blocking(move || {
        let state = app.state::<AppState>();
        state.db.read(|c| ssh_keychain::require_identity(c, &id))?;
        app.state::<SshState>()
            .vault
            .forget_all(&state.db, &SecretOwner::Identity(id.clone()))?;
        state.db.write(|c| ssh_keychain::delete_identity(c, &id))
    })
    .await
}
