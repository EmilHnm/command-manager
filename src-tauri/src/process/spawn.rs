use crate::error::Result;
use crate::pty::backpressure::IpcPipe;
use crate::pty::osc::{ShellHistoryRecord, ShellTracker};
use crate::pty::session::{self, PtySession};
use portable_pty::ChildKiller;
use std::sync::Arc;

pub struct Spawned {
    pub pid: u32,
    pub pty: Arc<PtySession>,
    pub shell_kind: String,
    pub history_level: u8,
    /// A handle tied to this exact child. Cleanup must not fall back to a
    /// recycled PID after the child has already exited.
    pub killer: Box<dyn ChildKiller + Send + Sync>,
}

#[allow(clippy::too_many_arguments)]
pub fn spawn(
    is_shell: bool,
    execution_string: &str,
    cols: u16,
    rows: u16,
    buffer_bytes: usize,
    run_event_id: String,
    ipc: IpcPipe,
    on_exit: impl FnOnce(Option<i32>) + Send + 'static,
) -> Result<Spawned> {
    let (pty, slave) = PtySession::open(cols, rows, buffer_bytes)?;
    let mut child = session::spawn_on_slave(slave, is_shell, execution_string)?;
    let killer = child.clone_killer();
    // ponytail: portable-pty 0.8 has no pre_exec hook; PR_SET_PDEATHSIG lives in platform::unix
    // and should be wired when we spawn via std::process::Command on Linux.
    let pid = child.process_id().unwrap_or(0);
    let reader = pty.clone_reader()?;
    let buffer = Arc::clone(&pty.buffer);
    std::thread::spawn(move || {
        session::pump_reader(reader, buffer, run_event_id, ipc);
    });

    std::thread::spawn(move || {
        let code = match child.wait() {
            Ok(st) => Some(st.exit_code() as i32),
            Err(_) => None,
        };
        on_exit(code);
    });

    Ok(Spawned {
        pid,
        pty: Arc::new(pty),
        shell_kind: "command".into(),
        history_level: 0,
        killer,
    })
}

pub fn spawn_argv(
    argv: &[String],
    cols: u16,
    rows: u16,
    buffer_bytes: usize,
    run_event_id: String,
    ipc: IpcPipe,
    on_exit: impl FnOnce(Option<i32>) + Send + 'static,
) -> Result<Spawned> {
    let (pty, slave) = PtySession::open(cols, rows, buffer_bytes)?;
    let mut child = session::spawn_argv_on_slave(slave, argv)?;
    let killer = child.clone_killer();
    let pid = child.process_id().unwrap_or(0);
    let reader = pty.clone_reader()?;
    let buffer = Arc::clone(&pty.buffer);
    std::thread::spawn(move || session::pump_reader(reader, buffer, run_event_id, ipc));
    std::thread::spawn(move || {
        let code = match child.wait() {
            Ok(st) => Some(st.exit_code() as i32),
            Err(_) => None,
        };
        on_exit(code);
    });
    Ok(Spawned {
        pid,
        pty: Arc::new(pty),
        shell_kind: "command".into(),
        history_level: 0,
        killer,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_interactive(
    cols: u16,
    rows: u16,
    buffer_bytes: usize,
    run_event_id: String,
    ipc: IpcPipe,
    integration_root: Option<std::path::PathBuf>,
    preferred_shell: Option<String>,
    load_powershell_profile: bool,
    on_exit: impl FnOnce(Option<i32>) + Send + 'static,
    on_history: impl FnMut(ShellHistoryRecord) + Send + 'static,
) -> Result<Spawned> {
    let (pty, slave) = PtySession::open(cols, rows, buffer_bytes)?;
    let metadata = session::spawn_interactive_with_metadata(
        slave,
        integration_root,
        preferred_shell,
        load_powershell_profile,
    )?;
    let mut child = metadata.child;
    let killer = child.clone_killer();
    let pid = child.process_id().unwrap_or(0);
    let reader = pty.clone_reader()?;
    let buffer = Arc::clone(&pty.buffer);
    let tracker = metadata.nonce.clone().map(|nonce| {
        ShellTracker::new(nonce, metadata.shell_kind.as_str().to_string(), on_history)
    });
    std::thread::spawn(move || {
        session::pump_reader_with_tracker(reader, buffer, run_event_id, ipc, tracker);
    });

    std::thread::spawn(move || {
        let code = match child.wait() {
            Ok(st) => Some(st.exit_code() as i32),
            Err(_) => None,
        };
        on_exit(code);
    });

    Ok(Spawned {
        pid,
        pty: Arc::new(pty),
        shell_kind: metadata.shell_kind.as_str().to_string(),
        history_level: metadata.history_level,
        killer,
    })
}
