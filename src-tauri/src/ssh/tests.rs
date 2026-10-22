//! End-to-end checks against an in-process russh server: host key prompt,
//! password login (with the keyboard-interactive fallback), saved secrets
//! and a shell channel through [`SshTerminal`].

use super::known_hosts::KnownHosts;
use super::manager::SshManager;
use super::secret::{SecretKind, SecretStore};
use super::terminal::{SshTerminal, TerminalEnd};
use super::*;
use crate::db::repos::ssh_hosts;
use crate::db::Db;
use crate::pty::backpressure::IpcPipe;
use crate::pty::terminal_io::TerminalIo;
use russh::keys::{decode_secret_key, PrivateKey};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId};
use secret::SecretOwner;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

const SERVER_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACDeOLhJBrZdV5AOZh35R9ySVxBp8gdUbtSpM5NAagetkgAAAJB5AHUpeQB1
KQAAAAtzc2gtZWQyNTUxOQAAACDeOLhJBrZdV5AOZh35R9ySVxBp8gdUbtSpM5NAagetkg
AAAEA1N0zKjJY98KfK3kZJsEF9jrfQbODbs4Mp40fVIkoGW944uEkGtl1XkA5mHflH3JJX
EGnyB1Ru1Kkzk0BqB62SAAAAC3Rlc3Qtc2VydmVyAQI=
-----END OPENSSH PRIVATE KEY-----
";
const PASSWORD: &str = "correct horse";

/// Client keys the test server accepts.
const CLIENT_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACCBR8fFUSl94to1Z3vZ4nNd6+LmDYMma7P8x1i4MnCmgAAAAIgaSqZYGkqm
WAAAAAtzc2gtZWQyNTUxOQAAACCBR8fFUSl94to1Z3vZ4nNd6+LmDYMma7P8x1i4MnCmgA
AAAEChBg79Gs16fhBztolQrOUBaO5BWaAP5OHPfFb7GHZdyYFHx8VRKX3i2jVne9nic13r
4uYNgyZrs/zHWLgycKaAAAAAAAECAwQF
-----END OPENSSH PRIVATE KEY-----
";
const ENCRYPTED_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABAKHWN57b
kSgyUrpWHIbnS8AAAAGAAAAAEAAAAzAAAAC3NzaC1lZDI1NTE5AAAAIA5u5MSGFrz++vaz
hNAeiyH3UT073FU18G8+8ExbWbTAAAAAkB3s4vnxtYVmCgnDlaVBmJG6bjEdW3+ydi9i4F
FeNiAIlzONG1sJNbf1wfhmwiubwsXKb16nXcMZmzhf1F5vAbcHkPs6UbXFJDAUgfaU4htK
kh/9baFx0qItH9qOelPvbyegdj2g5koBQBZmiXhAjeVmTxfMPX8ivDlPGlnZe6xPRmEml1
jP34K1hkg+nukZ8g==
-----END OPENSSH PRIVATE KEY-----
";
const ENCRYPTED_PASSPHRASE: &str = "open sesame";
const ACCEPTED_KEYS: [&str; 2] = [
    "AAAAC3NzaC1lZDI1NTE5AAAAIIFHx8VRKX3i2jVne9nic13r4uYNgyZrs/zHWLgycKaA",
    "AAAAC3NzaC1lZDI1NTE5AAAAIA5u5MSGFrz++vazhNAeiyH3UT073FU18G8+8ExbWbTA",
];

#[derive(Clone, Default)]
struct ShellServer {
    /// direct-tcpip channels: their data belongs to the tunnel, not the shell.
    tunnels: std::collections::HashSet<ChannelId>,
}

impl server::Handler for ShellServer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> std::result::Result<Auth, Self::Error> {
        Ok(if user == "deploy" && password == PASSWORD {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: &russh::keys::PublicKey,
    ) -> std::result::Result<Auth, Self::Error> {
        let accepted = ACCEPTED_KEYS.iter().any(|b64| {
            russh::keys::parse_public_key_base64(b64)
                .is_ok_and(|accepted| accepted.key_data() == key.key_data())
        });
        Ok(if user == "deploy" && accepted {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    /// Jump host duty: pipes the channel to the address asked for.
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host_to_connect: &str,
        port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        let Ok(mut upstream) =
            tokio::net::TcpStream::connect((host_to_connect, port_to_connect as u16)).await
        else {
            return Ok(());
        };
        self.tunnels.insert(channel.id());
        reply.accept().await;
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut upstream).await;
        });
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        session.data(channel, b"welcome\r\n".to_vec())?;
        Ok(())
    }

    /// `exec`: prints the command it got and exits with status 3.
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        let line = format!("ran: {}\r\n", String::from_utf8_lossy(data));
        session.data(channel, line.into_bytes())?;
        session.exit_status_request(channel, 3)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }

    /// Echoes input; `exit` ends the shell with status 0.
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        if self.tunnels.contains(&channel) {
            return Ok(());
        }
        if data.starts_with(b"exit") {
            session.exit_status_request(channel, 0)?;
            session.eof(channel)?;
            session.close(channel)?;
        } else {
            session.data(channel, data.to_vec())?;
        }
        Ok(())
    }
}

async fn start_server() -> u16 {
    start_server_with(10).await
}

async fn start_server_with(max_auth_attempts: usize) -> u16 {
    let config = Arc::new(server::Config {
        max_auth_attempts,
        keys: vec![decode_secret_key(SERVER_KEY, None).expect("server key")],
        auth_rejection_time: Duration::from_millis(0),
        auth_rejection_time_initial: Some(Duration::from_millis(0)),
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let config = Arc::clone(&config);
            tokio::spawn(async move {
                if let Ok(session) =
                    server::run_stream(config, stream, ShellServer::default()).await
                {
                    let _ = session.await;
                }
            });
        }
    });
    port
}

fn server_public_key() -> russh::keys::PublicKey {
    let key: PrivateKey = decode_secret_key(SERVER_KEY, None).expect("key");
    // Keys on the wire carry no comment, and comments take part in equality.
    russh::keys::PublicKey::new(key.public_key().key_data().clone(), "")
}

type SavedSecrets = HashMap<(SecretOwner, &'static str), (SecretStore, String)>;

/// A real database for hosts and the Keychain; secrets in memory. Dialogs
/// are answered from a script and recorded.
struct FakeEnv {
    db: Db,
    known: KnownHosts,
    secrets: Mutex<SavedSecrets>,
    passwords: Mutex<Vec<String>>,
    trust: bool,
    asked: Mutex<Vec<String>>,
}

impl FakeEnv {
    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }

    fn secret(&self, owner: SecretOwner, kind: &'static str) -> Option<(SecretStore, String)> {
        self.secrets.lock().unwrap().get(&(owner, kind)).cloned()
    }

    fn set_secret(&self, owner: SecretOwner, kind: &'static str, value: &str) {
        self.secrets
            .lock()
            .unwrap()
            .insert((owner, kind), (SecretStore::Sqlite, value.into()));
    }

    fn sql(&self, sql: &str) {
        self.db.write(|c| Ok(c.execute_batch(sql)?)).expect("sql");
    }

    fn target(&self) -> Target {
        SshEnv::target(self, "h1").expect("target")
    }
}

impl SshEnv for FakeEnv {
    fn target(&self, host_id: &str) -> Result<Target> {
        self.db
            .read(|c| resolve(c, ssh_hosts::require(c, host_id)?))
    }

    fn known_hosts(&self) -> &KnownHosts {
        &self.known
    }

    fn load_secret(
        &self,
        slot: &SecretSlot,
        kind: SecretKind,
    ) -> BoxFuture<'static, Option<String>> {
        let value = self
            .secret(slot.owner.clone(), kind.as_str())
            .map(|(_, value)| value);
        Box::pin(async move { value })
    }

    fn save_secret(
        &self,
        owner: &SecretOwner,
        store: SecretStore,
        kind: SecretKind,
        value: String,
    ) -> BoxFuture<'static, ()> {
        let mut secrets = self.secrets.lock().unwrap();
        let key = (owner.clone(), kind.as_str());
        if store == SecretStore::None {
            secrets.remove(&key);
        } else {
            secrets.insert(key, (store, value));
        }
        Box::pin(async {})
    }

    fn host_key_prompt(&self, prompt: HostKeyPrompt) -> BoxFuture<'static, HostKeyDecision> {
        let kind = if prompt.changed.is_some() {
            "hostkey-changed"
        } else {
            "hostkey"
        };
        self.asked.lock().unwrap().push(kind.into());
        let trust = self.trust && prompt.changed.is_none();
        Box::pin(async move {
            HostKeyDecision {
                trust,
                remember: true,
            }
        })
    }

    fn auth_prompt(&self, prompt: AuthPrompt) -> BoxFuture<'static, Option<AuthReply>> {
        self.asked.lock().unwrap().push(format!(
            "{:?}:{}",
            prompt.kind,
            prompt.error.unwrap_or_default()
        ));
        let answer = {
            let mut passwords = self.passwords.lock().unwrap();
            (!passwords.is_empty()).then(|| passwords.remove(0))
        };
        Box::pin(async move {
            answer.map(|password| AuthReply {
                answers: vec![password],
                store: SecretStore::Sqlite,
            })
        })
    }

    fn status(&self, _: &str, _: ConnState, _: Option<String>) {}

    fn connected(&self, _: &str) {}

    fn spawn(&self, task: BoxFuture<'static, ()>) {
        tokio::spawn(task);
    }
}

fn host_owner() -> SecretOwner {
    SecretOwner::Host("h1".into())
}

/// Host "h1" on the test server. Password auth only, so the user's agent
/// and ~/.ssh keys stay out of the test.
fn env(dir: &std::path::Path, port: u16, passwords: &[&str], trust: bool) -> Arc<FakeEnv> {
    let db = Db::open(dir.join("app.db")).expect("db");
    db.write(|c| {
        let input = ssh_hosts::SshHostInput {
            name: "test".into(),
            host: "127.0.0.1".into(),
            port,
            username: "deploy".into(),
            auth_method: "password".into(),
            key_source: "file".into(),
            ..Default::default()
        };
        ssh_hosts::insert(c, "h1", &input, "0")
    })
    .expect("host");
    Arc::new(FakeEnv {
        db,
        known: KnownHosts::new(dir),
        secrets: Mutex::new(HashMap::new()),
        passwords: Mutex::new(passwords.iter().map(|p| p.to_string()).collect()),
        trust,
        asked: Mutex::new(Vec::new()),
    })
}

async fn read_until(rx: &mut mpsc::Receiver<(String, Vec<u8>)>, needle: &str) -> String {
    let mut seen = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !seen.contains(needle) {
        let chunk = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {needle:?}, got {seen:?}"))
            .expect("pipe open");
        seen.push_str(&String::from_utf8_lossy(&chunk.1));
    }
    seen
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn password_login_shell_and_saved_secret() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &["wrong", PASSWORD], true);
    let manager = SshManager::new(env.clone());

    let lease = manager.lease("h1").await.expect("lease");
    // Unknown host key, then keyboard-interactive is refused and the
    // password dialog takes over; the first answer is wrong.
    assert_eq!(
        env.asked(),
        vec!["hostkey", "Password:", "Password:Sai mật khẩu"]
    );
    assert_eq!(
        env.secret(host_owner(), "password"),
        Some((SecretStore::Sqlite, PASSWORD.to_string()))
    );
    assert_eq!(
        env.known.check("127.0.0.1", port, &server_public_key()),
        known_hosts::Verdict::Known
    );

    let (tx, mut rx) = mpsc::channel(16);
    let terminal = SshTerminal::new("ssh:t1".into(), "h1".into(), 64 * 1024, 80, 24);
    let (end_tx, end_rx) = tokio::sync::oneshot::channel();
    terminal
        .start(lease, IpcPipe::new(tx), move |end| {
            let _ = end_tx.send(end);
        })
        .await
        .expect("start");
    read_until(&mut rx, "welcome").await;

    terminal.resize(120, 40).expect("resize");
    terminal.write(b"hello").expect("write");
    read_until(&mut rx, "hello").await;
    assert!(String::from_utf8_lossy(&terminal.snapshot().expect("snapshot")).contains("hello"));

    terminal.write(b"exit\r").expect("write");
    let end = tokio::time::timeout(Duration::from_secs(5), end_rx)
        .await
        .expect("end in time")
        .expect("end sent");
    assert_eq!(end, TerminalEnd::Exited(Some(0)));
    assert!(!terminal.is_connected());
    assert!(terminal.write(b"x").is_err());

    // The connection is still shared: a second lease asks nothing.
    let _again = manager.lease("h1").await.expect("lease again");
    assert_eq!(env.asked().len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_password_logs_in_without_asking() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &[], true);
    env.known
        .learn("127.0.0.1", port, &server_public_key())
        .expect("learn");
    env.set_secret(host_owner(), "password", PASSWORD);

    let manager = SshManager::new(env.clone());
    manager.lease("h1").await.expect("lease");
    assert!(env.asked().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_changed_host_key_is_refused() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &[PASSWORD], true);
    let recorded = russh::keys::parse_public_key_base64(
        "AAAAC3NzaC1lZDI1NTE5AAAAIKYUHyqlCzWrWZus90y547r0iCIZj21h4hS/fGgq5Qsj",
    )
    .expect("recorded key");
    env.known
        .learn("127.0.0.1", port, &recorded)
        .expect("learn");

    let error = client::connect(env.clone(), &env.target(), None)
        .await
        .err()
        .expect("refused");
    assert_eq!(error.stage, client::Stage::HostKey);
    assert_eq!(env.asked(), vec!["hostkey-changed"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn three_wrong_passwords_give_up() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &["a", "b", "c", "d"], true);
    let error = client::connect(env.clone(), &env.target(), None)
        .await
        .err()
        .expect("rejected");
    assert_eq!(error.stage, client::Stage::Auth);
    assert_eq!(env.passwords.lock().unwrap().len(), 1);
}

/// Switches "h1" to key login with key text saved on the host.
fn use_host_key(env: &FakeEnv, text: Option<&str>) {
    env.sql("UPDATE ssh_host SET auth_method = 'key', key_source = 'content' WHERE id = 'h1'");
    if let Some(text) = text {
        env.set_secret(host_owner(), "private_key", text);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_key_content_logs_in() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &[], true);
    use_host_key(&env, Some(CLIENT_KEY));
    client::connect(env.clone(), &env.target(), None)
        .await
        .map_err(|error| error.message)
        .expect("login with saved key");
    assert_eq!(env.asked(), vec!["hostkey"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn encrypted_key_content_asks_for_the_passphrase_and_keeps_it() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &["nope", ENCRYPTED_PASSPHRASE], true);
    use_host_key(&env, Some(ENCRYPTED_KEY));
    client::connect(env.clone(), &env.target(), None)
        .await
        .map_err(|error| error.message)
        .expect("login with encrypted key");
    assert_eq!(
        env.asked(),
        vec!["hostkey", "Passphrase:", "Passphrase:Sai passphrase"]
    );
    assert_eq!(
        env.secret(host_owner(), "passphrase"),
        Some((SecretStore::Sqlite, ENCRYPTED_PASSPHRASE.to_string()))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn key_content_host_without_a_saved_key_fails_clearly() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &[], true);
    use_host_key(&env, None);
    let error = client::connect(env.clone(), &env.target(), None)
        .await
        .err()
        .expect("no key");
    assert_eq!(error.stage, client::Stage::Auth);
    assert!(error.message.contains("nội dung khóa"), "{}", error.message);
}

#[test]
fn check_private_key_accepts_openssh_keys_only() {
    assert!(client::check_private_key(CLIENT_KEY).is_ok());
    assert!(client::check_private_key(&format!("\n  {ENCRYPTED_KEY}  \n")).is_ok());
    assert!(client::check_private_key("not a key").is_err());
    let ppk = client::check_private_key("PuTTY-User-Key-File-3: ssh-ed25519\n").unwrap_err();
    assert!(ppk.to_string().contains("PuTTY"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identity_supplies_username_and_keeps_the_password() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &[PASSWORD], true);
    // The host's own username is wrong; the identity's wins.
    env.sql(
        "INSERT INTO ssh_identity (id, name, username, created_at) VALUES ('i1', 'ops', 'deploy', '0');
         UPDATE ssh_host SET username = 'nobody', identity_id = 'i1' WHERE id = 'h1';",
    );
    let target = env.target();
    assert_eq!(target.username, "deploy");
    assert_eq!(target.password.owner, SecretOwner::Identity("i1".into()));

    client::connect(env.clone(), &target, None)
        .await
        .map_err(|error| error.message)
        .expect("login");
    assert_eq!(
        env.secret(SecretOwner::Identity("i1".into()), "password"),
        Some((SecretStore::Sqlite, PASSWORD.to_string()))
    );
    assert_eq!(env.secret(host_owner(), "password"), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn keychain_key_via_identity_keeps_its_own_passphrase() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &[ENCRYPTED_PASSPHRASE], true);
    env.sql(
        "INSERT INTO ssh_key (id, name, algorithm, public_key, fingerprint, encrypted, secret_store, created_at)
           VALUES ('k1', 'laptop', 'ssh-ed25519', 'pk', 'fp', 1, 'sqlite', '0');
         INSERT INTO ssh_identity (id, name, username, key_id, created_at)
           VALUES ('i1', 'ops', 'deploy', 'k1', '0');
         UPDATE ssh_host SET auth_method = 'key', identity_id = 'i1' WHERE id = 'h1';",
    );
    let key = SecretOwner::Key("k1".into());
    env.set_secret(key.clone(), "private_key", ENCRYPTED_KEY);

    let target = env.target();
    assert!(matches!(target.key, KeyChoice::Keychain { ref id, .. } if id == "k1"));
    client::connect(env.clone(), &target, None)
        .await
        .map_err(|error| error.message)
        .expect("login with keychain key");
    assert_eq!(env.asked(), vec!["hostkey", "Passphrase:"]);
    assert_eq!(
        env.secret(key, "passphrase"),
        Some((SecretStore::Sqlite, ENCRYPTED_PASSPHRASE.to_string()))
    );

    // A key set on the host itself beats the identity's key.
    env.sql(
        "INSERT INTO ssh_key (id, name, algorithm, public_key, fingerprint, secret_store, created_at)
           VALUES ('k2', 'ci', 'ssh-ed25519', 'pk', 'fp', 'sqlite', '0');
         UPDATE ssh_host SET key_id = 'k2' WHERE id = 'h1';",
    );
    assert!(matches!(env.target().key, KeyChoice::Keychain { ref id, .. } if id == "k2"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_that_hangs_up_during_login_is_reported_as_such() {
    let port = start_server_with(1).await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &["wrong", "again", "more"], true);
    let error = client::connect(env.clone(), &env.target(), None)
        .await
        .err()
        .expect("dropped");
    assert_eq!(error.stage, client::Stage::Auth);
    assert!(error.message.contains("ngắt kết nối"), "{}", error.message);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_runs_one_command_and_reports_its_exit_code() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &[PASSWORD], true);
    let manager = SshManager::new(env.clone());
    let lease = manager.lease("h1").await.expect("lease");

    let (tx, mut rx) = mpsc::channel(16);
    let terminal = SshTerminal::new("ssh:exec".into(), "h1".into(), 64 * 1024, 80, 24);
    let (end_tx, end_rx) = tokio::sync::oneshot::channel();
    terminal
        .start_exec(
            lease,
            IpcPipe::new(tx),
            "cd ~/app && make deploy".into(),
            move |end| {
                let _ = end_tx.send(end);
            },
        )
        .await
        .expect("exec");
    read_until(&mut rx, "ran: cd ~/app && make deploy").await;
    let end = tokio::time::timeout(Duration::from_secs(5), end_rx)
        .await
        .expect("end in time")
        .expect("end sent");
    assert_eq!(end, TerminalEnd::Exited(Some(3)));
}

/// Adds a password host on the test server that goes through `jump`.
fn add_hop(env: &FakeEnv, id: &str, port: u16, jump: Option<&str>) {
    env.db
        .write(|c| {
            let input = ssh_hosts::SshHostInput {
                name: id.into(),
                host: "127.0.0.1".into(),
                port,
                username: "deploy".into(),
                auth_method: "password".into(),
                key_source: "file".into(),
                jump_host_id: jump.map(str::to_string),
                ..Default::default()
            };
            ssh_hosts::insert(c, id, &input, "0")
        })
        .expect("host");
    env.set_secret(SecretOwner::Host(id.into()), "password", PASSWORD);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_jump_hosts_in_a_row_share_their_connections() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &[], true);
    env.set_secret(host_owner(), "password", PASSWORD);
    add_hop(&env, "j1", port, None);
    add_hop(&env, "j2", port, Some("j1"));
    env.sql("UPDATE ssh_host SET jump_host_id = 'j2' WHERE id = 'h1'");

    let manager = SshManager::new(env.clone());
    let lease = tokio::time::timeout(Duration::from_secs(10), manager.lease("h1"))
        .await
        .expect("in time")
        .expect("lease");
    let open = manager.open_channels();
    assert_eq!(open.get("h1"), Some(&1));
    assert_eq!(open.get("j2"), Some(&1));
    assert_eq!(open.get("j1"), Some(&1));

    // The tunnel carries a working session.
    let (tx, mut rx) = mpsc::channel(16);
    let terminal = SshTerminal::new("ssh:deep".into(), "h1".into(), 64 * 1024, 80, 24);
    terminal
        .start(lease, IpcPipe::new(tx), |_| {})
        .await
        .expect("start");
    read_until(&mut rx, "welcome").await;

    // j2 is reused, not logged into again.
    let _j2 = manager.lease("j2").await.expect("lease j2");
    assert_eq!(manager.open_channels().get("j2"), Some(&2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_jump_host_loop_fails_instead_of_hanging() {
    let port = start_server().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let env = env(dir.path(), port, &[], true);
    add_hop(&env, "j1", port, Some("h1"));
    // Rows the editor would refuse, e.g. from an old backup.
    env.sql("UPDATE ssh_host SET jump_host_id = 'j1' WHERE id = 'h1'");

    let manager = SshManager::new(env.clone());
    let error = tokio::time::timeout(Duration::from_secs(5), manager.lease("h1"))
        .await
        .expect("no deadlock")
        .err()
        .expect("refused");
    assert!(error.to_string().contains("vòng lặp"), "{error}");
}
