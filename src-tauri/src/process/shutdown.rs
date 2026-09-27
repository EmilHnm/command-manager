use super::manager::ProcessManager;
use super::platform;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub fn stop_pid(pid: u32) {
    let _ = platform::terminate_graceful(pid);
}

pub fn kill_pid(pid: u32) {
    let _ = platform::kill_force(pid);
}

/// SIGTERM/CTRL_C → wait → SIGKILL/TerminateProcess.
pub fn shutdown_all(mgr: &ProcessManager, timeout: Duration) {
    shutdown_all_with_control(mgr, timeout, &AtomicBool::new(false), |_, _| {});
}

pub fn shutdown_all_with_control(
    mgr: &ProcessManager,
    timeout: Duration,
    force: &AtomicBool,
    mut progress: impl FnMut(u64, bool),
) {
    let forcing = force.load(Ordering::SeqCst);
    progress(if forcing { 0 } else { timeout.as_secs() }, forcing);
    for pid in mgr.pids() {
        if force.load(Ordering::SeqCst) {
            break;
        }
        stop_pid(pid);
    }
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if mgr.is_empty() {
            return;
        }
        if force.load(Ordering::SeqCst) {
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        progress(
            remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0),
            false,
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    progress(0, true);
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

    #[test]
    fn force_request_interrupts_the_grace_period() {
        use crate::process::manager::Live;
        use crate::pty::session::PtySession;
        use std::sync::Arc;

        let mgr = ProcessManager::new();
        let (pty, _slave) = PtySession::open(80, 24, 65536).unwrap();
        // PID 0 is excluded from OS termination: only test the shutdown coordination.
        mgr.insert(Live {
            run_event_id: "shutdown-test".into(),
            command_id: String::new(),
            session_id: String::new(),
            group_id: String::new(),
            pid: 0,
            pty: Arc::new(pty),
            shell_kind: "test".into(),
            history_level: 0,
        });
        let force = AtomicBool::new(false);
        let mut saw_forcing = false;
        let start = Instant::now();
        shutdown_all_with_control(
            &mgr,
            Duration::from_secs(10),
            &force,
            |remaining, forcing| {
                if forcing {
                    assert_eq!(remaining, 0);
                    saw_forcing = true;
                    mgr.remove("shutdown-test");
                } else {
                    force.store(true, Ordering::SeqCst);
                }
            },
        );
        assert!(saw_forcing);
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
