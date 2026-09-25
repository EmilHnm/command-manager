#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
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
