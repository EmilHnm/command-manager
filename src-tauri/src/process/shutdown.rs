use super::manager::ProcessManager;
use super::platform;
use std::time::{Duration, Instant};

pub fn stop_pid(pid: u32) {
    let _ = platform::terminate_graceful(pid);
}

pub fn kill_pid(pid: u32) {
    let _ = platform::kill_force(pid);
}

/// SIGTERM/CTRL_C → wait → SIGKILL/TerminateProcess.
pub fn shutdown_all(mgr: &ProcessManager, timeout: Duration) {
    for pid in mgr.pids() {
        stop_pid(pid);
    }
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if mgr.is_empty() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    for pid in mgr.pids() {
        kill_pid(pid);
    }
    let hard = Instant::now() + Duration::from_secs(2);
    while Instant::now() < hard && !mgr.is_empty() {
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manager_returns_immediately() {
        let mgr = ProcessManager::new();
        let start = Instant::now();
        shutdown_all(&mgr, Duration::from_secs(5));
        assert!(start.elapsed() < Duration::from_millis(200));
    }
}
