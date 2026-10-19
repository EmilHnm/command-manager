pub mod autostart;
pub mod shutdown;
pub mod single_instance;

use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

/// Where the app keeps its database, vault key, known_hosts and exports.
///
/// A dev build (`pnpm tauri:dev`) uses its own `dev/` subfolder: it runs
/// migrations the installed release does not know yet, and a shared database
/// would leave the installed app refusing to start ("schema N is newer").
pub fn data_dir<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<PathBuf> {
    let dir = app.path().app_data_dir()?;
    #[cfg(debug_assertions)]
    let dir = dir.join("dev");
    Ok(dir)
}
