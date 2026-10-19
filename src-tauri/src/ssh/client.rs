//! Opening one SSH connection: TCP (or a jump host channel), host key check,
//! then authentication in OpenSSH's order: agent, key files, then
//! keyboard-interactive / password.

use super::config::expand_home;
use super::known_hosts::{fingerprint, home_dir, Verdict};
use super::secret::{SecretKind, SecretStore};
use super::{
    label, AuthPrompt, AuthPromptKind, AuthReply, ChangedKey, ConnState, HostKeyPrompt, KeyChoice,
    PromptField, SecretSlot, SshEnv, Target,
};
use crate::error::{Error, Result};
use russh::client::{self, AuthResult, Handle, KeyboardInteractiveAuthResponse};
use russh::keys::{self, Algorithm, HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey};
use russh::{Disconnect, MethodKind, MethodSet};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TCP_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_PROMPTS: u32 = 3;

/// Which step of a connection failed, for "Kiểm tra kết nối" in MOD-17.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Network,
    HostKey,
    Auth,
}

#[derive(Debug)]
pub struct ConnectError {
    pub stage: Stage,
    pub message: String,
}

impl From<ConnectError> for Error {
    fn from(error: ConnectError) -> Self {
        Error::msg(error.message)
    }
}

fn fail(stage: Stage, message: impl Into<String>) -> ConnectError {
    ConnectError {
        stage,
        message: message.into(),
    }
}

pub struct ClientHandler {
    host_id: String,
    label: String,
    host: String,
    port: u16,
    env: Arc<dyn SshEnv>,
    /// Why the host key was refused; russh only reports "unknown key".
    refusal: Arc<Mutex<Option<String>>>,
    closing: Arc<AtomicBool>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_key: &keys::PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        let key = match server_key {
            keys::PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            keys::PublicKeyOrCertificate::Certificate(cert) => {
                PublicKey::new(cert.public_key().clone(), "")
            }
        };
        let prompt = |changed| HostKeyPrompt {
            host_id: self.host_id.clone(),
            label: self.label.clone(),
            algorithm: key.algorithm().to_string(),
            fingerprint: fingerprint(&key),
            changed,
        };
        match self.env.known_hosts().check(&self.host, self.port, &key) {
            Verdict::Known => Ok(true),
            Verdict::Unknown => {
                self.env.status(&self.host_id, ConnState::Verifying, None);
                let decision = self.env.host_key_prompt(prompt(None)).await;
                if !decision.trust {
                    *self.refusal.lock().expect("refusal lock") =
                        Some("Đã hủy kết nối: chưa tin cậy khóa của máy chủ".into());
                    return Ok(false);
                }
                if decision.remember {
                    if let Err(error) = self.env.known_hosts().learn(&self.host, self.port, &key) {
                        eprintln!("Command Manager: {error}");
                    }
                }
                Ok(true)
            }
            Verdict::Changed {
                path,
                line,
                old_fingerprint,
            } => {
                *self.refusal.lock().expect("refusal lock") = Some(format!(
                    "Khóa của máy chủ đã thay đổi (khác dòng {line} trong {}). Kết nối bị chặn.",
                    path.display()
                ));
                let _ = self
                    .env
                    .host_key_prompt(prompt(Some(ChangedKey {
                        known_hosts_path: path.display().to_string(),
                        line,
                        old_fingerprint,
                    })))
                    .await;
                Ok(false)
            }
        }
    }

    async fn disconnected(
        &mut self,
        reason: client::DisconnectReason<Self::Error>,
    ) -> std::result::Result<(), Self::Error> {
        if self.closing.load(Ordering::SeqCst) {
            self.env.status(&self.host_id, ConnState::Closed, None);
        } else {
            let detail = match reason {
                client::DisconnectReason::ReceivedDisconnect(info) => {
                    format!("Máy chủ đóng kết nối: {}", info.message)
                }
                client::DisconnectReason::Error(error) => format!("Mất kết nối: {error}"),
            };
            self.env
                .status(&self.host_id, ConnState::Lost, Some(detail));
        }
        Ok(())
    }
}

pub struct Connected {
    pub handle: Handle<ClientHandler>,
    pub closing: Arc<AtomicBool>,
}

impl Connected {
    pub async fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
        let _ = self
            .handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }
}

/// Connects and logs in. `via` is an open connection to the jump host.
pub async fn connect(
    env: Arc<dyn SshEnv>,
    t: &Target,
    via: Option<&Handle<ClientHandler>>,
) -> std::result::Result<Connected, ConnectError> {
    env.status(&t.host.id, ConnState::Connecting, None);
    let result = connect_inner(env.clone(), t, via).await;
    match &result {
        Ok(_) => {
            env.status(&t.host.id, ConnState::Ready, None);
            env.connected(&t.host.id);
        }
        Err(error) => env.status(&t.host.id, ConnState::Closed, Some(error.message.clone())),
    }
    result
}

async fn connect_inner(
    env: Arc<dyn SshEnv>,
    t: &Target,
    via: Option<&Handle<ClientHandler>>,
) -> std::result::Result<Connected, ConnectError> {
    let config = Arc::new(client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        inactivity_timeout: None,
        nodelay: true,
        ..Default::default()
    });
    let refusal = Arc::new(Mutex::new(None));
    let closing = Arc::new(AtomicBool::new(false));
    let handler = ClientHandler {
        host_id: t.host.id.clone(),
        label: label(t),
        host: t.host.host.clone(),
        port: t.host.port,
        env: env.clone(),
        refusal: refusal.clone(),
        closing: closing.clone(),
    };
    let address = label(t);

    let handshake = match via {
        Some(jump) => {
            let channel = jump
                .channel_open_direct_tcpip(t.host.host.clone(), t.host.port as u32, "127.0.0.1", 0)
                .await
                .map_err(|error| {
                    fail(
                        Stage::Network,
                        format!("Jump host không mở được đường tới {address}: {error}"),
                    )
                })?;
            client::connect_stream(config, channel.into_stream(), handler).await
        }
        None => {
            let stream = tokio::time::timeout(
                TCP_TIMEOUT,
                tokio::net::TcpStream::connect((t.host.host.as_str(), t.host.port)),
            )
            .await
            .map_err(|_| {
                fail(
                    Stage::Network,
                    format!("Hết thời gian kết nối tới {address}"),
                )
            })?
            .map_err(|error| {
                fail(
                    Stage::Network,
                    format!("Không kết nối được tới {address}: {error}"),
                )
            })?;
            let _ = stream.set_nodelay(true);
            client::connect_stream(config, stream, handler).await
        }
    };
    let refused = || refusal.lock().expect("refusal lock").take();
    let mut handle = handshake.map_err(|error| match refused() {
        Some(reason) => fail(Stage::HostKey, reason),
        None => fail(Stage::Network, format!("Bắt tay SSH thất bại: {error}")),
    })?;

    env.status(&t.host.id, ConnState::Authenticating, None);
    if let Err(error) = authenticate(&env, &mut handle, t).await {
        let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
        // A refused host key surfaces as an error of the first auth request.
        return Err(match refused() {
            Some(reason) => fail(Stage::HostKey, reason),
            None => fail(Stage::Auth, error.to_string()),
        });
    }
    Ok(Connected { handle, closing })
}

fn ssh_error(error: impl std::fmt::Display) -> Error {
    Error::msg(format!("SSH: {error}"))
}

fn cancelled() -> Error {
    Error::msg("Đã hủy đăng nhập")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// The chosen key (Keychain, pasted text, `key_path`) or ~/.ssh defaults.
    Keys,
    Agent,
    Password,
}

/// The order logins are tried in.
///
/// Every refused key counts against the server's `MaxAuthTries` (6 by
/// default, often 3 on hardened servers). An agent holding several
/// unrelated keys would use those up and get the connection dropped before
/// the key the user picked is offered, so a chosen key always goes first.
fn auth_plan(t: &Target) -> Vec<Step> {
    let chosen_key = !matches!(t.key, KeyChoice::Defaults);
    match t.host.auth_method.as_str() {
        "agent" => vec![Step::Agent],
        "key" => vec![Step::Keys],
        "password" => vec![Step::Password],
        _ if chosen_key => vec![Step::Keys, Step::Agent, Step::Password],
        _ => vec![Step::Agent, Step::Keys, Step::Password],
    }
}

fn dropped_during_auth() -> Error {
    Error::msg(
        "Máy chủ đã ngắt kết nối trong lúc xác thực. Thường là do thử quá nhiều khóa \
         (MaxAuthTries): hãy chọn đúng khóa cho máy chủ này, hoặc đặt phương thức \
         xác thực là \"Khóa\" để không thử các khóa trong SSH agent.",
    )
}

async fn authenticate(
    env: &Arc<dyn SshEnv>,
    handle: &mut Handle<ClientHandler>,
    t: &Target,
) -> Result<()> {
    let mut remaining: Option<MethodSet> = None;
    for step in auth_plan(t) {
        if handle.is_closed() {
            return Err(dropped_during_auth());
        }
        let done = match step {
            Step::Keys => try_keys(env, handle, t, &mut remaining).await?,
            Step::Agent => try_agent(handle, t, &mut remaining).await?,
            Step::Password => try_password(env, handle, t, &remaining).await?,
        };
        if done {
            return Ok(());
        }
    }
    if handle.is_closed() {
        return Err(dropped_during_auth());
    }

    Err(Error::msg(match t.host.auth_method.as_str() {
        "agent" => "Máy chủ không chấp nhận khóa nào trong SSH agent",
        "key" => "Máy chủ không chấp nhận khóa đã chọn",
        "password" => "Máy chủ không chấp nhận mật khẩu",
        _ => "Máy chủ không chấp nhận khóa, agent hay mật khẩu nào đã thử",
    }))
}

async fn try_password(
    env: &Arc<dyn SshEnv>,
    handle: &mut Handle<ClientHandler>,
    t: &Target,
    remaining: &Option<MethodSet>,
) -> Result<bool> {
    let allows = |kind: MethodKind| remaining.as_ref().is_none_or(|set| set.contains(&kind));
    let stored = env.load_secret(&t.password, SecretKind::Password).await;
    if allows(MethodKind::KeyboardInteractive) {
        if let Some(done) = keyboard_flow(env, handle, t, stored.clone()).await? {
            return done
                .then_some(true)
                .ok_or_else(|| Error::msg("Xác thực thất bại"));
        }
    }
    Ok(allows(MethodKind::Password) && password_flow(env, handle, t, stored).await?)
}

async fn rsa_hash(handle: &Handle<ClientHandler>, key: &PublicKey) -> Option<HashAlg> {
    if !matches!(key.algorithm(), Algorithm::Rsa { .. }) {
        return None;
    }
    handle
        .best_supported_rsa_hash()
        .await
        .ok()
        .flatten()
        .flatten()
}

type DynAgent = keys::agent::client::AgentClient<
    Box<dyn keys::agent::client::AgentStream + Send + Unpin + 'static>,
>;

async fn connect_agent() -> Option<DynAgent> {
    #[cfg(unix)]
    {
        keys::agent::client::AgentClient::connect_env()
            .await
            .ok()
            .map(|agent| agent.dynamic())
    }
    #[cfg(windows)]
    {
        if let Ok(agent) =
            keys::agent::client::AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
                .await
        {
            return Some(agent.dynamic());
        }
        keys::agent::client::AgentClient::connect_pageant()
            .await
            .ok()
            .map(|agent| agent.dynamic())
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

async fn try_agent(
    handle: &mut Handle<ClientHandler>,
    t: &Target,
    remaining: &mut Option<MethodSet>,
) -> Result<bool> {
    let Some(mut agent) = connect_agent().await else {
        return Ok(false);
    };
    let Ok(identities) = agent.request_identities().await else {
        return Ok(false);
    };
    for identity in identities {
        if handle.is_closed() {
            break;
        }
        let key = identity.public_key().into_owned();
        let hash = rsa_hash(handle, &key).await;
        match handle
            .authenticate_publickey_with(&t.username, key, hash, &mut agent)
            .await
        {
            Ok(AuthResult::Success) => return Ok(true),
            Ok(AuthResult::Failure {
                remaining_methods, ..
            }) => *remaining = Some(remaining_methods),
            // The agent refused to sign (locked, confirm denied): next key.
            Err(_) => {}
        }
    }
    Ok(false)
}

/// Where a key comes from: a file, or the key text saved with the host.
enum KeySource {
    File { path: PathBuf, explicit: bool },
    Saved(String),
}

impl KeySource {
    fn decode(&self, passphrase: Option<&str>) -> std::result::Result<PrivateKey, keys::Error> {
        match self {
            Self::File { path, .. } => keys::load_secret_key(path, passphrase),
            Self::Saved(text) => keys::decode_secret_key(text.trim(), passphrase),
        }
    }

    /// A key the user chose (not a ~/.ssh default): failures are errors.
    fn explicit(&self) -> bool {
        match self {
            Self::File { explicit, .. } => *explicit,
            Self::Saved(_) => true,
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::File { path, .. } => format!("Tệp khóa {}", path.display()),
            Self::Saved(_) => "Khóa riêng đã lưu".into(),
        }
    }
}

async fn key_sources(env: &Arc<dyn SshEnv>, t: &Target) -> Result<Vec<KeySource>> {
    let saved = |slot: SecretSlot, missing: String| async move {
        env.load_secret(&slot, SecretKind::PrivateKey)
            .await
            .map(|text| vec![KeySource::Saved(text)])
            .ok_or_else(|| Error::msg(missing))
    };
    match &t.key {
        KeyChoice::Keychain { name, slot, .. } => {
            saved(
                slot.clone(),
                format!("Không đọc được khóa \"{name}\" trong Keychain trên máy này"),
            )
            .await
        }
        KeyChoice::HostContent => saved(
            t.host_slot(),
            "Chưa có nội dung khóa riêng đã lưu cho máy chủ này (hoặc không đọc được trên máy này)"
                .into(),
        )
        .await,
        KeyChoice::File(path) => Ok(vec![KeySource::File {
            path: expand_home(path),
            explicit: true,
        }]),
        KeyChoice::Defaults => {
            let Some(ssh_dir) = home_dir().map(|home| home.join(".ssh")) else {
                return Ok(Vec::new());
            };
            Ok(["id_ed25519", "id_ecdsa", "id_rsa"]
                .into_iter()
                .map(|name| ssh_dir.join(name))
                .filter(|path| path.is_file())
                .map(|path| KeySource::File {
                    path,
                    explicit: false,
                })
                .collect())
        }
    }
}

/// Checks key text pasted in MOD-17 before it is saved. An encrypted key is
/// fine: the passphrase is asked (or read from the vault) at login.
pub fn check_private_key(text: &str) -> Result<()> {
    let text = text.trim();
    if text.starts_with("PuTTY-User-Key-File") {
        return Err(Error::msg(
            "Khóa định dạng PuTTY (.ppk) chưa được hỗ trợ; hãy xuất sang OpenSSH bằng PuTTYgen",
        ));
    }
    match keys::decode_secret_key(text, None) {
        Ok(_) | Err(keys::Error::KeyIsEncrypted) => Ok(()),
        Err(error) => Err(Error::msg(format!(
            "Nội dung khóa riêng không hợp lệ: {error}"
        ))),
    }
}

async fn try_keys(
    env: &Arc<dyn SshEnv>,
    handle: &mut Handle<ClientHandler>,
    t: &Target,
    remaining: &mut Option<MethodSet>,
) -> Result<bool> {
    for source in key_sources(env, t).await? {
        if handle.is_closed() {
            break;
        }
        let (key, typed) = match source.decode(None) {
            Ok(key) => (key, None),
            Err(keys::Error::KeyIsEncrypted) => match unlock_key(env, t, &source).await? {
                Some(unlocked) => unlocked,
                None => continue,
            },
            Err(error) if source.explicit() => {
                return Err(Error::msg(format!(
                    "Không đọc được {}: {error}",
                    source.describe().to_lowercase()
                )));
            }
            Err(_) => continue,
        };
        let hash = rsa_hash(handle, key.public_key()).await;
        let result = handle
            .authenticate_publickey(&t.username, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
            .await
            .map_err(ssh_error)?;
        match result {
            AuthResult::Success => {
                if let Some((value, store)) = typed {
                    env.save_secret(&t.passphrase.owner, store, SecretKind::Passphrase, value)
                        .await;
                }
                return Ok(true);
            }
            AuthResult::Failure {
                remaining_methods, ..
            } => *remaining = Some(remaining_methods),
        }
    }
    Ok(false)
}

type Unlocked = (PrivateKey, Option<(String, SecretStore)>);

/// Decrypts a key with the saved passphrase, or asks (MOD-19). `None` skips
/// this key; cancelling an explicitly chosen key aborts the login.
async fn unlock_key(
    env: &Arc<dyn SshEnv>,
    t: &Target,
    source: &KeySource,
) -> Result<Option<Unlocked>> {
    if let Some(saved) = env.load_secret(&t.passphrase, SecretKind::Passphrase).await {
        if let Ok(key) = source.decode(Some(&saved)) {
            return Ok(Some((key, None)));
        }
    }
    let mut error = None;
    for attempt in 1..=MAX_PROMPTS {
        let reply = env
            .auth_prompt(AuthPrompt {
                host_id: t.host.id.clone(),
                label: label(t),
                kind: AuthPromptKind::Passphrase,
                name: String::new(),
                instructions: format!("{} được bảo vệ bằng passphrase.", source.describe()),
                prompts: vec![PromptField {
                    text: "Passphrase".into(),
                    echo: false,
                }],
                storable: true,
                store: t.passphrase.store,
                error: error.take(),
                attempt,
            })
            .await;
        let Some(AuthReply { answers, store }) = reply else {
            return if source.explicit() {
                Err(cancelled())
            } else {
                Ok(None)
            };
        };
        let passphrase = answers.into_iter().next().unwrap_or_default();
        match source.decode(Some(&passphrase)) {
            Ok(key) => return Ok(Some((key, Some((passphrase, store))))),
            Err(_) => error = Some("Sai passphrase".into()),
        }
    }
    Err(Error::msg("Sai passphrase quá 3 lần"))
}

fn password_prompt(t: &Target, error: Option<String>, attempt: u32) -> AuthPrompt {
    AuthPrompt {
        host_id: t.host.id.clone(),
        label: label(t),
        kind: AuthPromptKind::Password,
        name: String::new(),
        instructions: String::new(),
        prompts: vec![PromptField {
            text: "Mật khẩu".into(),
            echo: false,
        }],
        storable: true,
        store: t.password.store,
        error,
        attempt,
    }
}

async fn password_flow(
    env: &Arc<dyn SshEnv>,
    handle: &mut Handle<ClientHandler>,
    t: &Target,
    mut stored: Option<String>,
) -> Result<bool> {
    let mut error = None;
    let mut attempt = 0;
    loop {
        let (password, store) = match stored.take() {
            Some(saved) => (saved, None),
            None => {
                attempt += 1;
                if attempt > MAX_PROMPTS {
                    return Err(Error::msg("Sai mật khẩu quá 3 lần"));
                }
                let reply = env
                    .auth_prompt(password_prompt(t, error.take(), attempt))
                    .await
                    .ok_or_else(cancelled)?;
                let password = reply.answers.into_iter().next().ok_or_else(cancelled)?;
                (password, Some(reply.store))
            }
        };
        let result = handle
            .authenticate_password(&t.username, &password)
            .await
            .map_err(ssh_error)?;
        match result {
            AuthResult::Success => {
                if let Some(store) = store {
                    env.save_secret(&t.password.owner, store, SecretKind::Password, password)
                        .await;
                }
                return Ok(true);
            }
            // Some servers (russh among them) drop a method from the list
            // after a wrong answer, so only the attempt limit ends the loop.
            AuthResult::Failure { .. } if handle.is_closed() => {
                return Err(dropped_during_auth());
            }
            AuthResult::Failure { .. } => {
                error = Some(if store.is_none() {
                    "Mật khẩu đã lưu không đúng".into()
                } else {
                    "Sai mật khẩu".into()
                });
            }
        }
    }
}

fn looks_like_password(prompt: &str) -> bool {
    let prompt = prompt.to_lowercase();
    prompt.contains("password") || prompt.contains("mật khẩu")
}

/// Keyboard-interactive login (PAM password, OTP, ...). Returns `None` when
/// the server does not actually offer it, so the caller falls back to
/// plain password auth.
async fn keyboard_flow(
    env: &Arc<dyn SshEnv>,
    handle: &mut Handle<ClientHandler>,
    t: &Target,
    mut stored: Option<String>,
) -> Result<Option<bool>> {
    let mut error: Option<String> = None;
    let mut attempt = 0;
    loop {
        let mut response = handle
            .authenticate_keyboard_interactive_start(&t.username, None::<String>)
            .await
            .map_err(ssh_error)?;
        let mut asked = false;
        let mut used_saved = false;
        let mut to_save: Option<(String, SecretStore)> = None;
        loop {
            match response {
                KeyboardInteractiveAuthResponse::Success => {
                    if let Some((value, store)) = to_save {
                        env.save_secret(&t.password.owner, store, SecretKind::Password, value)
                            .await;
                    }
                    return Ok(Some(true));
                }
                KeyboardInteractiveAuthResponse::Failure { .. } => {
                    if handle.is_closed() {
                        return Err(dropped_during_auth());
                    }
                    if !asked && !used_saved {
                        // Offered but not usable: let password auth try.
                        return Ok(None);
                    }
                    error = Some(if asked {
                        "Thông tin đăng nhập không đúng".into()
                    } else {
                        "Mật khẩu đã lưu không đúng".into()
                    });
                    break;
                }
                KeyboardInteractiveAuthResponse::InfoRequest {
                    name,
                    instructions,
                    prompts,
                } => {
                    let answers = if prompts.is_empty() {
                        Vec::new()
                    } else {
                        let password_like = prompts.len() == 1
                            && !prompts[0].echo
                            && looks_like_password(&prompts[0].prompt);
                        match stored.take().filter(|_| password_like) {
                            Some(saved) => {
                                used_saved = true;
                                vec![saved]
                            }
                            None => {
                                if !asked {
                                    attempt += 1;
                                    if attempt > MAX_PROMPTS {
                                        return Err(Error::msg("Đăng nhập thất bại quá 3 lần"));
                                    }
                                }
                                asked = true;
                                let reply = env
                                    .auth_prompt(AuthPrompt {
                                        host_id: t.host.id.clone(),
                                        label: label(t),
                                        kind: AuthPromptKind::KeyboardInteractive,
                                        name,
                                        instructions,
                                        prompts: prompts
                                            .iter()
                                            .map(|prompt| PromptField {
                                                text: prompt.prompt.trim().to_string(),
                                                echo: prompt.echo,
                                            })
                                            .collect(),
                                        storable: password_like,
                                        store: t.password.store,
                                        error: error.take(),
                                        attempt,
                                    })
                                    .await
                                    .ok_or_else(cancelled)?;
                                if reply.answers.len() != prompts.len() {
                                    return Err(cancelled());
                                }
                                if password_like {
                                    to_save = Some((reply.answers[0].clone(), reply.store));
                                }
                                reply.answers
                            }
                        }
                    };
                    response = handle
                        .authenticate_keyboard_interactive_respond(answers)
                        .await
                        .map_err(ssh_error)?;
                }
            }
        }
    }
}

#[cfg(test)]
mod plan_tests {
    use super::*;
    use crate::db::repos::ssh_hosts::SshHost;
    use crate::ssh::secret::SecretOwner;

    fn target(auth_method: &str, key: KeyChoice) -> Target {
        let slot = SecretSlot {
            owner: SecretOwner::Host("h".into()),
            store: SecretStore::None,
        };
        Target {
            host: SshHost {
                id: "h".into(),
                name: "h".into(),
                host: "h".into(),
                port: 22,
                username: "u".into(),
                auth_method: auth_method.into(),
                key_path: None,
                key_source: "file".into(),
                identity_id: None,
                key_id: None,
                jump_host_id: None,
                use_system_ssh: false,
                agent_forwarding: false,
                default_dir: None,
                secret_store: "none".into(),
                color: None,
                tags: Vec::new(),
                sort_order: 0,
                created_at: "0".into(),
                last_connected_at: None,
            },
            username: "u".into(),
            key,
            password: slot.clone(),
            passphrase: slot,
        }
    }

    #[test]
    fn a_chosen_key_is_tried_before_the_agent() {
        for key in [
            KeyChoice::HostContent,
            KeyChoice::File("~/.ssh/prod".into()),
            KeyChoice::Keychain {
                id: "k".into(),
                name: "k".into(),
                slot: SecretSlot {
                    owner: SecretOwner::Key("k".into()),
                    store: SecretStore::Sqlite,
                },
            },
        ] {
            assert_eq!(
                auth_plan(&target("auto", key)),
                vec![Step::Keys, Step::Agent, Step::Password]
            );
        }
        // Without a chosen key, OpenSSH's order: agent, then ~/.ssh/id_*.
        assert_eq!(
            auth_plan(&target("auto", KeyChoice::Defaults)),
            vec![Step::Agent, Step::Keys, Step::Password]
        );
        assert_eq!(
            auth_plan(&target("key", KeyChoice::HostContent)),
            vec![Step::Keys]
        );
        assert_eq!(
            auth_plan(&target("password", KeyChoice::Defaults)),
            vec![Step::Password]
        );
    }
}
