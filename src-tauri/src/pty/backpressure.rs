//! Default policy: never block the PTY reader on IPC.
//! Ring buffer always records; overflowed IPC chunks are dropped (UI reattaches from buffer).

use tokio::sync::mpsc::Sender;

pub const IPC_CHUNK: usize = 8 * 1024;

#[derive(Clone)]
pub struct IpcPipe {
    tx: Sender<(String, Vec<u8>)>,
}

impl IpcPipe {
    pub fn new(tx: Sender<(String, Vec<u8>)>) -> Self {
        Self { tx }
    }

    /// Drop-oldest-equivalent: `try_send` fails when the UI is slow; bytes stay in the ring.
    pub fn push(&self, run_event_id: String, bytes: Vec<u8>) {
        let _ = self.tx.try_send((run_event_id, bytes));
    }
}
