//! What the IPC layer needs from a live terminal, local or remote.
//!
//! A local PTY has a child PID, so stopping it goes through the OS signal path
//! in `process::shutdown`. A remote (SSH) terminal has no local PID and stops
//! itself through [`TerminalIo::stop`].

use super::session::PtySession;
use crate::error::Result;

pub trait TerminalIo: Send + Sync {
    fn write(&self, bytes: &[u8]) -> Result<()>;
    fn resize(&self, cols: u16, rows: u16) -> Result<()>;
    /// Recent output for reattaching a tab.
    fn snapshot(&self) -> Result<Vec<u8>>;
    fn buffer_len(&self) -> usize;

    /// True when the terminal has no local process and must be stopped
    /// through [`TerminalIo::stop`] instead of by PID.
    fn is_remote(&self) -> bool {
        false
    }

    /// Asks a remote terminal to end. Returns false when there is nothing
    /// left to stop (already disconnected), so the caller cleans up itself.
    fn stop(&self, _force: bool) -> bool {
        false
    }
}

impl TerminalIo for PtySession {
    fn write(&self, bytes: &[u8]) -> Result<()> {
        PtySession::write(self, bytes)
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        PtySession::resize(self, cols, rows)
    }

    fn snapshot(&self) -> Result<Vec<u8>> {
        PtySession::snapshot(self)
    }

    fn buffer_len(&self) -> usize {
        PtySession::buffer_len(self)
    }
}
