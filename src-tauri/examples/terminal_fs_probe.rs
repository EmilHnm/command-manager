//! Read-only diagnostic through the app's real PTY, shell integration and Job
//! Object. Usage: terminal_fs_probe PROJECT_ROOT RELATIVE_FILE REPORT_FILE
//! Add --launch-from-installer to exercise the desktop handoff (no Tauri/DB).
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    use command_manager_lib::{
        pty::{
            backpressure::IpcPipe,
            session::{self, PtySession},
        },
        windows_launch,
    };
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    // Diagnostic-only: allow probing a guarded process without the production
    // startup check, so the same executable can reproduce and verify the fix.
    if std::env::args_os().any(|arg| arg == windows_launch::INSTALLER_LAUNCH_ARG) {
        windows_launch::prepare_startup().expect("desktop handoff");
        return;
    }
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    assert_eq!(
        args.len(),
        3,
        "usage: terminal_fs_probe PROJECT_ROOT RELATIVE_FILE REPORT_FILE"
    );
    let root = args[0].to_str().expect("UTF-8 project path");
    let relative_file = args[1].to_str().expect("UTF-8 relative file");
    let before = windows_launch::redirection_guard_enabled();
    let (pty, slave) = PtySession::open(240, 40, 256 * 1024).unwrap();
    let mut shell =
        session::spawn_interactive_with_metadata(slave, None, Some("pwsh".into()), false).unwrap();
    command_manager_lib::process::platform::register_process(shell.child.process_id().unwrap())
        .unwrap();
    let reader = pty.clone_reader().unwrap();
    let buffer = Arc::clone(&pty.buffer);
    let (tx, _rx) = tokio::sync::mpsc::channel(256);
    std::thread::spawn(move || {
        session::pump_reader(reader, buffer, "probe".into(), IpcPipe::new(tx))
    });
    let node_script = format!(
        "const fs=require('fs');const p={};try{{console.log('CM_FS_READ_OK',JSON.stringify({{node:process.execPath,real:fs.realpathSync(p),bytes:fs.readFileSync(p).length}}))}}catch(e){{console.log('CM_FS_READ_FAILED',e.code,e.errno);process.exit(1)}}",
        serde_json::to_string(relative_file).unwrap(),
    );
    let script = format!(
        "Set-Location -LiteralPath '{}'; node -e '{}'; $cmProbeExit = $LASTEXITCODE; pnpm --version; if ($LASTEXITCODE -ne 0) {{ $cmProbeExit = 1 }}; exit $cmProbeExit\r",
        root.replace('\'', "''"), node_script.replace('\'', "''"),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut sent = false;
    let status = loop {
        if !sent
            && pty
                .snapshot()
                .unwrap()
                .windows(7)
                .any(|w| w == b"\x1b]633;B")
        {
            pty.write(script.as_bytes()).unwrap();
            sent = true;
        }
        if let Some(status) = shell.child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            shell.child.kill().unwrap();
            shell.child.wait().unwrap();
            panic!(
                "probe timed out: {}",
                String::from_utf8_lossy(&pty.snapshot().unwrap())
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    std::thread::sleep(Duration::from_millis(100));
    let report = format!(
        "redirection_guard={before:?}\nshell_success={}\n{}",
        status.success(),
        String::from_utf8_lossy(&pty.snapshot().unwrap()),
    );
    std::fs::write(&args[2], report).expect("write diagnostic report");
    if !status.success() {
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows PTY diagnostic only");
}
