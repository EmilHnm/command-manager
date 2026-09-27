#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use crate::error::Result;

#[cfg(windows)]
pub use windows::CREATE_NO_WINDOW;

pub fn terminate_graceful(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        unix::terminate_graceful(pid)
    }
    #[cfg(windows)]
    {
        windows::terminate_graceful(pid)
    }
}

pub fn kill_force(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        unix::kill_force(pid)
    }
    #[cfg(windows)]
    {
        windows::kill_force(pid)
    }
}

/// Linux: set PDEATHSIG in the child before exec. No-op elsewhere.
pub fn pdeathsig_pre_exec() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        unix::pdeathsig_pre_exec()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(())
    }
}
