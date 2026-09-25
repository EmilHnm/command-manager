use crate::error::{Error, Result};
use std::process::Command;

/// CTRL_C_EVENT only reaches the console process group; adapter uses taskkill tree as fallback.
pub fn terminate_graceful(pid: u32) -> Result<()> {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .status()?;
    if !status.success() {
        return Err(Error::msg("taskkill graceful failed"));
    }
    Ok(())
}

pub fn kill_force(pid: u32) -> Result<()> {
    let status = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .status()?;
    if !status.success() {
        return Err(Error::msg("taskkill force failed"));
    }
    Ok(())
}
