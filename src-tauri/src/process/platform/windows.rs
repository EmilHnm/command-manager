use crate::error::{Error, Result};
use std::os::windows::process::CommandExt;
use std::process::Command;

/// The app is a GUI process without a console, so every console tool it
/// starts would otherwise open (and flash) a console window of its own.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn taskkill() -> Command {
    let mut command = Command::new("taskkill");
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Attempts tree termination with taskkill /T /PID <pid>.
/// If the process cannot be terminated gracefully (e.g., console apps requiring /F),
/// falls back to taskkill /F /T /PID <pid> to guarantee the entire process tree is terminated.
pub fn terminate_graceful(pid: u32) -> Result<()> {
    let status = taskkill().args(["/T", "/PID", &pid.to_string()]).status();

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => kill_force(pid),
    }
}

pub fn kill_force(pid: u32) -> Result<()> {
    let status = taskkill()
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .status()?;
    if !status.success() {
        return Err(Error::msg("taskkill force failed"));
    }
    Ok(())
}
