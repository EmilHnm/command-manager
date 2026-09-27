use crate::pty::session::PtySession;
use dashmap::{DashMap, DashSet};
use serde::Serialize;
use std::sync::{Arc, Mutex};

pub struct Live {
    pub run_event_id: String,
    pub command_id: String,
    pub session_id: String,
    pub group_id: String,
    pub pid: u32,
    pub pty: Arc<PtySession>,
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
            Some((_, value)) => Some(value),
            None if self.pending.remove(run_event_id).is_some() => {
                self.finished_before_insert.insert(run_event_id.to_string());
                None
            }
            None => None,
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
