fn main() {
    // Rust's test executable also loads Tauri's TaskDialogIndirect import,
    // which requires Common Controls v6 (the app already has a manifest).
    if cfg!(feature = "ipc-tests") && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
    }
    #[cfg(feature = "desktop")]
    tauri_build::build();

    #[cfg(windows)]
    {
        // Keep development PTYs on the same pinned ConPTY build as the
        // packaged app. portable-pty resolves conpty.dll by name.
        let manifest_dir =
            std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let source = manifest_dir
            .join("resources")
            .join("conpty")
            .join("win-x64");
        let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        if let Some(profile_dir) = out_dir.ancestors().nth(3) {
            for file in ["conpty.dll", "OpenConsole.exe"] {
                let from = source.join(file);
                if from.exists() {
                    let _ = std::fs::copy(&from, profile_dir.join(file));
                    println!("cargo:rerun-if-changed={}", from.display());
                }
            }
        }
    }
}
