#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Must run before Tauri's single-instance lock, DB, autostart or PTYs.
    #[cfg(all(windows, feature = "desktop"))]
    match command_manager_lib::windows_launch::prepare_startup() {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            command_manager_lib::windows_launch::show_launch_error(&error);
            std::process::exit(1);
        }
    }
    #[cfg(feature = "desktop")]
    command_manager_lib::run();
    #[cfg(not(feature = "desktop"))]
    {
        eprintln!(
            "Build with --features desktop after installing: libwebkit2gtk-4.1-dev libgtk-3-dev libglib2.0-dev"
        );
        std::process::exit(1);
    }
}
