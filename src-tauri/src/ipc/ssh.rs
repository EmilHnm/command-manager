//! Tauri side of SSH (SCR-07, MOD-17/18/19): host CRUD, the host key and
//! login dialogs (sent as events, answered by `ssh_*_respond`), and SSH
//! terminals registered in the same process manager as local ones.

use super::commands::{buffer_bytes, StatusEvent, TerminalInfo};
use super::{events, AppState};
use crate::clock;
use crate::db::repos::ssh_hosts::{self, SshHost, SshHostInput};
use crate::error::{Error, Result};
use crate::process::manager::Live;
use crate::process::spawn;
use crate::ssh::client::{self, Stage};
use crate::ssh::config::{self, ConfigHost};
use crate::ssh::known_hosts::{home_dir, KnownHosts};
use crate::ssh::manager::{Lease, SshManager};
use crate::ssh::secret::{
    self, SecretChange, SecretKind, SecretOwner, SecretStore, SecretsInput, Vault,
};
use crate::ssh::terminal::{SshTerminal, TerminalEnd};
use crate::ssh::{
    resolve, AuthPrompt, AuthReply, BoxFuture, ConnState, HostKeyDecision, HostKeyPrompt,
    KeyChoice, SecretSlot, SshEnv, Target,
};
use dashmap::DashMap;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;
use uuid::Uuid;

/// A dialog nobody answers counts as cancelled after this long.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(300);
const TEST_PREFIX: &str = "test:";

#[derive(Default)]
struct Pending {
    host_keys: DashMap<String, oneshot::Sender<HostKeyDecision>>,
    auth: DashMap<String, oneshot::Sender<Option<AuthReply>>>,
}

pub struct SshState {
    pub(crate) manager: SshManager,
    pub(crate) vault: Arc<Vault>,
    known: Arc<KnownHosts>,
    pending: Arc<Pending>,
    terminals: DashMap<String, Arc<SshTerminal>>,
}

impl SshState {
    pub fn new(app: &AppHandle, data_dir: &Path) -> Self {
        let vault = Arc::new(Vault::new(data_dir));
        let known = Arc::new(KnownHosts::new(data_dir));
        let pending = Arc::new(Pending::default());
        let env = Arc::new(TauriEnv {
            app: app.clone(),
            known: Arc::clone(&known),
            vault: Arc::clone(&vault),
            pending: Arc::clone(&pending),
        });
        Self {
            manager: SshManager::new(env),
            vault,
            known,
            pending,
            terminals: DashMap::new(),
        }
    }
}

struct TauriEnv {
    app: AppHandle,
    known: Arc<KnownHosts>,
    vault: Arc<Vault>,
    pending: Arc<Pending>,
}

impl SshEnv for TauriEnv {
    fn target(&self, host_id: &str) -> Result<Target> {
        self.app.state::<AppState>().db.read(|c| {
            let host = ssh_hosts::require(c, host_id)?;
            resolve(c, host)
        })
    }

    fn known_hosts(&self) -> &KnownHosts {
        &self.known
    }

    fn load_secret(
        &self,
        slot: &SecretSlot,
        kind: SecretKind,
    ) -> BoxFuture<'static, Option<String>> {
        let app = self.app.clone();
        let vault = Arc::clone(&self.vault);
        let slot = slot.clone();
        Box::pin(async move {
            tauri::async_runtime::spawn_blocking(move || {
                let db = &app.state::<AppState>().db;
                vault
                    .get(db, slot.store, &slot.owner, kind)
                    .unwrap_or_else(|error| {
                        eprintln!("Command Manager: không đọc được mật khẩu đã lưu: {error}");
                        None
                    })
            })
            .await
            .ok()
            .flatten()
        })
    }

    fn save_secret(
        &self,
        owner: &SecretOwner,
        store: SecretStore,
        kind: SecretKind,
        value: String,
    ) -> BoxFuture<'static, ()> {
        let app = self.app.clone();
        let vault = Arc::clone(&self.vault);
        let owner = owner.clone();
        Box::pin(async move {
            let _ = tauri::async_runtime::spawn_blocking(move || {
                let db = &app.state::<AppState>().db;
                // An unsaved profile (connection test) has nowhere to keep it.
                let Ok(Some(current)) = db.read(|c| owner.current_store(c)) else {
                    return;
                };
                if let Err(error) = vault.remember(db, &owner, current, store, kind, &value) {
                    eprintln!("Command Manager: không lưu được mật khẩu: {error}");
                }
            })
            .await;
        })
    }

    fn host_key_prompt(&self, prompt: HostKeyPrompt) -> BoxFuture<'static, HostKeyDecision> {
        let app = self.app.clone();
        let pending = Arc::clone(&self.pending);
        Box::pin(async move {
            let request_id = Uuid::new_v4().to_string();
            if prompt.changed.is_some() {
                // Already refused; the dialog only explains why.
                let _ = app.emit(
                    events::SSH_HOSTKEY_REQUEST,
                    serde_json::json!({ "request_id": request_id, "prompt": prompt }),
                );
                return HostKeyDecision::default();
            }
            let (tx, rx) = oneshot::channel();
            pending.host_keys.insert(request_id.clone(), tx);
            let _ = app.emit(
                events::SSH_HOSTKEY_REQUEST,
                serde_json::json!({ "request_id": request_id, "prompt": prompt }),
            );
            let decision = tokio::time::timeout(PROMPT_TIMEOUT, rx)
                .await
                .ok()
                .and_then(|answer| answer.ok())
                .unwrap_or_default();
            pending.host_keys.remove(&request_id);
            decision
        })
    }

    fn auth_prompt(&self, prompt: AuthPrompt) -> BoxFuture<'static, Option<AuthReply>> {
        let app = self.app.clone();
        let pending = Arc::clone(&self.pending);
        Box::pin(async move {
            let request_id = Uuid::new_v4().to_string();
            let (tx, rx) = oneshot::channel();
            pending.auth.insert(request_id.clone(), tx);
            let _ = app.emit(
                events::SSH_AUTH_REQUEST,
                serde_json::json!({ "request_id": request_id, "prompt": prompt }),
            );
            let reply = tokio::time::timeout(PROMPT_TIMEOUT, rx)
                .await
                .ok()
                .and_then(|answer| answer.ok())
                .flatten();
            pending.auth.remove(&request_id);
            reply
        })
    }

    fn status(&self, host_id: &str, state: ConnState, error: Option<String>) {
        if host_id.starts_with(TEST_PREFIX) {
            return;
        }
        let _ = self.app.emit(
            events::SSH_STATUS,
            serde_json::json!({ "host_id": host_id, "state": state, "error": error }),
        );
    }

    fn connected(&self, host_id: &str) {
        if host_id.starts_with(TEST_PREFIX) {
            return;
        }
        let at = clock::now_unix();
        let _ = self
            .app
            .state::<AppState>()
            .db
            .write(|c| ssh_hosts::touch_connected(c, host_id, &at));
    }

    fn spawn(&self, task: BoxFuture<'static, ()>) {
        tauri::async_runtime::spawn(task);
    }
}

/// The env used by "Kiểm tra kết nối": values typed in MOD-17 but not saved
/// yet, and no events or writes that would look like a real connection.
struct TestEnv {
    base: Arc<dyn SshEnv>,
    password: Option<String>,
    passphrase: Option<String>,
    private_key: Option<String>,
}

impl SshEnv for TestEnv {
    fn target(&self, host_id: &str) -> Result<Target> {
        self.base.target(host_id)
    }

    fn known_hosts(&self) -> &KnownHosts {
        self.base.known_hosts()
    }

    fn load_secret(
        &self,
        slot: &SecretSlot,
        kind: SecretKind,
    ) -> BoxFuture<'static, Option<String>> {
        // What the form holds wins; a Keychain key's own text is never
        // replaced by the host form's pasted key.
        let typed = match (kind, &slot.owner) {
            (SecretKind::Password, _) => self.password.clone(),
            (SecretKind::Passphrase, _) => self.passphrase.clone(),
            (SecretKind::PrivateKey, SecretOwner::Host(_)) => self.private_key.clone(),
            (SecretKind::PrivateKey, _) => None,
        };
        match typed {
            Some(value) => Box::pin(async move { Some(value) }),
            None if slot.owner.id().starts_with(TEST_PREFIX) => Box::pin(async { None }),
            None => self.base.load_secret(slot, kind),
        }
    }

    fn save_secret(
        &self,
        _: &SecretOwner,
        _: SecretStore,
        _: SecretKind,
        _: String,
    ) -> BoxFuture<'static, ()> {
        Box::pin(async {})
    }

    fn host_key_prompt(&self, prompt: HostKeyPrompt) -> BoxFuture<'static, HostKeyDecision> {
        self.base.host_key_prompt(prompt)
    }

    fn auth_prompt(&self, prompt: AuthPrompt) -> BoxFuture<'static, Option<AuthReply>> {
        self.base.auth_prompt(prompt)
    }

    fn status(&self, _: &str, _: ConnState, _: Option<String>) {}

    fn connected(&self, _: &str) {}

    fn spawn(&self, task: BoxFuture<'static, ()>) {
        self.base.spawn(task)
    }
}

pub(crate) async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| Error::msg(format!("ssh worker failed: {error}")))?
}

#[derive(Serialize)]
pub struct SshHostView {
    #[serde(flatten)]
    host: SshHost,
    open_channels: usize,
}

#[tauri::command(rename_all = "snake_case")]
pub fn ssh_hosts_list(app: AppHandle) -> Result<Vec<SshHostView>> {
    let hosts = app.state::<AppState>().db.read(ssh_hosts::list)?;
    let open = app.state::<SshState>().manager.open_channels();
    Ok(hosts
        .into_iter()
        .map(|host| SshHostView {
            open_channels: open.get(&host.id).copied().unwrap_or(0),
            host,
        })
        .collect())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_hosts_create(
    app: AppHandle,
    host: SshHostInput,
    secrets: Option<SecretsInput>,
) -> Result<SshHost> {
    blocking(move || {
        let state = app.state::<AppState>();
        let id = Uuid::new_v4().to_string();
        let at = clock::now_unix();
        let input = state.db.read(|c| ssh_hosts::normalize(c, None, host))?;
        check_key_content(&input, secrets.as_ref(), false)?;
        state.db.write(|c| ssh_hosts::insert(c, &id, &input, &at))?;
        if let Some(secrets) = secrets {
            let vault = &app.state::<SshState>().vault;
            if let Err(error) = vault.apply(
                &state.db,
                &SecretOwner::Host(id.clone()),
                SecretStore::None,
                &secrets,
            ) {
                // Leave nothing half-created; the form keeps its values.
                let _ = vault.forget_all(&state.db, &SecretOwner::Host(id.clone()));
                let _ = state.db.write(|c| ssh_hosts::delete(c, &id));
                return Err(error);
            }
        }
        state.db.read(|c| ssh_hosts::require(c, &id))
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_hosts_update(
    app: AppHandle,
    id: String,
    host: SshHostInput,
    secrets: Option<SecretsInput>,
) -> Result<SshHost> {
    blocking(move || {
        let state = app.state::<AppState>();
        let vault = &app.state::<SshState>().vault;
        let (current, input) = state.db.read(|c| {
            let current = ssh_hosts::require(c, &id)?;
            let input = ssh_hosts::normalize(c, Some(&id), host)?;
            Ok((current, input))
        })?;
        let current_store = SecretStore::parse(&current.secret_store);
        let has_saved_key = vault
            .get(
                &state.db,
                current_store,
                &SecretOwner::Host(id.clone()),
                SecretKind::PrivateKey,
            )
            .ok()
            .flatten()
            .is_some();
        check_key_content(&input, secrets.as_ref(), has_saved_key)?;

        // A key file host keeps no pasted key around.
        let mut secrets = secrets;
        if input.key_source != "content" {
            match secrets.as_mut() {
                Some(secrets) => secrets.private_key = SecretChange::Clear,
                None if has_saved_key => {
                    secrets = Some(SecretsInput::only(
                        current_store,
                        SecretKind::PrivateKey,
                        SecretChange::Clear,
                    ));
                }
                None => {}
            }
        }

        state.db.write(|c| ssh_hosts::update(c, &id, &input))?;
        if let Some(secrets) = secrets {
            vault.apply(
                &state.db,
                &SecretOwner::Host(id.clone()),
                current_store,
                &secrets,
            )?;
        }
        state.db.read(|c| ssh_hosts::require(c, &id))
    })
    .await
}

/// A pasted key exists only in the vault, so it must be saved somewhere,
/// and it has to parse before it is.
fn check_key_content(
    input: &SshHostInput,
    secrets: Option<&SecretsInput>,
    has_saved_key: bool,
) -> Result<()> {
    if let Some(SecretChange::Set(text)) = secrets.map(|secrets| &secrets.private_key) {
        if !text.trim().is_empty() {
            client::check_private_key(text)?;
        }
    }
    let uses_content = input.key_source == "content"
        && input.key_id.is_none()
        && matches!(input.auth_method.as_str(), "auto" | "key");
    if !uses_content {
        return Ok(());
    }
    let will_have_key = match secrets {
        None => has_saved_key,
        Some(secrets) if secrets.store == SecretStore::None => false,
        Some(secrets) => match &secrets.private_key {
            SecretChange::Set(text) => !text.trim().is_empty(),
            SecretChange::Keep => has_saved_key,
            SecretChange::Clear => false,
        },
    };
    if will_have_key {
        Ok(())
    } else {
        Err(Error::msg(
            "Hãy dán nội dung khóa riêng và chọn nơi lưu (Keyring hoặc SQLite)",
        ))
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_hosts_delete(app: AppHandle, id: String, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(Error::ConfirmationRequired);
    }
    let ssh = app.state::<SshState>();
    let state = app.state::<AppState>();
    let open: Vec<Arc<Live>> = ssh
        .terminals
        .iter()
        .filter(|entry| entry.value().host_id() == id)
        .filter_map(|entry| state.processes.get(entry.key()))
        .collect();
    let stop_app = app.clone();
    blocking(move || {
        for live in open {
            let _ = stop_terminal(&stop_app, &live, true);
        }
        Ok(())
    })
    .await?;
    ssh.manager.disconnect(&id).await;
    blocking(move || {
        let state = app.state::<AppState>();
        app.state::<SshState>()
            .vault
            .forget_all(&state.db, &SecretOwner::Host(id.clone()))?;
        state.db.write(|c| ssh_hosts::delete(c, &id))
    })
    .await
}

#[derive(Serialize)]
pub struct ConfigScan {
    path: Option<String>,
    hosts: Vec<ConfigHost>,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_config_scan() -> Result<ConfigScan> {
    blocking(|| {
        let Some(path) = home_dir().map(|home| home.join(".ssh").join("config")) else {
            return Ok(ConfigScan {
                path: None,
                hosts: Vec::new(),
            });
        };
        let hosts = match std::fs::read_to_string(&path) {
            Ok(text) => config::parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.into()),
        };
        Ok(ConfigScan {
            path: Some(path.display().to_string()),
            hosts,
        })
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_secret_status() -> Result<serde_json::Value> {
    // The first keyring call connects to D-Bus / the OS store.
    blocking(|| {
        Ok(serde_json::json!({
            "keyring_available": secret::keyring_available(),
        }))
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_forget_secret(app: AppHandle, host_id: String) -> Result<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        state.db.read(|c| ssh_hosts::require(c, &host_id))?;
        app.state::<SshState>()
            .vault
            .forget_all(&state.db, &SecretOwner::Host(host_id))
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub fn ssh_forget_host_key(app: AppHandle, host_id: String) -> Result<usize> {
    let host = app
        .state::<AppState>()
        .db
        .read(|c| ssh_hosts::require(c, &host_id))?;
    app.state::<SshState>().known.forget(&host.host, host.port)
}

#[derive(Serialize)]
pub struct TestReport {
    ok: bool,
    failed_stage: Option<Stage>,
    error: Option<String>,
}

/// "Kiểm tra kết nối" in MOD-17: runs host key check and login with the
/// values in the form, then disconnects. Nothing is saved.
#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_test_connection(
    app: AppHandle,
    host: SshHostInput,
    host_id: Option<String>,
    password: Option<String>,
    passphrase: Option<String>,
    private_key: Option<String>,
) -> Result<TestReport> {
    let lookup_app = app.clone();
    let target = blocking(move || {
        lookup_app.state::<AppState>().db.read(|c| {
            let input = ssh_hosts::normalize(c, host_id.as_deref(), host)?;
            if input.use_system_ssh {
                return Err(Error::msg(
                    "Không kiểm tra được máy chủ dùng OpenSSH hệ thống",
                ));
            }
            let saved = match host_id.as_deref() {
                Some(id) => ssh_hosts::get(c, id)?,
                None => None,
            };
            let probe = SshHost {
                id: saved
                    .as_ref()
                    .map(|host| host.id.clone())
                    .unwrap_or_else(|| format!("{TEST_PREFIX}{}", Uuid::new_v4())),
                name: input.name,
                host: input.host,
                port: input.port,
                username: input.username,
                auth_method: input.auth_method,
                key_path: input.key_path,
                key_source: input.key_source,
                identity_id: input.identity_id,
                key_id: input.key_id,
                jump_host_id: input.jump_host_id,
                use_system_ssh: false,
                agent_forwarding: input.agent_forwarding,
                default_dir: input.default_dir,
                secret_store: saved
                    .as_ref()
                    .map(|host| host.secret_store.clone())
                    .unwrap_or_else(|| "none".into()),
                color: input.color,
                tags: input.tags,
                sort_order: 0,
                created_at: String::new(),
                last_connected_at: None,
            };
            resolve(c, probe)
        })
    })
    .await?;

    let ssh = app.state::<SshState>();
    let env: Arc<dyn SshEnv> = Arc::new(TestEnv {
        base: ssh.manager.env(),
        password: password.filter(|value| !value.is_empty()),
        passphrase: passphrase.filter(|value| !value.is_empty()),
        private_key: private_key.filter(|value| !value.trim().is_empty()),
    });
    let via: Option<Lease> = match target.host.jump_host_id.as_deref() {
        Some(jump) => match ssh.manager.lease(jump).await {
            Ok(lease) => Some(lease),
            Err(error) => {
                return Ok(TestReport {
                    ok: false,
                    failed_stage: Some(Stage::Network),
                    error: Some(format!("Jump host: {error}")),
                })
            }
        },
        None => None,
    };
    match client::connect(env, &target, via.as_ref().map(Lease::handle)).await {
        Ok(connection) => {
            connection.close().await;
            Ok(TestReport {
                ok: true,
                failed_stage: None,
                error: None,
            })
        }
        Err(error) => Ok(TestReport {
            ok: false,
            failed_stage: Some(error.stage),
            error: Some(error.message),
        }),
    }
}

fn emit_status(app: &AppHandle, live: (&str, &str, &str), status: &str, exit_code: Option<i32>) {
    let (run_event_id, command_id, session_id) = live;
    let _ = app.emit(
        events::PROCESS_STATUS,
        StatusEvent {
            run_event_id: run_event_id.into(),
            command_id: command_id.into(),
            session_id: session_id.into(),
            status: status.into(),
            exit_code,
            pid: None,
        },
    );
}

/// What happens to the tab when the channel ends: a lost connection keeps
/// the terminal (and its output) for "Kết nối lại"; anything else ends it.
fn on_end(
    app: AppHandle,
    run_event_id: String,
    command_id: String,
    session_id: String,
) -> impl FnOnce(TerminalEnd) + Send + 'static {
    move |end| {
        let ids = (
            run_event_id.as_str(),
            command_id.as_str(),
            session_id.as_str(),
        );
        let (status, code) = match end {
            TerminalEnd::Lost => {
                emit_status(&app, ids, "disconnected", None);
                return;
            }
            TerminalEnd::Exited(Some(0)) => ("success", Some(0)),
            TerminalEnd::Exited(Some(code)) => ("failed", Some(code)),
            TerminalEnd::Exited(None) | TerminalEnd::Stopped => ("stopped", None),
        };
        app.state::<AppState>().processes.remove(&run_event_id);
        app.state::<SshState>().terminals.remove(&run_event_id);
        emit_status(&app, ids, status, code);
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_terminal_open(
    app: AppHandle,
    host_id: String,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<TerminalInfo> {
    let (cols, rows) = (cols.unwrap_or(100), rows.unwrap_or(30));
    let host = app
        .state::<AppState>()
        .db
        .read(|c| ssh_hosts::require(c, &host_id))?;
    if host.use_system_ssh {
        return blocking(move || open_system_ssh(&app, host, cols, rows)).await;
    }

    let ssh = app.state::<SshState>();
    let lease = ssh.manager.lease(&host_id).await?;
    let state = app.state::<AppState>();
    let id = Uuid::new_v4().to_string();
    let run_event_id = format!("ssh:{id}");
    let command_id = run_event_id.clone();
    let session_id = format!("ssh-session:{id}");
    let terminal = SshTerminal::new(
        run_event_id.clone(),
        host_id,
        buffer_bytes(&state),
        cols,
        rows,
    );

    state.processes.reserve(&run_event_id);
    ssh.terminals
        .insert(run_event_id.clone(), Arc::clone(&terminal));
    let started = terminal
        .start(
            lease,
            state.ipc.clone(),
            on_end(
                app.clone(),
                run_event_id.clone(),
                command_id.clone(),
                session_id.clone(),
            ),
        )
        .await;
    if let Err(error) = started {
        state.processes.cancel(&run_event_id);
        ssh.terminals.remove(&run_event_id);
        return Err(error);
    }

    state.processes.insert(Live {
        run_event_id: run_event_id.clone(),
        command_id: command_id.clone(),
        session_id: session_id.clone(),
        group_id: "ssh".into(),
        pid: 0,
        pty: terminal,
        shell_kind: "ssh".into(),
        history_level: 0,
    });
    emit_status(
        &app,
        (&run_event_id, &command_id, &session_id),
        "running",
        None,
    );
    Ok(TerminalInfo {
        run_event_id,
        command_id,
        session_id,
        pid: 0,
        shell_kind: "ssh".into(),
        history_level: 0,
    })
}

/// Opens a new shell for a tab whose connection was lost. The old output
/// stays above a separator line.
#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_terminal_reconnect(app: AppHandle, run_event_id: String) -> Result<()> {
    let ssh = app.state::<SshState>();
    let terminal = ssh
        .terminals
        .get(&run_event_id)
        .map(|entry| Arc::clone(entry.value()))
        .ok_or_else(|| Error::msg("Terminal SSH không còn tồn tại"))?;
    if terminal.is_connected() {
        return Ok(());
    }
    let state = app.state::<AppState>();
    let live = state
        .processes
        .get(&run_event_id)
        .ok_or_else(|| Error::msg("Terminal SSH không còn tồn tại"))?;
    let lease = ssh.manager.lease(terminal.host_id()).await?;
    let ipc = state.ipc.clone();
    terminal.notice(&ipc, "Đã kết nối lại").await;
    terminal
        .start(
            lease,
            ipc,
            on_end(
                app.clone(),
                live.run_event_id.clone(),
                live.command_id.clone(),
                live.session_id.clone(),
            ),
        )
        .await?;
    emit_status(
        &app,
        (&live.run_event_id, &live.command_id, &live.session_id),
        "running",
        None,
    );
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ssh_disconnect(app: AppHandle, host_id: String) -> Result<()> {
    app.state::<SshState>().manager.disconnect(&host_id).await;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn ssh_hostkey_respond(
    state: State<SshState>,
    request_id: String,
    trust: bool,
    remember: Option<bool>,
) -> Result<()> {
    let (_, tx) = state
        .pending
        .host_keys
        .remove(&request_id)
        .ok_or_else(|| Error::msg("Yêu cầu xác minh đã hết hạn"))?;
    let _ = tx.send(HostKeyDecision {
        trust,
        remember: remember.unwrap_or(true),
    });
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn ssh_auth_respond(
    state: State<SshState>,
    request_id: String,
    answers: Vec<String>,
    store: Option<SecretStore>,
) -> Result<()> {
    let (_, tx) = state
        .pending
        .auth
        .remove(&request_id)
        .ok_or_else(|| Error::msg("Yêu cầu đăng nhập đã hết hạn"))?;
    let reply = (!answers.is_empty()).then(|| AuthReply {
        answers,
        store: store.unwrap_or(SecretStore::None),
    });
    let _ = tx.send(reply);
    Ok(())
}

/// `process_stop` for an SSH terminal (MOD-01). Called on a blocking thread.
pub(crate) fn stop_terminal(app: &AppHandle, live: &Live, force: bool) -> Result<()> {
    let state = app.state::<AppState>();
    if !live.pty.stop(force) {
        // Disconnected: there is no shell left, only the tab's record.
        state.processes.remove(&live.run_event_id);
        app.state::<SshState>().terminals.remove(&live.run_event_id);
        emit_status(
            app,
            (&live.run_event_id, &live.command_id, &live.session_id),
            "stopped",
            None,
        );
        return Ok(());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if state.processes.get(&live.run_event_id).is_none() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(Error::msg("Phiên SSH chưa đóng sau 5 giây"))
}

/// `ssh` arguments for a "Dùng OpenSSH hệ thống" host, up to and including
/// the destination. `command` (a saved command) gets a forced TTY.
fn system_ssh_argv(app: &AppHandle, host: SshHost, command: Option<&str>) -> Result<Vec<String>> {
    let state = app.state::<AppState>();
    let target = state.db.read(|c| resolve(c, host))?;
    let host = &target.host;
    let mut argv = vec!["ssh".to_string(), "-p".into(), host.port.to_string()];
    if command.is_some() {
        argv.push("-t".into());
    }
    match &target.key {
        KeyChoice::File(key) => {
            argv.push("-i".into());
            argv.push(config::expand_home(key).display().to_string());
        }
        KeyChoice::Defaults => {}
        KeyChoice::Keychain { .. } | KeyChoice::HostContent => {
            return Err(Error::msg(
                "OpenSSH hệ thống chỉ dùng được tệp khóa, không dùng được khóa trong Keychain",
            ));
        }
    }
    if host.agent_forwarding {
        argv.push("-A".into());
    }
    if let Some(jump_id) = &host.jump_host_id {
        // OpenSSH wants the hops in the order they are crossed: outermost first.
        let hops = state.db.read(|c| {
            ssh_hosts::jump_chain(c, jump_id, Some(&host.id))?
                .into_iter()
                .rev()
                .map(|jump| resolve(c, jump))
                .collect::<Result<Vec<_>>>()
        })?;
        argv.push("-J".into());
        argv.push(jump_spec(&hops));
    }
    argv.push(format!("{}@{}", target.username, host.host));
    if let Some(command) = command {
        argv.push(command.to_string());
    }
    Ok(argv)
}

/// `-J` value: `user@host:port` per hop, comma separated.
fn jump_spec(hops: &[Target]) -> String {
    hops.iter()
        .map(|hop| {
            format!(
                "{}@{}:{}",
                hop.username,
                bracket(&hop.host.host),
                hop.host.port
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Quotes a word for the server's POSIX shell.
fn posix_quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./@%+=:,-".contains(c))
    {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The command line sent to the server: run in the host's default folder
/// when it has one (`~` and `~/x` keep working).
fn remote_command_line(host: &SshHost, command: &str) -> String {
    let Some(dir) = host
        .default_dir
        .as_deref()
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
    else {
        return command.to_string();
    };
    let cd = if dir == "~" {
        "cd ~".to_string()
    } else if let Some(rest) = dir.strip_prefix("~/") {
        format!("cd ~/{}", posix_quote(rest))
    } else {
        format!("cd {}", posix_quote(dir))
    };
    format!("{cd} && {command}")
}

/// A saved command started on its SSH host.
pub(crate) struct RemoteLaunch {
    /// 0 for russh channels; the local `ssh` process for system SSH hosts.
    pub pid: u32,
    pub pty: Arc<dyn crate::pty::terminal_io::TerminalIo>,
    pub killer: Option<Box<dyn portable_pty::ChildKiller + Send + Sync>>,
}

/// Runs `command` on the host behind a PTY. `on_exit` receives the exit code
/// like a local process does: `None` when stopped or killed by a signal,
/// `Some(255)` when the connection dropped (OpenSSH's convention).
pub(crate) async fn launch_remote_command(
    app: &AppHandle,
    host_id: &str,
    command: &str,
    run_event_id: String,
    cols: u16,
    rows: u16,
    on_exit: impl FnOnce(Option<i32>) + Send + 'static,
) -> Result<RemoteLaunch> {
    let state = app.state::<AppState>();
    let host = state.db.read(|c| ssh_hosts::require(c, host_id))?;
    let line = remote_command_line(&host, command);
    let buffer = buffer_bytes(&state);

    if host.use_system_ssh {
        let argv = system_ssh_argv(app, host, Some(&line))?;
        let spawned = spawn::spawn_argv(
            &argv,
            cols,
            rows,
            buffer,
            run_event_id,
            state.ipc.clone(),
            on_exit,
        )
        .map_err(|error| Error::msg(format!("Không chạy được lệnh ssh: {error}")))?;
        return Ok(RemoteLaunch {
            pid: spawned.pid,
            pty: spawned.pty,
            killer: Some(spawned.killer),
        });
    }

    let lease = app.state::<SshState>().manager.lease(host_id).await?;
    let terminal = SshTerminal::new(run_event_id, host_id.to_string(), buffer, cols, rows);
    terminal
        .start_exec(lease, state.ipc.clone(), line, move |end| {
            on_exit(match end {
                TerminalEnd::Exited(code) => code,
                TerminalEnd::Stopped => None,
                TerminalEnd::Lost => Some(255),
            })
        })
        .await?;
    Ok(RemoteLaunch {
        pid: 0,
        pty: terminal,
        killer: None,
    })
}

/// A profile marked "Dùng OpenSSH hệ thống" runs the `ssh` binary in a local
/// PTY, so ProxyCommand, FIDO keys and the like work as in a normal terminal.
fn open_system_ssh(app: &AppHandle, host: SshHost, cols: u16, rows: u16) -> Result<TerminalInfo> {
    let state = app.state::<AppState>();
    let argv = system_ssh_argv(app, host, None)?;

    let id = Uuid::new_v4().to_string();
    let run_event_id = format!("ssh:{id}");
    let command_id = run_event_id.clone();
    let session_id = format!("ssh-session:{id}");
    state.processes.reserve(&run_event_id);
    let exit_app = app.clone();
    let exit_ids = (run_event_id.clone(), command_id.clone(), session_id.clone());
    let spawned = spawn::spawn_argv(
        &argv,
        cols,
        rows,
        buffer_bytes(&state),
        run_event_id.clone(),
        state.ipc.clone(),
        move |code| {
            let (run_event_id, command_id, session_id) = exit_ids;
            exit_app.state::<AppState>().processes.remove(&run_event_id);
            let status = match code {
                Some(0) => "success",
                Some(_) => "failed",
                None => "stopped",
            };
            emit_status(
                &exit_app,
                (&run_event_id, &command_id, &session_id),
                status,
                code,
            );
        },
    );
    let spawned = match spawned {
        Ok(spawned) => spawned,
        Err(error) => {
            state.processes.cancel(&run_event_id);
            return Err(Error::msg(format!("Không chạy được lệnh ssh: {error}")));
        }
    };
    let pid = spawned.pid;
    state.processes.insert(Live {
        run_event_id: run_event_id.clone(),
        command_id: command_id.clone(),
        session_id: session_id.clone(),
        group_id: "ssh".into(),
        pid,
        pty: spawned.pty,
        shell_kind: "ssh".into(),
        history_level: 0,
    });
    let _ = app.emit(
        events::PROCESS_STATUS,
        StatusEvent {
            run_event_id: run_event_id.clone(),
            command_id: command_id.clone(),
            session_id: session_id.clone(),
            status: "running".into(),
            exit_code: None,
            pid: Some(pid),
        },
    );
    Ok(TerminalInfo {
        run_event_id,
        command_id,
        session_id,
        pid,
        shell_kind: "ssh".into(),
        history_level: 0,
    })
}

fn bracket(host: &str) -> String {
    if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(default_dir: Option<&str>) -> SshHost {
        SshHost {
            id: "h".into(),
            name: "h".into(),
            host: "h".into(),
            port: 22,
            username: "u".into(),
            auth_method: "auto".into(),
            key_path: None,
            key_source: "file".into(),
            identity_id: None,
            key_id: None,
            jump_host_id: None,
            use_system_ssh: false,
            agent_forwarding: false,
            default_dir: default_dir.map(str::to_string),
            secret_store: "none".into(),
            color: None,
            tags: Vec::new(),
            sort_order: 0,
            created_at: "0".into(),
            last_connected_at: None,
        }
    }

    #[test]
    fn remote_commands_start_in_the_default_folder() {
        assert_eq!(remote_command_line(&host(None), "ls -la"), "ls -la");
        assert_eq!(remote_command_line(&host(Some("~")), "ls"), "cd ~ && ls");
        assert_eq!(
            remote_command_line(&host(Some("~/my app")), "ls"),
            "cd ~/'my app' && ls"
        );
        assert_eq!(
            remote_command_line(&host(Some("/var/www/it's")), "ls"),
            "cd '/var/www/it'\\''s' && ls"
        );
        assert_eq!(remote_command_line(&host(Some("  ")), "ls"), "ls");
    }

    #[test]
    fn jump_spec_lists_hops_outermost_first() {
        let slot = || SecretSlot {
            owner: SecretOwner::Host("h".into()),
            store: SecretStore::None,
        };
        let hop = |user: &str, address: &str, port: u16| {
            let mut host = host(None);
            host.host = address.into();
            host.port = port;
            Target {
                host,
                username: user.into(),
                key: KeyChoice::Defaults,
                password: slot(),
                passphrase: slot(),
            }
        };
        assert_eq!(
            jump_spec(&[hop("a", "edge.example", 22), hop("b", "fd00::1", 2222)]),
            "a@edge.example:22,b@[fd00::1]:2222"
        );
    }
}
