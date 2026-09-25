pub mod backup;
pub mod clock;
pub mod db;
pub mod error;
pub mod process;
pub mod pty;

#[cfg(feature = "desktop")]
mod app;
#[cfg(feature = "desktop")]
pub mod ipc;

#[cfg(feature = "desktop")]
mod desktop {
    use crate::app;
    use crate::db;
    use crate::ipc;
    use crate::ipc::events;
    use crate::process;
    use crate::pty::backpressure::IpcPipe;
    use base64::Engine;
    use std::time::Duration;
    use tauri::{Emitter, Manager};
    use tauri_plugin_autostart::MacosLauncher;

    pub fn run() {
        tauri::Builder::default()
            .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
                app::single_instance::on_second_instance(app);
            }))
            .plugin(tauri_plugin_autostart::init(
                MacosLauncher::LaunchAgent,
                Some(vec![]),
            ))
            .setup(|app| {
                let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let db = db::Db::open(dir.join("app.db")).map_err(|e| e.to_string())?;
                let processes = process::manager::ProcessManager::new();
                let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, Vec<u8>)>(32);
                let ipc_pipe = IpcPipe::new(tx);
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    while let Some((id, data)) = rx.recv().await {
                        let b64 = base64::engine::general_purpose::STANDARD.encode(data);
                        let _ = handle.emit(
                            events::PTY_DATA,
                            serde_json::json!({ "run_event_id": id, "b64": b64 }),
                        );
                    }
                });
                app.manage(ipc::AppState {
                    db,
                    processes,
                    ipc: ipc_pipe,
                });

                let handle = app.handle().clone();
                let state = app.state::<ipc::AppState>();
                let _ = app::autostart::start_autostart_groups(&state.db, &state.processes, |gid| {
                    ipc::commands::start_group_inner(&handle, gid).map(|_| ())
                });
                Ok(())
            })
            .on_window_event(|window, event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if window.label() != "main" {
                        return;
                    }
                    api.prevent_close();
                    let app = window.app_handle().clone();
                    std::thread::spawn(move || {
                        let state = app.state::<ipc::AppState>();
                        let secs = state
                            .db
                            .read(|c| db::repos::settings::get(c, "shutdown_timeout_secs"))
                            .ok()
                            .flatten()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(8);
                        let _ = app.emit(events::SHUTDOWN_PROGRESS, "stopping");
                        process::shutdown::shutdown_all(
                            &state.processes,
                            Duration::from_secs(secs),
                        );
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.destroy();
                        }
                    });
                }
            })
            .invoke_handler(tauri::generate_handler![
                ipc::commands::commands_list,
                ipc::commands::commands_create,
                ipc::commands::commands_update,
                ipc::commands::commands_delete,
                ipc::commands::groups_list,
                ipc::commands::groups_create,
                ipc::commands::groups_update,
                ipc::commands::groups_delete,
                ipc::commands::groups_memberships,
                ipc::commands::groups_set_memberships,
                ipc::commands::groups_set_autostart,
                ipc::commands::sessions_list,
                ipc::commands::session_events,
                ipc::commands::process_list,
                ipc::commands::session_start,
                ipc::commands::session_stop,
                ipc::commands::process_stop,
                ipc::commands::pty_resize,
                ipc::commands::pty_write,
                ipc::commands::pty_reattach,
                ipc::commands::backup_export,
                ipc::commands::backup_import,
                ipc::commands::settings_get,
                ipc::commands::settings_set,
                ipc::commands::autostart_os_is_enabled,
                ipc::commands::autostart_os_set,
                ipc::commands::app_shutdown,
            ])
            .run(tauri::generate_context!())
            .expect("error while running Command Manager");
    }
}

#[cfg(feature = "desktop")]
pub fn run() {
    desktop::run();
}
