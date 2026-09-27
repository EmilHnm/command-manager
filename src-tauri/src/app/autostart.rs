use crate::db::repos::groups;
use crate::db::Db;
use crate::error::Result;
use crate::process::manager::ProcessManager;

pub const AUTOSTART_ARG: &str = "--command-manager-autostart";

pub fn launched_by_os_autostart() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
}

/// After single-instance lock: start autostart groups that are not already live.
pub fn start_autostart_groups(
    db: &Db,
    processes: &ProcessManager,
    start_group: impl Fn(&str) -> Result<()>,
) -> Result<()> {
    let groups = db.read(groups::list_autostart)?;
    for g in groups {
        if processes.group_has_live(&g.id) {
            continue;
        }
        start_group(&g.id)?;
    }
    Ok(())
}

pub fn os_enabled(app: &tauri::AppHandle) -> Result<bool> {
    use tauri_plugin_autostart::ManagerExt;
    Ok(app.autolaunch().is_enabled().unwrap_or(false))
}

pub fn os_set(app: &tauri::AppHandle, enabled: bool) -> Result<()> {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = app.autolaunch();
    if enabled {
        mgr.enable()
            .map_err(|e| crate::error::Error::msg(e.to_string()))?;
    } else if let Err(error) = mgr.disable() {
        // auto-launch deletes the Run registry value (or the Linux .desktop
        // file) unconditionally, so disabling an entry that was never
        // registered fails with "file not found". That is already the
        // requested state; only surface the error if the entry still exists.
        if mgr.is_enabled().unwrap_or(false) {
            return Err(crate::error::Error::msg(error.to_string()));
        }
    }
    Ok(())
}
