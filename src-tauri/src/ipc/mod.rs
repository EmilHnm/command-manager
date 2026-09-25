pub mod commands;
pub mod events;

use crate::db::Db;
use crate::process::manager::ProcessManager;
use crate::pty::backpressure::IpcPipe;

pub struct AppState {
    pub db: Db,
    pub processes: ProcessManager,
    pub ipc: IpcPipe,
}
