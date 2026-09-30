pub mod argv;
pub mod backup;
pub mod clock;
pub mod db;
pub mod error;
pub mod os_history;
pub mod process;
pub mod pty;
pub mod template;
#[cfg(windows)]
pub mod windows_launch;

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
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
    use tauri::{Emitter, Manager};
    use tauri_plugin_autostart::MacosLauncher;

    pub fn run() {
        tauri::Builder::default()
            .manage(app::shutdown::ShutdownState::default())
            .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
                app::single_instance::on_second_instance(app);
            }))
            .plugin(tauri_plugin_autostart::init(
                MacosLauncher::LaunchAgent,
                Some(vec![app::autostart::AUTOSTART_ARG]),
            ))
            .setup(|app| {
                let show_item =
                    MenuItem::with_id(app, "show", "Mở Command Manager", true, None::<&str>)?;
                let quit_item =
                    MenuItem::with_id(app, "quit", "Thoát hoàn toàn", true, None::<&str>)?;
                let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;
                let mut tray = TrayIconBuilder::with_id("main-tray")
                    .menu(&tray_menu)
                    .tooltip("Command Manager")
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => app::shutdown::request(app, false),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        } = event
                        {
                            if let Some(window) = tray.app_handle().get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    });
                if let Some(icon) = app.default_window_icon() {
                    tray = tray.icon(icon.clone());
                }
                tray.build(app)?;

                if app::autostart::launched_by_os_autostart() {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }

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
                let _ =
                    app::autostart::start_autostart_groups(&state.db, &state.processes, |gid| {
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
                    let shutdown_state = window.state::<app::shutdown::ShutdownState>();
                    if shutdown_state.is_started() {
                        app::shutdown::request(window.app_handle(), true);
                    } else {
                        let _ = window.emit(events::CLOSE_REQUESTED, ());
                    }
                }
            })
            .invoke_handler(tauri::generate_handler![
                ipc::commands::commands_list,
                ipc::commands::commands_create,
                ipc::commands::commands_update,
                ipc::commands::commands_delete,
                ipc::commands::templates_list,
                ipc::commands::templates_create,
                ipc::commands::templates_update,
                ipc::commands::templates_delete,
                ipc::commands::template_preview,
                ipc::commands::template_preset_save,
                ipc::commands::template_preset_delete,
                ipc::commands::template_run,
                ipc::commands::groups_list,
                ipc::commands::groups_create,
                ipc::commands::groups_update,
                ipc::commands::groups_delete,
                ipc::commands::groups_memberships,
                ipc::commands::groups_set_memberships,
                ipc::commands::groups_set_autostart,
                ipc::commands::sessions_list,
                ipc::commands::session_events,
                ipc::commands::history_list,
                ipc::commands::history_delete,
                ipc::commands::history_clear,
                ipc::commands::history_os_sources,
                ipc::commands::history_import_os,
                ipc::commands::history_record_typed,
                ipc::commands::process_list,
                ipc::commands::session_start,
                ipc::commands::command_run,
                ipc::commands::terminal_open,
                ipc::commands::session_stop,
                ipc::commands::process_stop,
                ipc::commands::pty_resize,
                ipc::commands::pty_write,
                ipc::commands::pty_reattach,
                ipc::commands::backup_export,
                ipc::commands::backup_info,
                ipc::commands::backup_verify_bytes,
                ipc::commands::backup_import,
                ipc::commands::backup_import_bytes,
                ipc::commands::settings_get,
                ipc::commands::settings_set,
                ipc::commands::autostart_os_is_enabled,
                ipc::commands::autostart_os_set,
                ipc::commands::app_hide,
                ipc::commands::app_shutdown,
                ipc::commands::open_url,
            ])
            .run(tauri::generate_context!())
            .expect("error while running Command Manager");
    }
}

#[cfg(feature = "desktop")]
pub fn run() {
    desktop::run();
}
