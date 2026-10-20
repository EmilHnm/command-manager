//! A remote shell on one SSH channel, exposed as a [`TerminalIo`].
//!
//! Output goes through the same ring buffer and IPC pipe as a local PTY, so
//! reattach, backpressure and the xterm side work unchanged. Input is queued
//! to the channel task in order; the IPC thread never waits on the network.
//!
//! When the connection drops the terminal stays registered (status
//! `disconnected`) with its output kept. [`SshTerminal::start`] can then run
//! again on a new channel: that is a new shell, not the old one resumed.

use super::manager::Lease;
use crate::error::{Error, Result};
use crate::pty::backpressure::IpcPipe;
use crate::pty::buffer::RingBuffer;
use crate::pty::terminal_io::TerminalIo;
use russh::{ChannelMsg, Sig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

const GRACEFUL_STOP: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalEnd {
    /// The remote shell exited; `None` when it died from a signal.
    Exited(Option<i32>),
    /// Stopped from the app (MOD-01 or app exit).
    Stopped,
    /// The connection went away without the shell exiting.
    Lost,
}

enum Input {
    Data(Vec<u8>),
    Resize(u16, u16),
    Stop { force: bool },
}

pub struct SshTerminal {
    run_event_id: String,
    host_id: String,
    buffer: Arc<Mutex<RingBuffer>>,
    input: Mutex<Option<mpsc::UnboundedSender<Input>>>,
    size: Mutex<(u16, u16)>,
    /// Set while [`SshTerminal::start`] runs, so two reconnects cannot
    /// race and leave two shells writing into one tab.
    starting: AtomicBool,
}

impl SshTerminal {
    pub fn new(
        run_event_id: String,
        host_id: String,
        buffer_bytes: usize,
        cols: u16,
        rows: u16,
    ) -> Arc<Self> {
        Arc::new(Self {
            run_event_id,
            host_id,
            buffer: Arc::new(Mutex::new(RingBuffer::new(buffer_bytes))),
            input: Mutex::new(None),
            size: Mutex::new((cols.max(1), rows.max(1))),
            starting: AtomicBool::new(false),
        })
    }

    pub fn host_id(&self) -> &str {
        &self.host_id
    }

    pub fn is_connected(&self) -> bool {
        self.input
            .lock()
            .map(|input| input.is_some())
            .unwrap_or(false)
    }

    /// Opens a shell channel on `lease` and pumps it until it ends, then
    /// calls `on_end`. The lease is released with the channel.
    pub async fn start(
        self: &Arc<Self>,
        lease: Lease,
        ipc: IpcPipe,
        on_end: impl FnOnce(TerminalEnd) + Send + 'static,
    ) -> Result<()> {
        if self.is_connected() || self.starting.swap(true, Ordering::SeqCst) {
            return Err(Error::msg("Terminal SSH đang kết nối"));
        }
        let result = self.open(lease, ipc, None, on_end).await;
        self.starting.store(false, Ordering::SeqCst);
        result
    }

    /// Runs one command (a saved command bound to this host) on a PTY. The
    /// server's login shell interprets it, as `ssh host 'command'` does; the
    /// exit status ends the terminal like a local process.
    pub async fn start_exec(
        self: &Arc<Self>,
        lease: Lease,
        ipc: IpcPipe,
        command: String,
        on_end: impl FnOnce(TerminalEnd) + Send + 'static,
    ) -> Result<()> {
        if self.is_connected() || self.starting.swap(true, Ordering::SeqCst) {
            return Err(Error::msg("Terminal SSH đang kết nối"));
        }
        let result = self.open(lease, ipc, Some(command), on_end).await;
        self.starting.store(false, Ordering::SeqCst);
        result
    }

    async fn open(
        self: &Arc<Self>,
        lease: Lease,
        ipc: IpcPipe,
        command: Option<String>,
        on_end: impl FnOnce(TerminalEnd) + Send + 'static,
    ) -> Result<()> {
        let channel = lease
            .handle()
            .channel_open_session()
            .await
            .map_err(|error| Error::msg(format!("Không mở được phiên SSH: {error}")))?;
        let (cols, rows) = *self.size.lock().map_err(|_| Error::msg("ssh size lock"))?;
        channel
            .request_pty(false, "xterm-256color", cols.into(), rows.into(), 0, 0, &[])
            .await
            .map_err(|error| Error::msg(format!("Máy chủ từ chối cấp PTY: {error}")))?;
        match command {
            Some(command) => channel
                .exec(true, command)
                .await
                .map_err(|error| Error::msg(format!("Máy chủ từ chối chạy lệnh: {error}")))?,
            None => channel
                .request_shell(false)
                .await
                .map_err(|error| Error::msg(format!("Máy chủ từ chối mở shell: {error}")))?,
        }

        let (tx, rx) = mpsc::unbounded_channel();
        *self
            .input
            .lock()
            .map_err(|_| Error::msg("ssh input lock"))? = Some(tx);
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let end = this.pump(channel, rx, &ipc).await;
            drop(lease);
            if let Ok(mut input) = this.input.lock() {
                *input = None;
            }
            if end == TerminalEnd::Lost {
                this.notice(&ipc, "Mất kết nối SSH").await;
            }
            on_end(end);
        });
        Ok(())
    }

    /// Writes a dim marker line into the output, e.g. around a reconnect.
    pub async fn notice(&self, ipc: &IpcPipe, text: &str) {
        let line = format!("\r\n\x1b[2m──── {text} ────\x1b[0m\r\n").into_bytes();
        self.record(ipc, line).await;
    }

    async fn record(&self, ipc: &IpcPipe, bytes: Vec<u8>) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push(&bytes);
        }
        ipc.send(self.run_event_id.clone(), bytes).await;
    }

    async fn pump(
        &self,
        mut channel: russh::Channel<russh::client::Msg>,
        mut input: mpsc::UnboundedReceiver<Input>,
        ipc: &IpcPipe,
    ) -> TerminalEnd {
        let mut exit: Option<Option<i32>> = None;
        let mut stopping = false;
        let mut deadline: Option<Instant> = None;
        let mut input_open = true;

        loop {
            tokio::select! {
                message = channel.wait() => match message {
                    Some(ChannelMsg::Data { data })
                    | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        self.record(ipc, data.to_vec()).await;
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        exit = Some(i32::try_from(exit_status).ok());
                    }
                    Some(ChannelMsg::ExitSignal { .. }) => exit = Some(None),
                    Some(ChannelMsg::Close) | None => break,
                    Some(_) => {}
                },
                command = input.recv(), if input_open => match command {
                    Some(Input::Data(bytes)) => {
                        if channel.data_bytes(bytes).await.is_err() {
                            break;
                        }
                    }
                    Some(Input::Resize(cols, rows)) => {
                        let _ = channel.window_change(cols.into(), rows.into(), 0, 0).await;
                    }
                    Some(Input::Stop { force }) => {
                        stopping = true;
                        if force {
                            let _ = channel.close().await;
                            break;
                        }
                        // sshd hangs up the shell when the channel closes;
                        // the signal is a polite first try (some servers
                        // ignore it).
                        let _ = channel.signal(Sig::HUP).await;
                        let _ = channel.eof().await;
                        deadline.get_or_insert_with(|| Instant::now() + GRACEFUL_STOP);
                    }
                    None => input_open = false,
                },
                _ = sleep_until(deadline), if deadline.is_some() => {
                    let _ = channel.close().await;
                    break;
                }
            }
        }

        if stopping {
            TerminalEnd::Stopped
        } else if let Some(code) = exit {
            TerminalEnd::Exited(code)
        } else {
            TerminalEnd::Lost
        }
    }

    fn send(&self, command: Input) -> bool {
        self.input
            .lock()
            .ok()
            .and_then(|input| input.as_ref().map(|tx| tx.send(command).is_ok()))
            .unwrap_or(false)
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

impl TerminalIo for SshTerminal {
    fn write(&self, bytes: &[u8]) -> Result<()> {
        if self.send(Input::Data(bytes.to_vec())) {
            Ok(())
        } else {
            Err(Error::msg("Mất kết nối SSH"))
        }
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let (cols, rows) = (cols.max(1), rows.max(1));
        if let Ok(mut size) = self.size.lock() {
            *size = (cols, rows);
        }
        // While disconnected the size is kept for the next shell.
        self.send(Input::Resize(cols, rows));
        Ok(())
    }

    fn snapshot(&self) -> Result<Vec<u8>> {
        let buffer = self.buffer.lock().map_err(|_| Error::msg("buffer lock"))?;
        Ok(buffer.snapshot())
    }

    fn buffer_len(&self) -> usize {
        self.buffer.lock().map(|b| b.len()).unwrap_or(0)
    }

    fn is_remote(&self) -> bool {
        true
    }

    fn stop(&self, force: bool) -> bool {
        self.send(Input::Stop { force })
    }
}
