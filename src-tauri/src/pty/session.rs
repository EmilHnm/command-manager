use super::backpressure::IpcPipe;
use super::buffer::RingBuffer;
use super::osc::ShellTracker;
use super::shell_integration::{self, InteractiveShell};
use crate::argv::split_argv;
use crate::error::{Error, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

pub struct PtySession {
    pub master: Mutex<Box<dyn MasterPty + Send>>,
    writer: SharedWriter,
    pub buffer: Arc<Mutex<RingBuffer>>,
}

impl PtySession {
    pub fn open(
        cols: u16,
        rows: u16,
        buffer_bytes: usize,
    ) -> Result<(Self, Box<dyn portable_pty::SlavePty + Send>)> {
        prepare_conpty_sideload();
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
                writer: Arc::new(Mutex::new(writer)),
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
        let mut w = self
            .writer
            .lock()
            .map_err(|_| Error::msg("pty writer lock"))?;
        w.write_all(bytes)?;
        w.flush()?;
        Ok(())
    }

    /// Reader for the output pump. On Windows it answers the pinned ConPTY's
    /// startup handshake; see [`StartupQueryFilter`].
    pub fn clone_reader(&self) -> Result<Box<dyn Read + Send>> {
        let reader = self
            .master
            .lock()
            .map_err(|_| Error::msg("pty lock"))?
            .try_clone_reader()
            .map_err(|e| Error::msg(e.to_string()))?;
        #[cfg(windows)]
        {
            Ok(Box::new(StartupQueryFilter::new(
                reader,
                Arc::clone(&self.writer),
            )))
        }
        #[cfg(not(windows))]
        {
            Ok(reader)
        }
    }

    pub fn snapshot(&self) -> Result<Vec<u8>> {
        let b = self.buffer.lock().map_err(|_| Error::msg("buffer lock"))?;
        Ok(b.snapshot())
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.lock().map(|b| b.len()).unwrap_or(0)
    }
}

/// The pinned ConPTY (OpenConsole) sends a Primary Device Attributes query
/// (`ESC [ c`) as soon as it starts and holds the client for about three
/// seconds waiting for the terminal's answer. No webview is attached yet at
/// that point, so the backend answers it and drops the query from the stream;
/// otherwise xterm.js would answer again, live or when the buffer is replayed
/// on reattach, and the late reply would be typed into the shell.
#[cfg_attr(not(windows), allow(dead_code))]
struct StartupQueryFilter<R> {
    inner: R,
    writer: SharedWriter,
    scanned: usize,
    done: bool,
    carry: Vec<u8>,
    pending: Vec<u8>,
}

#[cfg_attr(not(windows), allow(dead_code))]
const DA1_QUERY: &[u8] = b"[c";
/// Same Primary Device Attributes answer as xterm.js.
#[cfg_attr(not(windows), allow(dead_code))]
const DA1_REPLY: &[u8] = b"[?1;2c";
/// The handshake is the first thing ConPTY writes; never touch later output,
/// where a DA1 query belongs to a program and must reach xterm.js.
#[cfg_attr(not(windows), allow(dead_code))]
const STARTUP_WINDOW: usize = 512;

#[cfg_attr(not(windows), allow(dead_code))]
impl<R> StartupQueryFilter<R> {
    fn new(inner: R, writer: SharedWriter) -> Self {
        Self {
            inner,
            writer,
            scanned: 0,
            done: false,
            carry: Vec::new(),
            pending: Vec::new(),
        }
    }

    fn filter(&mut self, mut data: Vec<u8>) -> Vec<u8> {
        if let Some(index) = data
            .windows(DA1_QUERY.len())
            .position(|window| window == DA1_QUERY)
            .filter(|index| self.scanned + index < STARTUP_WINDOW)
        {
            if let Ok(mut writer) = self.writer.lock() {
                let _ = writer.write_all(DA1_REPLY).and_then(|()| writer.flush());
            }
            data.drain(index..index + DA1_QUERY.len());
            self.done = true;
            return data;
        }
        self.scanned += data.len();
        if self.scanned >= STARTUP_WINDOW {
            self.done = true;
            return data;
        }
        // Hold back a partial query split across two reads.
        let keep = (1..DA1_QUERY.len())
            .rev()
            .find(|&len| data.ends_with(&DA1_QUERY[..len]))
            .unwrap_or(0);
        self.scanned -= keep;
        self.carry = data.split_off(data.len() - keep);
        data
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
impl<R: Read> Read for StartupQueryFilter<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.pending.is_empty() {
            if self.done && self.carry.is_empty() {
                return self.inner.read(buf);
            }
            let mut chunk = [0u8; 8192];
            let n = self.inner.read(&mut chunk)?;
            let mut data = std::mem::take(&mut self.carry);
            if n == 0 {
                self.done = true;
                if data.is_empty() {
                    return Ok(0);
                }
                self.pending = data;
                break;
            }
            data.extend_from_slice(&chunk[..n]);
            self.pending = if self.done { data } else { self.filter(data) };
        }
        let n = buf.len().min(self.pending.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }
}

#[cfg(windows)]
fn conpty_sideload_directory() -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.to_path_buf());
            candidates.push(parent.join("resources").join("conpty").join("win-x64"));
            // Unit tests run from target/debug/deps while the build script
            // copies the pinned DLL beside the debug executable.
            if let Some(profile_dir) = parent.parent() {
                candidates.push(profile_dir.to_path_buf());
                candidates.push(profile_dir.join("resources").join("conpty").join("win-x64"));
            }
        }
    }
    candidates.push(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/conpty/win-x64"),
    );
    candidates.push(
        std::env::current_dir()
            .unwrap_or_default()
            .join("src-tauri/resources/conpty/win-x64"),
    );
    candidates.into_iter().find(|directory| {
        directory.join("conpty.dll").is_file() && directory.join("OpenConsole.exe").is_file()
    })
}

#[cfg(windows)]
fn prepare_conpty_sideload() {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetDllDirectoryW(path: *const u16) -> i32;
    }

    if let Some(directory) = conpty_sideload_directory() {
        let mut wide: Vec<u16> = directory.as_os_str().encode_wide().collect();
        wide.push(0);
        // portable-pty loads its ConPTY function table during
        // native_pty_system(), so the DLL directory must be set first.
        unsafe {
            SetDllDirectoryW(wide.as_ptr());
        }
    } else {
        eprintln!(
            "Command Manager: pinned ConPTY resources are missing; falling back to Windows ConPTY"
        );
    }
}

#[cfg(not(windows))]
fn prepare_conpty_sideload() {}

/// Describe the xterm.js front end to the child. An app started from a
/// desktop launcher has no TERM of its own, and zsh/ZLE without one cannot
/// clear to end of line, so every keystroke redraw leaves the old text behind.
/// An inherited TERM (tmux, kitty, ...) would describe the wrong terminal.
pub(crate) fn set_terminal_env(cmd: &mut CommandBuilder) {
    // CommandBuilder::new() already captures the complete parent environment
    // and, on Windows, merges the current User/Machine environment from the
    // registry. Keep this layer generic: tool-specific variables must not be
    // copied here one by one because new tools would otherwise require code
    // changes.
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    #[cfg(windows)]
    preserve_inherited_path(cmd);
}

#[cfg(windows)]
fn preserve_inherited_path(cmd: &mut CommandBuilder) {
    use std::env;

    // portable-pty intentionally rebuilds PATH from the Windows environment
    // registry. A desktop-launched app can nevertheless be opened from a
    // process whose PATH contains session-local entries (for example a tool
    // manager activated by a dev shell). Keep both sources, without knowing
    // which tools own the entries.
    let Some(inherited) = env::var_os("PATH") else {
        return;
    };
    let Some(builder_path) = cmd.get_env("PATH").map(std::ffi::OsStr::to_owned) else {
        return;
    };

    let mut paths: Vec<_> = env::split_paths(&inherited).collect();
    for path in env::split_paths(&builder_path) {
        if !paths.iter().any(|existing| {
            existing
                .to_string_lossy()
                .eq_ignore_ascii_case(&path.to_string_lossy())
        }) {
            paths.push(path);
        }
    }

    if let Ok(path) = env::join_paths(paths) {
        cmd.env("PATH", path);
    }
}

pub fn spawn_on_slave(
    slave: Box<dyn portable_pty::SlavePty + Send>,
    is_shell: bool,
    execution_string: &str,
) -> Result<Box<dyn portable_pty::Child + Send + Sync>> {
    let mut cmd = build_command(is_shell, execution_string)?;
    set_terminal_env(&mut cmd);
    slave
        .spawn_command(cmd)
        .map_err(|e| Error::msg(e.to_string()))
}

/// Spawn a saved shell command using the shell that produced its history.
/// This prevents a PowerShell command saved from HistoryView from being
/// replayed through `cmd.exe` on Windows.
pub fn spawn_shell_on_slave(
    slave: Box<dyn portable_pty::SlavePty + Send>,
    shell_kind: Option<&str>,
    execution_string: &str,
) -> Result<Box<dyn portable_pty::Child + Send + Sync>> {
    let mut cmd = build_shell_command(shell_kind, execution_string)?;
    set_terminal_env(&mut cmd);
    match slave.spawn_command(cmd) {
        Ok(child) => Ok(child),
        Err(first_error) if cfg!(windows) && shell_kind == Some("pwsh") => {
            // Microsoft Store/App Execution Alias entries can be visible on
            // PATH while CreateProcessW rejects them. Retry with Windows
            // PowerShell so a saved pwsh command remains runnable.
            let mut fallback = build_shell_command(Some("powershell"), execution_string)?;
            set_terminal_env(&mut fallback);
            fallback.env(
                "COMMAND_MANAGER_SHELL_FALLBACK_WARNING",
                "[Command Manager] pwsh không khả dụng; đang chạy bằng Windows PowerShell 5.1. Một số cú pháp pwsh 7 có thể không tương thích.",
            );
            slave.spawn_command(fallback).map_err(|fallback_error| {
                Error::msg(format!(
                    "PowerShell spawn failed ({first_error}); fallback failed ({fallback_error})"
                ))
            })
        }
        Err(error) => Err(Error::msg(error.to_string())),
    }
}

pub fn spawn_argv_on_slave(
    slave: Box<dyn portable_pty::SlavePty + Send>,
    argv: &[String],
) -> Result<Box<dyn portable_pty::Child + Send + Sync>> {
    if argv.is_empty() {
        return Err(Error::Argv("empty command".into()));
    }
    let mut cmd = CommandBuilder::from_argv(argv.iter().map(Into::into).collect());
    set_terminal_env(&mut cmd);
    slave
        .spawn_command(cmd)
        .map_err(|e| Error::msg(e.to_string()))
}

/// Spawn the user's interactive shell for a standalone terminal tab.
/// Unlike a saved command, this process must remain attached to stdin and
/// provide a normal shell prompt until the user exits it.
pub fn spawn_interactive_on_slave(
    slave: Box<dyn portable_pty::SlavePty + Send>,
) -> Result<Box<dyn portable_pty::Child + Send + Sync>> {
    Ok(spawn_interactive_with_metadata(slave, None, None, true, None)?.child)
}

pub fn spawn_interactive_with_metadata(
    slave: Box<dyn portable_pty::SlavePty + Send>,
    integration_root: Option<std::path::PathBuf>,
    preferred_shell: Option<String>,
    load_powershell_profile: bool,
    cwd: Option<&std::path::Path>,
) -> Result<InteractiveShell> {
    shell_integration::spawn(
        slave,
        integration_root.as_deref(),
        preferred_shell.as_deref(),
        load_powershell_profile,
        cwd,
    )
}

pub fn build_command(is_shell: bool, execution_string: &str) -> Result<CommandBuilder> {
    if is_shell {
        #[cfg(windows)]
        {
            let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
            let mut cmd = CommandBuilder::new(shell);
            // portable-pty quotes every argument for CreateProcessW. Passing
            // a command containing `"` directly to `/C` therefore turns the
            // quotes into literal `\"` characters for cmd.exe. Put the full
            // command in an environment variable and let cmd expand it after
            // argument parsing instead.
            cmd.args(["/D", "/S", "/C", "%COMMAND_MANAGER_SHELL_COMMAND%"]);
            cmd.env("COMMAND_MANAGER_SHELL_COMMAND", execution_string);
            Ok(cmd)
        }
        #[cfg(not(windows))]
        {
            let mut cmd = CommandBuilder::new("sh");
            cmd.arg("-c");
            cmd.arg(execution_string);
            Ok(cmd)
        }
    } else {
        let words = split_argv(execution_string)?;
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

fn build_shell_command(shell_kind: Option<&str>, execution_string: &str) -> Result<CommandBuilder> {
    #[cfg(not(windows))]
    {
        // Unix replays saved shell commands through `sh -c`. The recorded
        // shell kind only selects PowerShell versus cmd.exe on Windows.
        let _ = shell_kind;
    }
    #[cfg(windows)]
    {
        let kind = shell_kind.unwrap_or("cmd").to_ascii_lowercase();
        if matches!(kind.as_str(), "pwsh" | "powershell" | "powershell.exe") {
            let executable = if kind == "pwsh" {
                "pwsh"
            } else {
                "powershell.exe"
            };
            let mut command = CommandBuilder::new(executable);
            // Invoke the environment value as a script block. Passing the
            // variable expression alone only prints the command text and
            // incorrectly returns exit code 0.
            let runner = "if ($env:COMMAND_MANAGER_SHELL_FALLBACK_WARNING) { Write-Output $env:COMMAND_MANAGER_SHELL_FALLBACK_WARNING }; $global:LASTEXITCODE = 0; $cmErrors = $Error.Count; & ([scriptblock]::Create($env:COMMAND_MANAGER_SHELL_COMMAND)); $cmOk = $?; if ($global:LASTEXITCODE) { exit $global:LASTEXITCODE }; if (-not $cmOk -or $Error.Count -gt $cmErrors) { exit 1 }; exit 0";
            command.args(["-NoLogo", "-NoProfile", "-Command", runner]);
            command.env("COMMAND_MANAGER_SHELL_COMMAND", execution_string);
            return Ok(command);
        }
    }
    build_command(true, execution_string)
}

pub fn pump_reader(
    reader: Box<dyn Read + Send>,
    buffer: Arc<Mutex<RingBuffer>>,
    run_event_id: String,
    ipc: IpcPipe,
) {
    pump_reader_with_tracker(reader, buffer, run_event_id, ipc, None);
}

pub fn pump_reader_with_tracker(
    mut reader: Box<dyn Read + Send>,
    buffer: Arc<Mutex<RingBuffer>>,
    run_event_id: String,
    ipc: IpcPipe,
    mut tracker: Option<ShellTracker>,
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
                if let Some(tracker) = tracker.as_mut() {
                    tracker.feed(chunk);
                }
                ipc.push(run_event_id.clone(), chunk.to_vec());
            }
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::backpressure::IpcPipe;
    use std::time::Duration;

    #[cfg(windows)]
    #[test]
    fn terminal_env_keeps_inherited_path_entries() {
        let inherited = std::env::var_os("PATH").expect("test process has PATH");
        let inherited_first = std::env::split_paths(&inherited)
            .next()
            .expect("PATH has an entry");
        let registry_only = std::path::PathBuf::from(r"C:\command-manager-registry-path");
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.env(
            "PATH",
            std::env::join_paths([registry_only.as_path()]).unwrap(),
        );

        preserve_inherited_path(&mut cmd);

        let paths: Vec<_> = std::env::split_paths(cmd.get_env("PATH").unwrap()).collect();
        assert!(paths.contains(&inherited_first));
        assert!(paths.contains(&registry_only));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires a local pnpm installation and persisted VOLTA_HOME"]
    fn persisted_volta_runs_pnpm_in_pty_without_inherited_home() {
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).unwrap();
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.args(["/D", "/C", "pnpm --version"]);
        // Run in a test process without inherited tool homes to verify that
        // CommandBuilder recovers the persisted settings before spawning.
        set_terminal_env(&mut cmd);
        assert!(cmd.get_env("VOLTA_HOME").is_some());
        let child = slave.spawn_command(cmd).unwrap();
        let mut guard = ChildGuard(Some(child));
        let reader = pty.clone_reader().unwrap();
        let buffer = Arc::clone(&pty.buffer);
        let tx = crate::pty::backpressure::drained_sender();
        std::thread::spawn(move || {
            pump_reader(reader, buffer, "pnpm-env-probe".into(), IpcPipe::new(tx))
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = guard.0.as_mut().unwrap().try_wait().unwrap() {
                assert!(
                    status.success(),
                    "pnpm failed: {}",
                    String::from_utf8_lossy(&pty.snapshot().unwrap())
                );
                break;
            }
            assert!(std::time::Instant::now() < deadline, "pnpm probe timed out");
            std::thread::sleep(Duration::from_millis(50));
        }
        let version = regex::Regex::new(r"\b\d+\.\d+\.\d+\b").unwrap();
        for _ in 0..40 {
            if version.is_match(&String::from_utf8_lossy(&pty.snapshot().unwrap())) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!(
            "pnpm produced no version: {}",
            String::from_utf8_lossy(&pty.snapshot().unwrap())
        );
    }

    #[cfg(windows)]
    struct ChildGuard(Option<Box<dyn portable_pty::Child + Send + Sync>>);

    #[cfg(windows)]
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if let Some(mut child) = self.0.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    fn assert_prompt_marker_wraps_visible_prompt(snapshot: &[u8], shell: &str) {
        let text = String::from_utf8_lossy(snapshot);
        let start_marker = "\x1b]633;A\x07";
        let end_marker = "\x1b]633;B\x07";
        let start = text
            .find(start_marker)
            .unwrap_or_else(|| panic!("{shell} did not emit an A marker: {text:?}"));
        let content_start = start + start_marker.len();
        let end_relative = text[content_start..]
            .find(end_marker)
            .unwrap_or_else(|| panic!("{shell} did not emit B after A: {text:?}"));
        let prompt = &text[content_start..content_start + end_relative];
        assert!(
            !prompt.is_empty()
                // zsh's default prompt ends in `%#`, drawn as `%` or `#`.
                && (['>', '$', '%', '#'].iter().any(|c| prompt.contains(*c))
                    || prompt.contains("PS ")),
            "{shell} A-B marker range did not contain the visible prompt: {prompt:?}"
        );
    }

    struct Chunks(Vec<Vec<u8>>);

    impl Read for Chunks {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.0.is_empty() {
                return Ok(0);
            }
            let chunk = self.0.remove(0);
            buf[..chunk.len()].copy_from_slice(&chunk);
            Ok(chunk.len())
        }
    }

    fn filtered(chunks: &[&[u8]]) -> (Vec<u8>, Vec<u8>) {
        let sink = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer: Box<dyn Write + Send> = Box::new(SinkWriter(Arc::clone(&sink)));
        let mut reader = StartupQueryFilter::new(
            Chunks(chunks.iter().map(|chunk| chunk.to_vec()).collect()),
            Arc::new(Mutex::new(writer)),
        );
        let mut output = Vec::new();
        reader.read_to_end(&mut output).unwrap();
        let replies = sink.lock().unwrap().clone();
        (output, replies)
    }

    struct SinkWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SinkWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn startup_da1_query_is_answered_and_removed() {
        let (output, replies) = filtered(&[b"[1t[c[?1004hPS> "]);
        assert_eq!(output, b"[1t[?1004hPS> ");
        assert_eq!(replies, DA1_REPLY);
    }

    #[test]
    fn startup_da1_query_split_across_reads_is_answered() {
        for split in 1..DA1_QUERY.len() {
            let (head, tail) = DA1_QUERY.split_at(split);
            let (output, replies) =
                filtered(&[&[b"ab".as_slice(), head].concat(), &[tail, b"cd"].concat()]);
            assert_eq!(output, b"abcd", "split at {split}");
            assert_eq!(replies, DA1_REPLY, "split at {split}");
        }
    }

    #[test]
    fn da1_query_after_startup_reaches_the_terminal() {
        let late = [vec![b'x'; STARTUP_WINDOW], DA1_QUERY.to_vec()].concat();
        let (output, replies) = filtered(&[&late]);
        assert_eq!(output, late);
        assert!(replies.is_empty());
        let (output, replies) = filtered(&[b"[c", b"[c"]);
        assert_eq!(output, b"[c");
        assert_eq!(replies, DA1_REPLY);
    }

    #[test]
    fn interactive_shell_accepts_pty_input() {
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
        let tx = crate::pty::backpressure::drained_sender();
        let run_event_id = "test-interactive-shell".to_string();
        let shell =
            spawn_interactive_with_metadata(slave, None, None, false, None).expect("spawn shell");
        let mut child = shell.child;
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        std::thread::spawn(move || {
            pump_reader(reader, buffer, run_event_id, IpcPipe::new(tx));
        });

        let mut ready = false;
        let prompt_marker = b"\x1b]633;B";
        for _ in 0..120 {
            if pty
                .snapshot()
                .expect("read pty buffer")
                .windows(prompt_marker.len())
                .any(|window| window == prompt_marker)
            {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(ready, "interactive shell did not produce a prompt");
        std::thread::sleep(Duration::from_secs(2));
        pty.write(b"echo CM_PTY_INPUT_TEST\r\n")
            .expect("write pty input");
        let mut received = false;
        for _ in 0..120 {
            let snapshot = pty.snapshot().expect("read pty buffer");
            if snapshot
                .windows(b"CM_PTY_INPUT_TEST".len())
                .any(|window| window == b"CM_PTY_INPUT_TEST")
            {
                received = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let _ = child.kill();
        let _ = child.wait();
        assert!(
            received,
            "interactive shell did not echo input: {:?}",
            String::from_utf8_lossy(&pty.snapshot().expect("snapshot"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn interactive_shell_starts_in_the_requested_cwd() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cwd = dir.path().canonicalize().expect("canonical cwd");
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
        let mut shell = spawn_interactive_with_metadata(
            slave,
            None,
            Some(String::from("bash")),
            true,
            Some(&cwd),
        )
        .expect("spawn shell");
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        let tx = crate::pty::backpressure::drained_sender();
        std::thread::spawn(move || {
            pump_reader(reader, buffer, "test-cwd".into(), IpcPipe::new(tx));
        });

        let marker = format!("\x1b]633;P;Cwd={}", cwd.display());
        let mut found = false;
        for _ in 0..200 {
            let snapshot = pty.snapshot().expect("read pty buffer");
            if String::from_utf8_lossy(&snapshot).contains(&marker) {
                found = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = shell.child.kill();
        let _ = shell.child.wait();
        assert!(
            found,
            "shell did not report {cwd:?}: {:?}",
            String::from_utf8_lossy(&pty.snapshot().expect("snapshot"))
        );
    }

    #[test]
    fn interactive_shell_tracker_records_a_level_one_command() {
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
        let mut shell =
            spawn_interactive_with_metadata(slave, None, None, true, None).expect("spawn shell");
        let Some(nonce) = shell.nonce.clone() else {
            let _ = shell.child.kill();
            let _ = shell.child.wait();
            return;
        };

        let records = Arc::new(Mutex::new(Vec::<String>::new()));
        let output = Arc::clone(&records);
        let tracker = ShellTracker::new(
            nonce,
            shell.shell_kind.as_str().to_string(),
            move |record| {
                output
                    .lock()
                    .expect("history lock")
                    .push(record.command_line);
            },
        );
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        let tx = crate::pty::backpressure::drained_sender();
        std::thread::spawn(move || {
            pump_reader_with_tracker(
                reader,
                buffer,
                "test-history-tracker".into(),
                IpcPipe::new(tx),
                Some(tracker),
            );
        });

        let marker = b"\x1b]633;B";
        for _ in 0..120 {
            if pty
                .snapshot()
                .expect("read pty buffer")
                .windows(marker.len())
                .any(|window| window == marker)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let input = match shell.shell_kind {
            shell_integration::ShellKind::Pwsh
            | shell_integration::ShellKind::WindowsPowerShell => {
                b"Write-Output CM_HISTORY_TRACKER_TEST\r\n".as_slice()
            }
            shell_integration::ShellKind::Bash | shell_integration::ShellKind::Zsh => {
                b"printf 'CM_HISTORY_TRACKER_TEST\\n'\n".as_slice()
            }
            _ => {
                let _ = shell.child.kill();
                let _ = shell.child.wait();
                return;
            }
        };
        pty.write(input).expect("write shell input");

        let mut recorded = false;
        for _ in 0..120 {
            if records
                .lock()
                .expect("history lock")
                .iter()
                .any(|line| line.contains("CM_HISTORY_TRACKER_TEST"))
            {
                recorded = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = shell.child.kill();
        let _ = shell.child.wait();
        assert!(
            recorded,
            "level-one shell command was not recorded: {:?}",
            records.lock().expect("history lock")
        );
    }

    #[test]
    fn shell_execution_uses_the_platform_shell() {
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
        let mut child = spawn_on_slave(slave, true, "echo CM_SHELL_EXECUTION_TEST")
            .expect("spawn shell command");
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        std::thread::spawn(move || {
            let tx = crate::pty::backpressure::drained_sender();
            pump_reader(
                reader,
                buffer,
                "test-shell-command".into(),
                IpcPipe::new(tx),
            );
        });

        let status = child.wait().expect("wait for shell command");
        assert_eq!(status.exit_code(), 0);
        for _ in 0..20 {
            if pty
                .snapshot()
                .expect("read pty buffer")
                .windows(b"CM_SHELL_EXECUTION_TEST".len())
                .any(|window| window == b"CM_SHELL_EXECUTION_TEST")
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("shell command output was not captured");
    }

    #[cfg(not(windows))]
    #[test]
    fn children_see_an_xterm_term_even_when_the_app_has_none() {
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
        let mut child = spawn_on_slave(
            slave,
            true,
            "printf 'CM_TERM=%s/%s\\n' \"$TERM\" \"$COLORTERM\"",
        )
        .expect("spawn shell command");
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        std::thread::spawn(move || {
            let tx = crate::pty::backpressure::drained_sender();
            pump_reader(reader, buffer, "test-term-env".into(), IpcPipe::new(tx));
        });
        child.wait().expect("wait for shell command");
        let expected = b"CM_TERM=xterm-256color/truecolor";
        for _ in 0..40 {
            if pty
                .snapshot()
                .expect("read pty buffer")
                .windows(expected.len())
                .any(|window| window == expected)
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!(
            "TERM was not set for the child: {:?}",
            String::from_utf8_lossy(&pty.snapshot().expect("snapshot"))
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_shells_emit_integration_markers_and_accept_input() {
        let shells = [
            ("bash", shell_integration::ShellKind::Bash, 1),
            ("zsh", shell_integration::ShellKind::Zsh, 1),
            ("sh", shell_integration::ShellKind::Sh, 2),
        ];
        let mut tested = 0;

        for (name, expected_kind, expected_level) in shells {
            let available = std::process::Command::new("sh")
                .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            if !available {
                continue;
            }

            tested += 1;
            let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
            let mut shell =
                spawn_interactive_with_metadata(slave, None, Some(name.to_string()), true, None)
                    .expect("spawn unix shell");
            let reader = pty.clone_reader().expect("clone pty reader");
            let buffer = Arc::clone(&pty.buffer);
            let tx = crate::pty::backpressure::drained_sender();
            std::thread::spawn(move || {
                pump_reader(
                    reader,
                    buffer,
                    format!("test-{name}-shell"),
                    IpcPipe::new(tx),
                );
            });

            let marker = b"\x1b]633;B";
            let mut found_prompt = false;
            for _ in 0..120 {
                if pty
                    .snapshot()
                    .expect("read pty buffer")
                    .windows(marker.len())
                    .any(|window| window == marker)
                {
                    found_prompt = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(
                found_prompt,
                "{name} did not emit the B prompt marker: {:?}",
                String::from_utf8_lossy(&pty.snapshot().expect("snapshot"))
            );
            assert_prompt_marker_wraps_visible_prompt(&pty.snapshot().expect("snapshot"), name);

            pty.write(b"printf 'CM_UNIX_SHELL_TEST\\n'\n")
                .expect("write unix shell input");
            let mut found_output = false;
            for _ in 0..120 {
                if pty
                    .snapshot()
                    .expect("read pty buffer")
                    .windows(b"CM_UNIX_SHELL_TEST".len())
                    .any(|window| window == b"CM_UNIX_SHELL_TEST")
                {
                    found_output = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            let snapshot = pty.snapshot().expect("snapshot unix shell");
            let snapshot_text = String::from_utf8_lossy(&snapshot);
            let nonce = shell.nonce.clone().unwrap_or_default();
            let _ = shell.child.kill();
            let _ = shell.child.wait();

            assert_eq!(
                shell.shell_kind, expected_kind,
                "unexpected shell kind for {name}"
            );
            assert_eq!(
                shell.history_level, expected_level,
                "unexpected history level for {name}"
            );
            assert!(
                found_output,
                "{name} did not execute input: {snapshot_text:?}"
            );
            if expected_level == 1 {
                assert!(
                    snapshot_text.contains(&format!("E;")) && snapshot_text.contains(&nonce),
                    "{name} did not emit a nonce-bound E marker: {snapshot_text:?}"
                );
            }
        }

        assert!(
            tested > 0,
            "no supported Unix shell was available for the smoke test"
        );
    }

    #[cfg(windows)]
    #[test]
    fn pinned_conpty_resources_are_available_to_windows_pty() {
        let directory = super::conpty_sideload_directory()
            .expect("pinned ConPTY resources must be found for Windows PTY tests");
        assert!(directory.join("conpty.dll").is_file());
        assert!(directory.join("OpenConsole.exe").is_file());
    }

    #[cfg(windows)]
    #[test]
    fn cmd_preference_emits_prompt_markers_and_uses_level_two_history() {
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
        let mut shell =
            spawn_interactive_with_metadata(slave, None, Some(String::from("cmd")), true, None)
                .expect("spawn cmd shell");
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        let tx = crate::pty::backpressure::drained_sender();
        std::thread::spawn(move || {
            pump_reader(reader, buffer, "test-cmd-shell".into(), IpcPipe::new(tx));
        });
        pty.write(b"echo CM_CMD_BOOT\r").expect("write cmd input");
        let marker = b"\x1b]633;B";
        let mut found = false;
        for _ in 0..120 {
            if pty
                .snapshot()
                .expect("read pty buffer")
                .windows(marker.len())
                .any(|window| window == marker)
            {
                found = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let snapshot = pty.snapshot().expect("snapshot");
        let _ = shell.child.kill();
        let _ = shell.child.wait();
        assert_eq!(shell.shell_kind, shell_integration::ShellKind::Cmd);
        assert_eq!(shell.history_level, 2);
        assert!(
            snapshot
                .clone()
                .windows(b"\x1b]633;P;Cwd=".len())
                .any(|window| window == b"\x1b]633;P;Cwd="),
            "cmd shell did not emit the CWD marker: {:?}",
            String::from_utf8_lossy(&snapshot)
        );
        assert!(
            found,
            "cmd shell did not emit the B prompt marker: {:?}",
            String::from_utf8_lossy(&snapshot)
        );
        assert_prompt_marker_wraps_visible_prompt(&snapshot, "cmd");
    }

    #[cfg(windows)]
    #[test]
    fn git_bash_integration_records_compound_commands_and_skips_ignored_lines() {
        let bash = std::path::PathBuf::from(r"C:\Program Files\Git\bin\bash.exe");
        if !bash.is_file() {
            return;
        }

        let integration_dir = tempfile::tempdir().expect("integration tempdir");
        let script_path = integration_dir.path().join("bash.sh");
        std::fs::write(&script_path, include_str!("shell_integration/bash.sh"))
            .expect("write bash integration script");

        let (pty, slave) = PtySession::open(120, 30, 64 * 1024).expect("open pty");
        let mut command = CommandBuilder::new(bash.to_string_lossy().as_ref());
        command.args([
            "--noprofile",
            "--rcfile",
            &script_path.to_string_lossy(),
            "-i",
        ]);
        command.env("CM_NONCE", "git-bash-test");
        let child = slave
            .spawn_command(command)
            .expect("spawn Git Bash integration shell");
        let _child_guard = ChildGuard(Some(child));

        let records = Arc::new(Mutex::new(Vec::<String>::new()));
        let output = Arc::clone(&records);
        let tracker = ShellTracker::new("git-bash-test".into(), "bash".into(), move |record| {
            output
                .lock()
                .expect("history lock")
                .push(record.command_line)
        });
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        let tx = crate::pty::backpressure::drained_sender();
        std::thread::spawn(move || {
            pump_reader_with_tracker(
                reader,
                buffer,
                "test-git-bash-shell".into(),
                IpcPipe::new(tx),
                Some(tracker),
            );
        });

        let marker = b"\x1b]633;B";
        let mut found_prompt = false;
        for _ in 0..120 {
            if pty
                .snapshot()
                .expect("snapshot Git Bash")
                .windows(marker.len())
                .any(|window| window == marker)
            {
                found_prompt = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(found_prompt, "Git Bash did not emit the B prompt marker");

        pty.write(b"echo CM_GIT_BASH_ONE; echo CM_GIT_BASH_TWO\n")
            .expect("write compound Git Bash input");
        let mut recorded_compound = false;
        for _ in 0..120 {
            recorded_compound =
                records.lock().expect("history lock").iter().any(|line| {
                    line.contains("CM_GIT_BASH_ONE") && line.contains("CM_GIT_BASH_TWO")
                });
            if recorded_compound {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            recorded_compound,
            "compound Git Bash command was not recorded: records={:?}, output={:?}",
            records.lock().expect("history lock"),
            String::from_utf8_lossy(&pty.snapshot().expect("snapshot Git Bash"))
        );

        pty.write(b"export HISTCONTROL=ignorespace\n")
            .expect("enable ignorespace");
        std::thread::sleep(Duration::from_millis(150));
        pty.write(b"  echo CM_GIT_BASH_HIDDEN\n")
            .expect("write ignored Git Bash input");
        std::thread::sleep(Duration::from_millis(300));

        let history = records.lock().expect("history lock");
        assert!(
            !history
                .iter()
                .any(|line| line.contains("CM_GIT_BASH_HIDDEN")),
            "HISTCONTROL=ignorespace command leaked into history: {history:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn powershell_five_one_preference_uses_integrated_prompt_when_installed() {
        if std::process::Command::new("where.exe")
            .arg("powershell.exe")
            .status()
            .map(|status| !status.success())
            .unwrap_or(true)
        {
            return;
        }
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
        let mut shell = spawn_interactive_with_metadata(
            slave,
            None,
            Some(String::from("powershell")),
            true,
            None,
        )
        .expect("spawn Windows PowerShell");
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        let tx = crate::pty::backpressure::drained_sender();
        std::thread::spawn(move || {
            pump_reader(
                reader,
                buffer,
                "test-powershell-shell".into(),
                IpcPipe::new(tx),
            );
        });
        let marker = b"\x1b]633;B";
        let mut found = false;
        for _ in 0..120 {
            if pty
                .snapshot()
                .expect("read pty buffer")
                .windows(marker.len())
                .any(|window| window == marker)
            {
                found = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let initial_snapshot = pty.snapshot().expect("snapshot");
        assert_prompt_marker_wraps_visible_prompt(&initial_snapshot, "Windows PowerShell");
        pty.write(b"Get-Item -LiteralPath 'C:\\__command_manager_missing_9f2a'\r\n")
            .expect("write failing PowerShell cmdlet");
        let mut found_failure = false;
        for _ in 0..120 {
            let snapshot = pty.snapshot().expect("read pty buffer");
            if snapshot
                .windows(b"\x1b]633;D;1\x07".len())
                .any(|window| window == b"\x1b]633;D;1\x07")
            {
                found_failure = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let snapshot = pty.snapshot().expect("snapshot");
        let _ = shell.child.kill();
        let _ = shell.child.wait();
        assert_eq!(
            shell.shell_kind,
            shell_integration::ShellKind::WindowsPowerShell
        );
        assert_eq!(shell.history_level, 1);
        assert!(
            found,
            "Windows PowerShell did not emit the B prompt marker: {:?}",
            String::from_utf8_lossy(&pty.snapshot().expect("snapshot"))
        );
        assert!(
            found_failure,
            "Windows PowerShell did not report a failed cmdlet as D;1: {:?}",
            String::from_utf8_lossy(&snapshot)
        );
    }

    #[cfg(windows)]
    #[test]
    fn powershell_without_profile_uses_the_default_prompt() {
        if std::process::Command::new("where.exe")
            .arg("pwsh")
            .status()
            .map(|status| !status.success())
            .unwrap_or(true)
        {
            return;
        }
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
        let mut shell =
            spawn_interactive_with_metadata(slave, None, Some(String::from("pwsh")), false, None)
                .expect("spawn pwsh without profile");
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        let tx = crate::pty::backpressure::drained_sender();
        std::thread::spawn(move || {
            pump_reader(
                reader,
                buffer,
                "test-pwsh-no-profile".into(),
                IpcPipe::new(tx),
            );
        });
        let mut prompt = None;
        for _ in 0..120 {
            let text = String::from_utf8_lossy(&pty.snapshot().expect("snapshot")).to_string();
            if let Some(start) = text.find("]633;A") {
                let rest = &text[start + "]633;A".len()..];
                if let Some(end) = rest.find("]633;B") {
                    prompt = Some(rest[..end].to_string());
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = shell.child.kill();
        let _ = shell.child.wait();
        let prompt = prompt.expect("pwsh without profile did not emit an A..B prompt");
        assert!(
            prompt.trim_end().ends_with('>'),
            "profile-free pwsh should emit a usable built-in prompt: {prompt:?}"
        );
        assert!(
            !prompt.to_ascii_lowercase().contains("oh-my-posh")
                && !prompt.to_ascii_lowercase().contains("starship"),
            "-NoProfile shell must not load a profile prompt theme: {prompt:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn powershell_saved_command_executes_and_propagates_exit_code() {
        if std::process::Command::new("where.exe")
            .arg("pwsh")
            .status()
            .map(|status| !status.success())
            .unwrap_or(true)
        {
            return;
        }
        let (pty, slave) = PtySession::open(100, 30, 64 * 1024).expect("open pty");
        let mut child = spawn_shell_on_slave(
            slave,
            Some("pwsh"),
            r#"Write-Output ("RAN-" + (1+1)); exit 7"#,
        )
        .expect("spawn saved PowerShell command");
        let reader = pty.clone_reader().expect("clone pty reader");
        let buffer = Arc::clone(&pty.buffer);
        std::thread::spawn(move || {
            let tx = crate::pty::backpressure::drained_sender();
            pump_reader(reader, buffer, "test-pwsh-command".into(), IpcPipe::new(tx));
        });
        let status = child.wait().expect("wait for saved PowerShell command");
        let mut snapshot = Vec::new();
        for _ in 0..40 {
            snapshot = pty.snapshot().expect("snapshot output");
            if snapshot
                .windows(b"RAN-2".len())
                .any(|window| window == b"RAN-2")
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let output = String::from_utf8_lossy(&snapshot);
        assert!(
            output.contains("RAN-2"),
            "saved PowerShell command did not execute: {output:?}"
        );
        assert_eq!(status.exit_code(), 7);
    }
}
