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

/// Return whether the operating system still reports the process as alive.
/// This is used after a stop request so callers do not treat a best-effort
/// signal as proof that the process has released its resources.
pub fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        unix::is_process_alive(pid)
    }
    #[cfg(windows)]
    {
        windows::is_process_alive(pid)
    }
}

/// Attach a spawned process to the platform's app-bound lifetime mechanism.
/// Windows uses a Job Object with `KILL_ON_JOB_CLOSE`; other platforms do not
/// need an additional registration here.
pub fn register_process(pid: u32) -> Result<()> {
    #[cfg(windows)]
    {
        windows::register_process(pid)
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        Ok(())
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
