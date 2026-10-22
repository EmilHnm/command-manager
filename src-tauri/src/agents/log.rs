//! The read-only terminal tab of one agent process ("🤖 codex · api-cart").
//! The stream-json is turned into readable lines; the tab attaches through
//! the process manager like any terminal.

use super::adapter::LogLine;
use crate::error::{Error, Result};
use crate::pty::backpressure::IpcPipe;
use crate::pty::buffer::RingBuffer;
use crate::pty::terminal_io::TerminalIo;
use std::sync::Mutex;
use tokio::sync::{mpsc, watch};

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const CYAN: &str = "\x1b[36m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

pub struct AgentLog {
    id: String,
    buffer: Mutex<RingBuffer>,
    /// To the UI, in order: a forwarder task waits on the IPC pipe so the
    /// agent never does.
    forward: Option<mpsc::UnboundedSender<Vec<u8>>>,
    /// "Dừng" in the tab ends the process.
    cancel: watch::Sender<bool>,
}

impl AgentLog {
    /// With a pipe, call from inside the tokio runtime (it spawns the
    /// forwarder).
    pub fn new(id: String, capacity: usize, pipe: Option<IpcPipe>) -> Self {
        let (cancel, _) = watch::channel(false);
        let forward = pipe.map(|pipe| {
            let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
            let id = id.clone();
            tokio::spawn(async move {
                while let Some(bytes) = rx.recv().await {
                    pipe.send(id.clone(), bytes).await;
                }
            });
            tx
        });
        Self {
            id,
            buffer: Mutex::new(RingBuffer::new(capacity)),
            forward,
            cancel,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn snapshot_bytes(&self) -> Vec<u8> {
        self.buffer.lock().expect("agent log lock").snapshot()
    }

    pub fn cancelled(&self) -> watch::Receiver<bool> {
        self.cancel.subscribe()
    }

    fn emit(&self, text: String) {
        // A terminal wants CRLF.
        let bytes = text.replace('\n', "\r\n").into_bytes();
        self.buffer.lock().expect("agent log lock").push(&bytes);
        if let Some(forward) = &self.forward {
            let _ = forward.send(bytes);
        }
    }

    pub fn line(&self, line: &LogLine) {
        let text = match line {
            LogLine::Text(text) => format!("{text}\n"),
            LogLine::Tool(name, detail) if detail.is_empty() => {
                format!("{CYAN}▸ {name}{RESET}\n")
            }
            LogLine::Tool(name, detail) => format!("{CYAN}▸ {name}{RESET} {DIM}{detail}{RESET}\n"),
            LogLine::ToolError(text) => format!("{YELLOW}  ✗ {text}{RESET}\n"),
            LogLine::Info(text) => format!("{DIM}{text}{RESET}\n"),
            LogLine::Error(text) => format!("{RED}✗ {text}{RESET}\n"),
        };
        self.emit(text);
    }

    pub fn heading(&self, text: &str) {
        self.emit(format!("{BOLD}{text}{RESET}\n"));
    }

    pub fn info(&self, text: &str) {
        self.line(&LogLine::Info(text.to_string()));
    }

    pub fn raw(&self, text: &str) {
        self.emit(text.to_string());
    }

    pub fn result(&self, ok: bool, text: &str) {
        let color = if ok { GREEN } else { RED };
        let mark = if ok { "✓" } else { "✗" };
        self.emit(format!("{color}{BOLD}{mark} {text}{RESET}\n"));
    }
}

impl TerminalIo for AgentLog {
    fn write(&self, _bytes: &[u8]) -> Result<()> {
        Err(Error::msg("Tab log agent chỉ để xem"))
    }

    fn resize(&self, _cols: u16, _rows: u16) -> Result<()> {
        Ok(())
    }

    fn snapshot(&self) -> Result<Vec<u8>> {
        Ok(self.buffer.lock().expect("agent log lock").snapshot())
    }

    fn buffer_len(&self) -> usize {
        self.buffer.lock().expect("agent log lock").len()
    }

    fn is_remote(&self) -> bool {
        // No PID to signal: stopping goes through `stop`.
        true
    }

    fn stop(&self, _force: bool) -> bool {
        self.cancel.send_replace(true);
        true
    }
}
