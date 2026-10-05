use super::*;
use crate::process::manager::ProcessManager;
use crate::pty::{backpressure::IpcPipe, session};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};

struct ShellGuard(Box<dyn portable_pty::Child + Send + Sync>);
impl Drop for ShellGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn frontend_snake_case_payload_executes_input_through_tauri() {
    let dir = tempfile::tempdir().unwrap();
    let tx = crate::pty::backpressure::drained_sender();
    let ipc = IpcPipe::new(tx);
    let (pty, slave) = session::PtySession::open(100, 30, 65536).unwrap();
    let child = ShellGuard(session::spawn_interactive_on_slave(slave).unwrap());
    let pid = child.0.process_id().unwrap();
    let reader = pty.clone_reader().unwrap();
    let buffer = Arc::clone(&pty.buffer);
    let pipe = ipc.clone();
    std::thread::spawn(move || session::pump_reader(reader, buffer, "terminal:test".into(), pipe));
    let processes = ProcessManager::new();
    processes.insert(Live {
        run_event_id: "terminal:test".into(),
        command_id: "terminal:test".into(),
        session_id: "terminal-session:test".into(),
        group_id: "terminal".into(),
        pid,
        pty: Arc::new(pty),
        shell_kind: "test".into(),
        history_level: 0,
    });
    let app = mock_builder()
        .manage(AppState {
            db: crate::db::Db::open(dir.path().join("test.db")).unwrap(),
            processes,
            ipc,
        })
        .invoke_handler(tauri::generate_handler![
            pty_write,
            pty_resize,
            pty_reattach,
            process_stop
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str, body: serde_json::Value| {
        get_ipc_response(
            &window,
            tauri::webview::InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.into(),
            },
        )
    };
    invoke(
        "pty_resize",
        serde_json::json!({
            "run_event_id": "terminal:test", "cols": 110, "rows": 32
        }),
    )
    .expect("frontend resize payload must reach PTY");

    // Expected output is constructed by the shell, not present literally in input.
    #[cfg(windows)]
    let input = "set CM_IPC_VALUE=INPUT_OK\r\necho CM_IPC_%CM_IPC_VALUE%\r\n";
    #[cfg(not(windows))]
    let input = "printf 'CM_IPC_%s\\n' INPUT_OK\n";
    invoke(
        "pty_write",
        serde_json::json!({
            "run_event_id": "terminal:test",
            "b64": base64::engine::general_purpose::STANDARD.encode(input)
        }),
    )
    .expect("frontend write payload must reach PTY");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let encoded = invoke(
            "pty_reattach",
            serde_json::json!({
                "run_event_id": "terminal:test"
            }),
        )
        .expect("frontend reattach payload must reach PTY")
        .deserialize::<String>()
        .unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        if String::from_utf8_lossy(&bytes).contains("CM_IPC_INPUT_OK") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "shell did not execute input sent through IPC"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // An unknown ID must pass argument decoding and fail at process lookup.
    let error = invoke(
        "process_stop",
        serde_json::json!({
            "run_event_id": "missing"
        }),
    )
    .unwrap_err();
    assert!(error.to_string().contains("process not running"), "{error}");
}

#[test]
fn frontend_snake_case_payload_creates_command() {
    let dir = tempfile::tempdir().unwrap();
    let tx = crate::pty::backpressure::drained_sender();
    let app = mock_builder()
        .manage(AppState {
            db: crate::db::Db::open(dir.path().join("test.db")).unwrap(),
            processes: ProcessManager::new(),
            ipc: IpcPipe::new(tx),
        })
        .invoke_handler(tauri::generate_handler![commands_create])
        .build(mock_context(noop_assets()))
        .unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let response = get_ipc_response(
        &window,
        tauri::webview::InvokeRequest {
            cmd: "commands_create".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(serde_json::json!({
                "name": "IPC create test",
                "execution_string": "echo test",
                "is_shell": false,
                "shell_kind": null,
                "confirmed": true
            })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        },
    )
    .expect("snake_case create payload must reach Rust command");

    let created = response
        .deserialize::<crate::db::repos::commands::CommandDef>()
        .unwrap();
    assert_eq!(created.name, "IPC create test");
    assert_eq!(created.execution_string, "echo test");
    assert!(!created.is_shell);
}
