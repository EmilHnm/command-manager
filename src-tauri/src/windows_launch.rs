//! Keep the installer launch context out of the terminal process tree.
//!
//! MSI can enforce Redirection Guard on its children. Node then reports
//! UNKNOWN/ENOENT for pnpm junctions even though their targets exist. Asking
//! the existing desktop Explorer to open the app preserves the desktop user's
//! normal environment/token/policies; we never disable a mitigation or edit
//! machine policy. Ordinary CLI launches retain their own environment.

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use windows::core::{Interface, BSTR, HSTRING};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IDispatch, IServiceProvider,
    CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessMitigationPolicy, ProcessRedirectionTrustPolicy,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Shell::{
    IShellBrowser, IShellDispatch2, IShellFolderViewDual, IShellWindows, SID_STopLevelBrowser,
    ShellWindows, SVGIO_BACKGROUND, SWC_DESKTOP, SWFO_NEEDDISPATCH,
};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

pub const INSTALLER_LAUNCH_ARG: &str = "--launch-from-installer";

/// `true` means launch was delegated: the caller must exit without starting
/// Tauri. Stripping the flag prevents recursive relaunch, even if Explorer
/// itself is restricted by an administrator's policy.
pub fn prepare_startup() -> Result<bool, String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.iter().any(|arg| arg == INSTALLER_LAUNCH_ARG) {
        let forwarded = forwarded_arguments(args);
        launch_from_desktop(&forwarded).map_err(|error| {
            format!("Không thể mở Command Manager qua Windows Explorer: {error}\n\nHãy đóng bộ cài và mở Command Manager từ Start Menu hoặc shortcut Desktop.")
        })?;
        return Ok(true);
    }
    if redirection_guard_enabled() == Some(true) {
        return Err("Windows Redirection Guard đang hạn chế truy cập junction của tiến trình này. Terminal sẽ không đọc được một số package trong node_modules của pnpm.\n\nHãy thoát hoàn toàn Command Manager (kể cả tray), rồi mở lại từ Start Menu hoặc shortcut Desktop, không mở từ bộ cài. Nếu vẫn gặp lỗi, hãy kiểm tra chính sách với quản trị viên Windows. Không cần xoá hay cài lại node_modules.".into());
    }
    Ok(false)
}

fn forwarded_arguments(args: Vec<OsString>) -> Vec<OsString> {
    args.into_iter()
        .filter(|arg| arg != INSTALLER_LAUNCH_ARG)
        .collect()
}

/// Older Windows versions may not support this query. This is a read-only
/// diagnostic, not an attempt to clear or override process security settings.
pub fn redirection_guard_enabled() -> Option<bool> {
    let mut flags = 0u32;
    unsafe {
        GetProcessMitigationPolicy(
            GetCurrentProcess(),
            ProcessRedirectionTrustPolicy,
            (&mut flags as *mut u32).cast(),
            std::mem::size_of_val(&flags),
        )
        .ok()?;
    }
    Some(flags & 1 != 0)
}

struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn launch_from_desktop(args: &[OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or("Executable has no parent directory")?;
    let file = BSTR::from_wide(&executable.as_os_str().encode_wide().collect::<Vec<_>>());
    let parameters = VARIANT::from(BSTR::from_wide(&quote_arguments(args)));
    let cwd = VARIANT::from(BSTR::from_wide(
        &directory.as_os_str().encode_wide().collect::<Vec<_>>(),
    ));
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let _apartment = ComApartment;
        // Bind to the existing desktop, NOT a new Shell.Application instance
        // and NOT ShellExecuteW in this MSI-inherited process.
        let shell_windows: IShellWindows =
            CoCreateInstance(&ShellWindows, None, CLSCTX_LOCAL_SERVER)?;
        let mut hwnd = 0;
        let desktop = shell_windows.FindWindowSW(
            &VARIANT::default(),
            &VARIANT::default(),
            SWC_DESKTOP,
            &mut hwnd,
            SWFO_NEEDDISPATCH,
        )?;
        let services: IServiceProvider = desktop.cast()?;
        let browser: IShellBrowser = services.QueryService(&SID_STopLevelBrowser)?;
        let view = browser.QueryActiveShellView()?;
        let dispatch: IDispatch = view.GetItemObject(SVGIO_BACKGROUND)?;
        let folder: IShellFolderViewDual = dispatch.cast()?;
        let shell: IShellDispatch2 = folder.Application()?.cast()?;
        shell.ShellExecute(
            &file,
            &parameters,
            &cwd,
            &VARIANT::from("open"),
            &VARIANT::from(1i32),
        )?;
    }
    Ok(())
}

/// Windows argv quoting (not PowerShell/cmd interpolation). Preserve Unicode,
/// embedded quotes, empty arguments and trailing backslashes exactly.
fn quote_arguments(args: &[OsString]) -> Vec<u16> {
    let mut result = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        if index != 0 {
            result.push(b' ' as u16);
        }
        result.push(b'"' as u16);
        let mut backslashes = 0;
        for unit in OsStr::new(arg).encode_wide() {
            if unit == b'\\' as u16 {
                backslashes += 1;
                continue;
            }
            let count = if unit == b'"' as u16 {
                backslashes * 2 + 1
            } else {
                backslashes
            };
            result.extend(std::iter::repeat_n(b'\\' as u16, count));
            result.push(unit);
            backslashes = 0;
        }
        result.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2));
        result.push(b'"' as u16);
    }
    result
}

pub fn show_launch_error(message: &str) {
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from("Command Manager — Không thể khởi động terminal"),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_flag_is_removed_without_losing_autostart_or_other_args() {
        let args = [
            INSTALLER_LAUNCH_ARG,
            "--autostart",
            "",
            "đường dẫn có dấu",
            INSTALLER_LAUNCH_ARG,
        ];
        let forwarded = forwarded_arguments(args.into_iter().map(OsString::from).collect());
        assert_eq!(
            forwarded,
            ["--autostart", "", "đường dẫn có dấu"].map(OsString::from)
        );
    }

    #[test]
    fn windows_argument_quoting_handles_quotes_and_trailing_slashes() {
        for (arg, expected) in [
            ("", "\"\""),
            ("--autostart", "\"--autostart\""),
            ("đường dẫn & dấu ;", "\"đường dẫn & dấu ;\""),
            ("C:\\path with spaces\\", "\"C:\\path with spaces\\\\\""),
            ("a\"b", "\"a\\\"b\""),
            ("a\\\"b", "\"a\\\\\\\"b\""),
        ] {
            assert_eq!(
                String::from_utf16(&quote_arguments(&[arg.into()])).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn msi_launch_uses_desktop_handoff_for_both_ui_and_silent_install() {
        let template = include_str!("../windows/msi/main.wxs");
        assert!(template.contains("ExeCommand=\"--launch-from-installer [LAUNCHAPPARGS]\""));
        assert!(template.contains("Value=\"LaunchApplication\""));
        assert!(
            template.contains("<Custom Action=\"LaunchApplication\" After=\"InstallFinalize\">")
        );
    }
}
