#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

use super::InteractiveShell;
use crate::error::Result;
use portable_pty::SlavePty;

pub(super) fn spawn(
    slave: Box<dyn SlavePty + Send>,
    integration_root: Option<&std::path::Path>,
    preferred_shell: Option<&str>,
    load_powershell_profile: bool,
) -> Result<InteractiveShell> {
    #[cfg(windows)]
    {
        windows::spawn(
            slave,
            integration_root,
            preferred_shell,
            load_powershell_profile,
        )
    }
    #[cfg(not(windows))]
    {
        unix::spawn(
            slave,
            integration_root,
            preferred_shell,
            load_powershell_profile,
        )
    }
}
