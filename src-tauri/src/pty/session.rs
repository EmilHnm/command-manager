use super::backpressure::IpcPipe;
use super::buffer::RingBuffer;
use crate::error::{Error, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub struct PtySession {
    pub master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    pub buffer: Arc<Mutex<RingBuffer>>,
}

impl PtySession {
    pub fn open(cols: u16, rows: u16, buffer_bytes: usize) -> Result<(Self, Box<dyn portable_pty::SlavePty + Send>)> {
        let sys = native_pty_system();
        let pair = sys
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::msg(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| Error::msg(e.to_string()))?;
        Ok((
            Self {
                master: Mutex::new(pair.master),
                writer: Mutex::new(writer),
                buffer: Arc::new(Mutex::new(RingBuffer::new(buffer_bytes))),
            },
            pair.slave,
        ))
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let m = self.master.lock().map_err(|_| Error::msg("pty lock"))?;
        m.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| Error::msg(e.to_string()))
    }

    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        let mut w = self.writer.lock().map_err(|_| Error::msg("pty writer lock"))?;
        w.write_all(bytes)?;
        w.flush()?;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<Vec<u8>> {
        let b = self.buffer.lock().map_err(|_| Error::msg("buffer lock"))?;
        Ok(b.snapshot())
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.lock().map(|b| b.len()).unwrap_or(0)
    }
}

pub fn spawn_on_slave(
    slave: Box<dyn portable_pty::SlavePty + Send>,
    is_shell: bool,
    execution_string: &str,
) -> Result<Box<dyn portable_pty::Child + Send + Sync>> {
    let cmd = build_command(is_shell, execution_string)?;
    slave
        .spawn_command(cmd)
        .map_err(|e| Error::msg(e.to_string()))
}

pub fn build_command(is_shell: bool, execution_string: &str) -> Result<CommandBuilder> {
    if is_shell {
        let mut cmd = CommandBuilder::new("sh");
        cmd.arg("-c");
        cmd.arg(execution_string);
        Ok(cmd)
    } else {
        let words = shell_words::split(execution_string).map_err(|e| Error::Argv(e.to_string()))?;
        if words.is_empty() {
            return Err(Error::Argv("empty command".into()));
        }
        let mut cmd = CommandBuilder::new(&words[0]);
        if words.len() > 1 {
            cmd.args(&words[1..]);
        }
        Ok(cmd)
    }
}

pub fn pump_reader(
    mut reader: Box<dyn Read + Send>,
    buffer: Arc<Mutex<RingBuffer>>,
    run_event_id: String,
    ipc: IpcPipe,
) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = &buf[..n];
                if let Ok(mut b) = buffer.lock() {
                    b.push(chunk);
                }
                ipc.push(run_event_id.clone(), chunk.to_vec());
            }
            Err(_) => break,
        }
    }
}
