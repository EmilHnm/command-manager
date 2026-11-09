use tauri::{AppHandle, Emitter, Manager};

/// Plugin is registered in `lib.rs` before setup so the lock is held
/// before ProcessManager exists. This callback focuses the live window.
/// `argv`: the second instance's arguments; a folder from Explorer's
/// "Open in Command Manager" opens in this window instead.
pub fn on_second_instance(app: &AppHandle, argv: &[String]) {
    let folder = super::explorer_menu::open_dir_arg(argv);
    if let Some(dir) = &folder {
        app.state::<super::explorer_menu::PendingDirs>()
            .push(dir.clone());
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        if folder.is_some() {
            let _ = w.emit(crate::ipc::events::OPEN_DIR, ());
        } else {
            let _ = w.emit("app://instance", "already-running");
        }
    }
}
