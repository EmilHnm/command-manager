//! Backpressure policy: pause the PTY reader instead of dropping output.
//!
//! A terminal stream cannot lose bytes: a TUI (Claude Code, htop) redraws by
//! moving the cursor relative to what it drew before, so one dropped chunk
//! leaves xterm out of step with the program until it repaints the whole
//! screen. When the UI falls behind, `push` blocks the reader thread; the
//! child then blocks on its own writes, as with a real terminal. The emitter
//! (`lib.rs`) merges whatever has queued up per terminal into one event, so a
//! slow webview costs fewer, larger events rather than lost output.

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

    /// Waits for room in the channel. Must be called from a plain thread (the
    /// PTY reader), never from inside the tokio runtime. Returns without
    /// sending once the emitter is gone (app shutting down).
    pub fn push(&self, run_event_id: String, bytes: Vec<u8>) {
        let _ = self.tx.blocking_send((run_event_id, bytes));
    }
}

/// Queued chunks merged per terminal, keeping each terminal's byte order.
pub fn coalesce(chunks: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
    let mut merged: Vec<(String, Vec<u8>)> = Vec::new();
    for (id, bytes) in chunks {
        match merged.iter_mut().find(|(existing, _)| *existing == id) {
            Some((_, data)) => data.extend_from_slice(&bytes),
            None => merged.push((id, bytes)),
        }
    }
    merged
}

/// A sender whose receiver is drained on a background thread, for tests
/// that pump a PTY without reading the IPC side.
#[cfg(test)]
pub fn drained_sender() -> Sender<(String, Vec<u8>)> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    std::thread::spawn(move || while rx.blocking_recv().is_some() {});
    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_slow_receiver_gets_every_chunk_in_order() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        let pipe = IpcPipe::new(tx);
        let writer = std::thread::spawn(move || {
            for i in 0..50u8 {
                pipe.push("t".into(), vec![i]);
            }
        });
        let mut received = Vec::new();
        while received.len() < 50 {
            std::thread::sleep(Duration::from_millis(1));
            let (_, bytes) = rx.blocking_recv().expect("chunk");
            received.extend(bytes);
        }
        writer.join().expect("writer");
        assert_eq!(received, (0..50u8).collect::<Vec<_>>());
    }

    #[test]
    fn push_returns_once_the_receiver_is_gone() {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        drop(rx);
        IpcPipe::new(tx).push("t".into(), vec![1]);
    }

    #[test]
    fn coalesce_merges_per_terminal_and_keeps_order() {
        let merged = coalesce(vec![
            ("a".into(), b"1".to_vec()),
            ("b".into(), b"x".to_vec()),
            ("a".into(), b"2".to_vec()),
            ("b".into(), b"y".to_vec()),
            ("a".into(), b"3".to_vec()),
        ]);
        assert_eq!(
            merged,
            vec![("a".into(), b"123".to_vec()), ("b".into(), b"xy".to_vec())]
        );
    }
}
