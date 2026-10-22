//! Runs one CLI process: stream-json from stdout goes through the adapter's
//! parser into the log tab; stderr is shown dimmed. Stops on a timeout or
//! a cancel (the run's "Dừng ngay" or the tab's stop), taking the CLI's
//! child processes with it.

use super::adapter::{child_path, Cli, LogLine, Outcome, Parser};
use super::log::AgentLog;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::watch;

const STDERR_TAIL: usize = 20;

pub struct Spawn<'a> {
    pub cli: Cli,
    pub program: &'a Path,
    pub args: Vec<String>,
    pub cwd: &'a Path,
    pub timeout: Duration,
    /// Every stdout line as received, for diagnosing a CLI.
    pub raw_log: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunError {
    Cancelled,
    TimedOut,
    Failed(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "Đã dừng"),
            Self::TimedOut => write!(f, "Hết thời gian"),
            Self::Failed(message) => write!(f, "{message}"),
        }
    }
}

pub async fn run(
    spawn: Spawn<'_>,
    log: &AgentLog,
    mut stop: watch::Receiver<bool>,
) -> Result<Outcome, RunError> {
    let mut cmd = Command::new(spawn.program);
    cmd.args(&spawn.args)
        .current_dir(spawn.cwd)
        .env("PATH", child_path(spawn.program))
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd.spawn().map_err(|error| {
        RunError::Failed(format!(
            "Không chạy được {}: {error}",
            spawn.program.display()
        ))
    })?;
    let pid = child.id();

    let tail: Arc<Mutex<Vec<String>>> = Arc::default();
    let stderr_task = child.stderr.take().map(|stderr| {
        let tail = Arc::clone(&tail);
        let mut lines = BufReader::new(stderr).lines();
        // The log is not 'static; stderr lines are shown after the run.
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                let mut tail = tail.lock().expect("stderr tail");
                tail.push(line);
                if tail.len() > STDERR_TAIL {
                    tail.remove(0);
                }
            }
        })
    });

    let mut raw = spawn
        .raw_log
        .as_ref()
        .and_then(|path| std::fs::File::create(path).ok());
    let mut parser = Parser::new(spawn.cli);
    let mut lines = BufReader::new(child.stdout.take().expect("piped stdout")).lines();
    let mut tab_stop = log.cancelled();
    let deadline = tokio::time::Instant::now() + spawn.timeout;

    let ended = loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    if let Some(file) = raw.as_mut() {
                        let _ = writeln!(file, "{line}");
                    }
                    for entry in parser.feed(&line) {
                        log.line(&entry);
                    }
                }
                _ => break None,
            },
            _ = tokio::time::sleep_until(deadline) => break Some(RunError::TimedOut),
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    break Some(RunError::Cancelled);
                }
            }
            changed = tab_stop.changed() => {
                if changed.is_ok() && *tab_stop.borrow() {
                    break Some(RunError::Cancelled);
                }
            }
        }
    };

    if let Some(error) = ended {
        terminate(&mut child, pid).await;
        if let Some(task) = stderr_task {
            task.abort();
        }
        return Err(error);
    }
    let status = child.wait().await;
    if let Some(task) = stderr_task {
        let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
    }
    let stderr: Vec<String> = tail.lock().expect("stderr tail").clone();
    for line in &stderr {
        log.line(&LogLine::Info(line.clone()));
    }

    let mut outcome = parser.finish();
    let code = status.ok().and_then(|status| status.code());
    if outcome.error.is_none() && code != Some(0) {
        let last = stderr
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .cloned()
            .unwrap_or_default();
        outcome.error = Some(format!(
            "{} thoát với mã {}{}",
            spawn.cli.as_str(),
            code.map_or("?".to_string(), |c| c.to_string()),
            if last.is_empty() {
                String::new()
            } else {
                format!(": {last}")
            }
        ));
    }
    if outcome.error.is_none() && !outcome.finished {
        outcome.error = Some(format!(
            "{} kết thúc mà không trả kết quả",
            spawn.cli.as_str()
        ));
    }
    Ok(outcome)
}

/// SIGTERM to the whole process group, SIGKILL if it lingers.
async fn terminate(child: &mut tokio::process::Child, pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        use nix::sys::signal::{killpg, Signal};
        use nix::unistd::Pid;
        let group = Pid::from_raw(pid as i32);
        let _ = killpg(group, Signal::SIGTERM);
        if tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .is_ok()
        {
            return;
        }
        let _ = killpg(group, Signal::SIGKILL);
    }
    #[cfg(not(unix))]
    let _ = pid;
    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// Runs a check or setup command through the shell. Output goes to the log;
/// the end of it is returned.
pub async fn run_shell(
    command: &str,
    cwd: &Path,
    timeout: Duration,
    log: &AgentLog,
    mut stop: watch::Receiver<bool>,
) -> Result<(bool, Option<i32>, String), RunError> {
    let mut cmd = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);
        cmd
    };
    // Same PATH as the agents: a desktop-started app may lack ~/.local/bin.
    cmd.current_dir(cwd)
        .env("PATH", child_path(Path::new("")))
        .env("CI", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    cmd.process_group(0);
    let mut child = cmd
        .spawn()
        .map_err(|error| RunError::Failed(format!("Không chạy được \"{command}\": {error}")))?;
    let pid = child.id();
    let mut out = BufReader::new(child.stdout.take().expect("piped")).lines();
    let mut err = BufReader::new(child.stderr.take().expect("piped")).lines();
    let mut tail: Vec<String> = Vec::new();
    let keep = |line: String, tail: &mut Vec<String>| {
        log.raw(&format!("{line}\n"));
        tail.push(line);
        if tail.len() > 60 {
            tail.remove(0);
        }
    };
    let deadline = tokio::time::Instant::now() + timeout;
    let (mut out_done, mut err_done) = (false, false);
    let mut tab_stop = log.cancelled();
    while !(out_done && err_done) {
        tokio::select! {
            line = out.next_line(), if !out_done => match line {
                Ok(Some(line)) => keep(line, &mut tail),
                _ => out_done = true,
            },
            line = err.next_line(), if !err_done => match line {
                Ok(Some(line)) => keep(line, &mut tail),
                _ => err_done = true,
            },
            _ = tokio::time::sleep_until(deadline) => {
                terminate(&mut child, pid).await;
                return Err(RunError::TimedOut);
            }
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    terminate(&mut child, pid).await;
                    return Err(RunError::Cancelled);
                }
            }
            changed = tab_stop.changed() => {
                if changed.is_ok() && *tab_stop.borrow() {
                    terminate(&mut child, pid).await;
                    return Err(RunError::Cancelled);
                }
            }
        }
    }
    let status = child.wait().await.ok();
    let code = status.and_then(|status| status.code());
    Ok((code == Some(0), code, tail.join("\n")))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn script(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("fake-cli");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn streams_parse_and_failures_report_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(
            dir.path(),
            r#"echo '{"type":"thread.started","thread_id":"t9"}'
echo 'warn: something' >&2
echo '{"type":"item.completed","item":{"type":"agent_message","text":"done"}}'
echo '{"type":"turn.completed","usage":{"input_tokens":3,"output_tokens":1}}'"#,
        );
        let log = AgentLog::new("t".into(), 64 * 1024, None);
        let (_tx, rx) = watch::channel(false);
        let outcome = run(
            Spawn {
                cli: Cli::Codex,
                program: &program,
                args: Vec::new(),
                cwd: dir.path(),
                timeout: Duration::from_secs(10),
                raw_log: Some(dir.path().join("raw.jsonl")),
            },
            &log,
            rx,
        )
        .await
        .expect("ran");
        assert_eq!(outcome.text, "done");
        assert_eq!(outcome.session_id.as_deref(), Some("t9"));
        assert!(outcome.error.is_none());
        let shown = String::from_utf8(log.snapshot_bytes()).unwrap();
        assert!(shown.contains("done") && shown.contains("warn: something"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("raw.jsonl"))
                .unwrap()
                .lines()
                .count(),
            3
        );

        let failing = script(dir.path(), "echo 'not logged in' >&2; exit 3");
        let (_tx, rx) = watch::channel(false);
        let outcome = run(
            Spawn {
                cli: Cli::Claude,
                program: &failing,
                args: Vec::new(),
                cwd: dir.path(),
                timeout: Duration::from_secs(10),
                raw_log: None,
            },
            &log,
            rx,
        )
        .await
        .expect("ran");
        assert_eq!(
            outcome.error.as_deref(),
            Some("claude thoát với mã 3: not logged in")
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stop_and_timeout_kill_the_process_group() {
        let dir = tempfile::tempdir().unwrap();
        // A child that would outlive a plain kill of the shell.
        let program = script(dir.path(), "sleep 30 & sleep 30");
        let log = AgentLog::new("t".into(), 1024, None);
        let (tx, rx) = watch::channel(false);
        let started = std::time::Instant::now();
        let stopper = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            tx.send_replace(true);
            tx
        });
        let result = run(
            Spawn {
                cli: Cli::Claude,
                program: &program,
                args: Vec::new(),
                cwd: dir.path(),
                timeout: Duration::from_secs(20),
                raw_log: None,
            },
            &log,
            rx,
        )
        .await;
        assert_eq!(result, Err(RunError::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(5));
        let _tx = stopper.await.unwrap();

        let (_tx, rx) = watch::channel(false);
        let result = run_shell("sleep 30", dir.path(), Duration::from_millis(300), &log, rx).await;
        assert_eq!(result, Err(RunError::TimedOut));

        let (_tx, rx) = watch::channel(false);
        let (ok, code, tail) = run_shell(
            "echo hi; echo bad >&2; exit 2",
            dir.path(),
            Duration::from_secs(10),
            &log,
            rx,
        )
        .await
        .expect("ran");
        assert!(!ok);
        assert_eq!(code, Some(2));
        assert!(tail.contains("hi") && tail.contains("bad"));
    }
}
