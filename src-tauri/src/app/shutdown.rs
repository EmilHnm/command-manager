use crate::{db::repos::settings, ipc::events, ipc::AppState, process::shutdown};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct ShutdownState {
    started: AtomicBool,
    force: AtomicBool,
}

impl ShutdownState {
    pub fn is_started(&self) -> bool {
        self.started.load(Ordering::SeqCst)
    }

    fn request(&self, force: bool) -> bool {
        if force {
            self.force.store(true, Ordering::SeqCst);
        }
        !self.started.swap(true, Ordering::SeqCst)
    }
}

/// All close paths share one worker; IPC returns immediately so Force Exit stays usable.
pub fn request(app: &AppHandle, force: bool) {
    if !app.state::<ShutdownState>().request(force) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let control = app.state::<ShutdownState>();
        let timeout = state
            .db
            .read(|c| settings::get(c, "shutdown_timeout_secs"))
            .ok()
            .flatten()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(8)
            .clamp(1, 15);
        let mut last_progress = None;
        shutdown::shutdown_all_with_control(
            &state.processes,
            Duration::from_secs(timeout),
            &control.force,
            |remaining, forcing| {
                if last_progress == Some((remaining, forcing)) {
                    return;
                }
                last_progress = Some((remaining, forcing));
                let _ = app.emit(
                    events::SHUTDOWN_PROGRESS,
                    serde_json::json!({
                        "remaining_secs": remaining,
                        "timeout_secs": timeout,
                        "phase": if forcing { "forcing" } else { "stopping" },
                    }),
                );
            },
        );
        // Exit directly: window.close() would re-enter CloseRequested and run cleanup twice.
        app.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_close_does_not_start_another_worker_and_force_escalates() {
        let state = ShutdownState::default();
        assert!(state.request(false));
        assert!(!state.request(false));
        assert!(!state.force.load(Ordering::SeqCst));
        assert!(!state.request(true));
        assert!(state.force.load(Ordering::SeqCst));
        assert!(!state.request(false));
        assert!(state.force.load(Ordering::SeqCst));
    }

    #[test]
    fn force_can_start_shutdown() {
        let state = ShutdownState::default();
        assert!(state.request(true));
        assert!(state.force.load(Ordering::SeqCst));
    }
}
