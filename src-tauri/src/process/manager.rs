use crate::pty::terminal_io::TerminalIo;
use dashmap::{DashMap, DashSet};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Finished terminals whose output is kept for a tab that opens late: a
/// command like `echo 1` exits before its tab attaches.
const KEEP_FINISHED: usize = 16;

pub struct Live {
    pub run_event_id: String,
    pub command_id: String,
    pub session_id: String,
    pub group_id: String,
    pub pid: u32,
    pub pty: Arc<dyn TerminalIo>,
    pub shell_kind: String,
    pub history_level: u8,
}

#[derive(Clone, Serialize)]
pub struct LiveSnapshot {
    pub run_event_id: String,
    pub command_id: String,
    pub session_id: String,
    pub group_id: String,
    pub pid: u32,
    pub buffer_bytes: usize,
    pub shell_kind: String,
    pub history_level: u8,
}

#[derive(Default)]
pub struct ProcessManager {
    live: DashMap<String, Arc<Live>>,
    pending: DashSet<String>,
    finished_before_insert: DashSet<String>,
    transition_lock: Mutex<()>,
    /// The terminal itself, not a snapshot: output still in flight when the
    /// process exited lands in its buffer a moment later.
    recently_finished: Mutex<VecDeque<(String, Arc<dyn TerminalIo>)>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark an event as ready to be inserted after its process has spawned.
    ///
    /// The child may exit before the caller gets a chance to insert its PTY.
    /// Reserving the id lets the exit callback record that completion without
    /// treating unrelated remove calls as a completion tombstone.
    pub fn reserve(&self, run_event_id: &str) {
        let _guard = self.transition_lock.lock().expect("process manager lock");
        self.pending.insert(run_event_id.to_string());
    }

    pub fn insert(&self, live: Live) {
        let _guard = self.transition_lock.lock().expect("process manager lock");
        self.pending.remove(&live.run_event_id);
        if self
            .finished_before_insert
            .remove(&live.run_event_id)
            .is_some()
        {
            return;
        }
        self.live.insert(live.run_event_id.clone(), Arc::new(live));
    }

    /// Cancel a reservation when spawning or database registration fails.
    pub fn cancel(&self, run_event_id: &str) {
        let _guard = self.transition_lock.lock().expect("process manager lock");
        self.pending.remove(run_event_id);
        self.finished_before_insert.remove(run_event_id);
    }

    pub fn get(&self, run_event_id: &str) -> Option<Arc<Live>> {
        self.live.get(run_event_id).map(|v| Arc::clone(&*v))
    }

    pub fn remove(&self, run_event_id: &str) -> Option<Arc<Live>> {
        let _guard = self.transition_lock.lock().expect("process manager lock");
        match self.live.remove(run_event_id) {
            Some((_, value)) => {
                let mut finished = self
                    .recently_finished
                    .lock()
                    .expect("finished terminals lock");
                finished.retain(|(id, _)| id != run_event_id);
                finished.push_back((run_event_id.to_string(), Arc::clone(&value.pty)));
                while finished.len() > KEEP_FINISHED {
                    finished.pop_front();
                }
                Some(value)
            }
            None if self.pending.remove(run_event_id).is_some() => {
                self.finished_before_insert.insert(run_event_id.to_string());
                None
            }
            None => None,
        }
    }

    /// The terminal of a process that already exited, if still kept.
    pub fn finished(&self, run_event_id: &str) -> Option<Arc<dyn TerminalIo>> {
        self.recently_finished
            .lock()
            .expect("finished terminals lock")
            .iter()
            .find(|(id, _)| id == run_event_id)
            .map(|(_, pty)| Arc::clone(pty))
    }

    pub fn snapshot(&self) -> Vec<LiveSnapshot> {
        self.live
            .iter()
            .map(|e| {
                let v = e.value();
                LiveSnapshot {
                    run_event_id: v.run_event_id.clone(),
                    command_id: v.command_id.clone(),
                    session_id: v.session_id.clone(),
                    group_id: v.group_id.clone(),
                    pid: v.pid,
                    buffer_bytes: v.pty.buffer_len(),
                    shell_kind: v.shell_kind.clone(),
                    history_level: v.history_level,
                }
            })
            .collect()
    }

    pub fn pids(&self) -> Vec<u32> {
        self.live
            .iter()
            .map(|e| e.value().pid)
            .filter(|p| *p != 0)
            .collect()
    }

    pub fn ids_for_session(&self, session_id: &str) -> Vec<String> {
        self.live
            .iter()
            .filter(|e| e.value().session_id == session_id)
            .map(|e| e.key().clone())
            .collect()
    }

    pub fn group_has_live(&self, group_id: &str) -> bool {
        self.live.iter().any(|e| e.value().group_id == group_id)
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// Asks every terminal without a local PID (SSH) to end.
    pub fn stop_remote(&self, force: bool) {
        let remote: Vec<Arc<Live>> = self
            .live
            .iter()
            .filter(|e| e.value().pty.is_remote())
            .map(|e| Arc::clone(e.value()))
            .collect();
        for live in remote {
            live.pty.stop(force);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::session::PtySession;

    fn live(run_event_id: &str) -> Live {
        let (pty, _slave) = PtySession::open(80, 24, 64 * 1024).expect("open pty");
        Live {
            run_event_id: run_event_id.to_string(),
            command_id: "command".into(),
            session_id: "session".into(),
            group_id: "group".into(),
            pid: 1,
            pty: Arc::new(pty),
            shell_kind: "test".into(),
            history_level: 0,
        }
    }

    #[test]
    fn removing_an_unknown_id_does_not_block_a_later_insert() {
        let manager = ProcessManager::new();

        assert!(manager.remove("unknown").is_none());
        manager.insert(live("unknown"));

        assert!(manager.get("unknown").is_some());
    }

    #[test]
    fn exit_before_insert_is_not_reinserted() {
        let manager = ProcessManager::new();
        manager.reserve("fast");

        assert!(manager.remove("fast").is_none());
        manager.insert(live("fast"));

        assert!(manager.get("fast").is_none());
        assert!(manager.is_empty());
    }

    #[test]
    fn output_of_a_finished_process_stays_readable() {
        let manager = ProcessManager::new();
        manager.insert(live("quick"));
        manager.get("quick").unwrap().pty.write(b"").unwrap();
        assert!(manager.remove("quick").is_some());
        assert!(manager.get("quick").is_none());
        assert!(manager.finished("quick").is_some());

        for i in 0..KEEP_FINISHED {
            let id = format!("p{i}");
            manager.insert(live(&id));
            manager.remove(&id);
        }
        // Only the most recent ones are kept.
        assert!(manager.finished("quick").is_none());
        assert!(manager
            .finished(&format!("p{}", KEEP_FINISHED - 1))
            .is_some());
    }
}
