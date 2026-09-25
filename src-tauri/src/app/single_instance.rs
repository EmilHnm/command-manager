use tauri::{AppHandle, Emitter, Manager};

/// Plugin is registered in `lib.rs` before setup so the lock is held
/// before ProcessManager exists. This callback focuses the live window.
pub fn on_second_instance(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("app://instance", "already-running");
    }
}
