use crate::error::{Error, Result};
use nix::errno::Errno;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

pub fn terminate_graceful(pid: u32) -> Result<()> {
    kill(Pid::from_raw(pid as i32), Signal::SIGTERM).map_err(|e| Error::msg(e.to_string()))
}

pub fn kill_force(pid: u32) -> Result<()> {
    kill(Pid::from_raw(pid as i32), Signal::SIGKILL).map_err(|e| Error::msg(e.to_string()))
}

pub fn is_process_alive(pid: u32) -> bool {
    match kill(Pid::from_raw(pid as i32), None) {
        Ok(()) | Err(Errno::EPERM) => true,
        Err(_) => false,
    }
}

pub fn pdeathsig_pre_exec() -> Result<()> {
    // SAFETY: called only from the child after fork, before exec.
    let rc = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) };
    if rc != 0 {
        return Err(Error::msg("PR_SET_PDEATHSIG failed"));
    }
    Ok(())
}
