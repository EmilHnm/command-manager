use crate::pty::session::PtySession;
use dashmap::DashMap;
use serde::Serialize;
use std::sync::Arc;

pub struct Live {
    pub run_event_id: String,
    pub command_id: String,
    pub session_id: String,
    pub group_id: String,
    pub pid: u32,
    pub pty: Arc<PtySession>,
}

#[derive(Clone, Serialize)]
pub struct LiveSnapshot {
    pub run_event_id: String,
    pub command_id: String,
    pub session_id: String,
    pub group_id: String,
    pub pid: u32,
    pub buffer_bytes: usize,
}

#[derive(Default)]
pub struct ProcessManager {
    live: DashMap<String, Arc<Live>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, live: Live) {
        self.live.insert(live.run_event_id.clone(), Arc::new(live));
    }

    pub fn get(&self, run_event_id: &str) -> Option<Arc<Live>> {
        self.live.get(run_event_id).map(|v| Arc::clone(&*v))
    }

    pub fn remove(&self, run_event_id: &str) -> Option<Arc<Live>> {
        self.live.remove(run_event_id).map(|(_, v)| v)
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
                }
            })
            .collect()
    }

    pub fn pids(&self) -> Vec<u32> {
        self.live.iter().map(|e| e.value().pid).filter(|p| *p != 0).collect()
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
