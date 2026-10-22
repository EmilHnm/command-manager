//! One shared connection per host. Every terminal holds a [`Lease`]; when the
//! last lease goes away the connection stays open for [`IDLE_CLOSE`] so a
//! reopened tab does not log in again.

use super::client::{self, ClientHandler, Connected};
use super::{ConnState, SshEnv};
use crate::db::repos::ssh_hosts;
use crate::error::{Error, Result};
use russh::client::Handle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

pub const IDLE_CLOSE: Duration = Duration::from_secs(60);

const JUMP_FAILED: &str = "Không kết nối được jump host";

pub struct Connection {
    host_id: String,
    connected: Connected,
    channels: AtomicUsize,
    /// Keeps the jump host connection open while this one lives.
    _via: Option<Lease>,
}

impl Connection {
    pub fn handle(&self) -> &Handle<ClientHandler> {
        &self.connected.handle
    }

    fn is_alive(&self) -> bool {
        !self.connected.handle.is_closed()
    }
}

/// A claim on a connection, held for as long as a channel is open on it.
pub struct Lease {
    conn: Arc<Connection>,
    inner: Weak<Inner>,
}

impl Lease {
    fn new(conn: Arc<Connection>, inner: Weak<Inner>) -> Self {
        conn.channels.fetch_add(1, Ordering::SeqCst);
        Self { conn, inner }
    }

    pub fn handle(&self) -> &Handle<ClientHandler> {
        self.conn.handle()
    }

    pub fn host_id(&self) -> &str {
        &self.conn.host_id
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if self.conn.channels.fetch_sub(1, Ordering::SeqCst) == 1 {
            if let Some(inner) = self.inner.upgrade() {
                inner.schedule_idle_close(Arc::clone(&self.conn));
            }
        }
    }
}

type Slot = Arc<tokio::sync::Mutex<Option<Arc<Connection>>>>;

struct Inner {
    env: Arc<dyn SshEnv>,
    slots: Mutex<HashMap<String, Slot>>,
}

impl Inner {
    fn slot(&self, host_id: &str) -> Slot {
        self.slots
            .lock()
            .expect("ssh slots lock")
            .entry(host_id.to_string())
            .or_default()
            .clone()
    }

    /// `path` holds the hosts already waiting on this one (it is their jump
    /// host). Checked before taking the slot lock: a loop in the saved jump
    /// hosts would otherwise wait on a lock its own caller holds.
    fn lease(
        self: Arc<Self>,
        host_id: String,
        mut path: Vec<String>,
    ) -> super::BoxFuture<'static, Result<Lease>> {
        Box::pin(async move {
            if path.contains(&host_id) {
                return Err(Error::msg("Chuỗi jump host bị vòng lặp"));
            }
            if path.len() > ssh_hosts::MAX_JUMP_HOPS {
                return Err(Error::msg(format!(
                    "Chỉ hỗ trợ đi qua tối đa {} jump host liên tiếp",
                    ssh_hosts::MAX_JUMP_HOPS
                )));
            }
            let slot = self.slot(&host_id);
            // Held while connecting, so tabs opened together share one
            // login and one set of dialogs.
            let mut current = slot.lock().await;
            if let Some(conn) = current.as_ref().filter(|conn| conn.is_alive()) {
                return Ok(Lease::new(Arc::clone(conn), Arc::downgrade(&self)));
            }
            *current = None;

            let target = self.env.target(&host_id)?;
            let host = &target.host;
            if host.use_system_ssh {
                return Err(Error::msg(
                    "Máy chủ này dùng OpenSSH hệ thống, không có kết nối dùng chung",
                ));
            }
            let via = match host.jump_host_id.clone() {
                Some(jump_id) => {
                    path.push(host_id.clone());
                    let lease = Arc::clone(&self).lease(jump_id, path).await;
                    Some(lease.map_err(|error| {
                        // Only the hop nearest the failure says so.
                        let error = error.to_string();
                        if error.starts_with(JUMP_FAILED) {
                            Error::msg(error)
                        } else {
                            Error::msg(format!("{JUMP_FAILED}: {error}"))
                        }
                    })?)
                }
                None => None,
            };
            let connected = client::connect(
                Arc::clone(&self.env),
                &target,
                via.as_ref().map(Lease::handle),
            )
            .await?;
            let conn = Arc::new(Connection {
                host_id: host_id.clone(),
                connected,
                channels: AtomicUsize::new(0),
                _via: via,
            });
            *current = Some(Arc::clone(&conn));
            Ok(Lease::new(conn, Arc::downgrade(&self)))
        })
    }

    fn schedule_idle_close(self: Arc<Self>, conn: Arc<Connection>) {
        let env = Arc::clone(&self.env);
        env.spawn(Box::pin(async move {
            tokio::time::sleep(IDLE_CLOSE).await;
            let slot = self.slot(&conn.host_id);
            let mut current = slot.lock().await;
            let still_idle = current
                .as_ref()
                .is_some_and(|open| Arc::ptr_eq(open, &conn))
                && conn.channels.load(Ordering::SeqCst) == 0;
            if still_idle {
                *current = None;
                drop(current);
                conn.connected.close().await;
            }
        }));
    }
}

#[derive(Clone)]
pub struct SshManager {
    inner: Arc<Inner>,
}

impl SshManager {
    pub fn new(env: Arc<dyn SshEnv>) -> Self {
        Self {
            inner: Arc::new(Inner {
                env,
                slots: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn env(&self) -> Arc<dyn SshEnv> {
        Arc::clone(&self.inner.env)
    }

    /// Returns a lease on the host's connection, connecting (and asking for
    /// host key / password) when there is none.
    pub async fn lease(&self, host_id: &str) -> Result<Lease> {
        Arc::clone(&self.inner)
            .lease(host_id.to_string(), Vec::new())
            .await
    }

    /// Number of open channels per connected host, for SCR-07.
    pub fn open_channels(&self) -> HashMap<String, usize> {
        let slots: Vec<(String, Slot)> = self
            .inner
            .slots
            .lock()
            .expect("ssh slots lock")
            .iter()
            .map(|(id, slot)| (id.clone(), slot.clone()))
            .collect();
        slots
            .into_iter()
            .filter_map(|(id, slot)| {
                // A slot that is locked is connecting; it has no channels yet.
                let current = slot.try_lock().ok()?;
                let conn = current.as_ref().filter(|conn| conn.is_alive())?;
                Some((id, conn.channels.load(Ordering::SeqCst)))
            })
            .collect()
    }

    /// Closes a host's connection now. Its terminals see the channel end.
    pub async fn disconnect(&self, host_id: &str) {
        let slot = self.inner.slot(host_id);
        let conn = slot.lock().await.take();
        if let Some(conn) = conn {
            conn.connected.close().await;
            self.inner.env.status(host_id, ConnState::Closed, None);
        }
    }
}
