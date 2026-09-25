use crate::error::Result;
use crate::pty::backpressure::IpcPipe;
use crate::pty::session::{self, PtySession};
use std::sync::Arc;

pub struct Spawned {
    pub pid: u32,
    pub pty: Arc<PtySession>,
}

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
    // ponytail: portable-pty 0.8 has no pre_exec hook; PR_SET_PDEATHSIG lives in platform::unix
    // and should be wired when we spawn via std::process::Command on Linux.
    let pid = child.process_id().unwrap_or(0);
    let reader = {
        let master = pty.master.lock().map_err(|_| crate::error::Error::msg("pty lock"))?;
        master
            .try_clone_reader()
            .map_err(|e| crate::error::Error::msg(e.to_string()))?
    };
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
    })
}
