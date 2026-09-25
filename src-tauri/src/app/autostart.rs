use crate::db::repos::groups;
use crate::db::Db;
use crate::error::Result;
use crate::process::manager::ProcessManager;

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
    } else {
        mgr.disable()
            .map_err(|e| crate::error::Error::msg(e.to_string()))?;
    }
    Ok(())
}
