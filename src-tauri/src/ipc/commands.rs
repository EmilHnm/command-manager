use crate::app;
use crate::backup;
use crate::clock;
use crate::db::repos::{commands as cmd_repo, groups as group_repo, runs, settings};
use crate::db::repos::commands::CommandDef;
use crate::db::repos::groups::{Group, Membership};
use crate::error::{Error, Result};
use crate::ipc::events;
use crate::process::manager::{Live, LiveSnapshot};
use crate::process::{shutdown, spawn};
use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use super::AppState;

fn require(confirmed: bool) -> Result<()> {
    if confirmed {
        Ok(())
    } else {
        Err(Error::ConfirmationRequired)
    }
}

fn buffer_bytes(state: &AppState) -> usize {
    state
        .db
        .read(|c| settings::get(c, "ring_buffer_bytes"))
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2 * 1024 * 1024)
}

fn shutdown_secs(state: &AppState) -> u64 {
    state
        .db
        .read(|c| settings::get(c, "shutdown_timeout_secs"))
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
}

#[tauri::command]
pub fn commands_list(state: State<AppState>) -> Result<Vec<CommandDef>> {
    state.db.read(cmd_repo::list)
}

#[tauri::command]
pub fn commands_create(state: State<AppState>, name: String, execution_string: String, is_shell: bool, confirmed: bool) -> Result<CommandDef> {
    require(confirmed)?;
    let cmd = CommandDef {
        id: Uuid::new_v4().to_string(),
        name,
        execution_string,
        is_shell,
    };
    state.db.write(|c| cmd_repo::insert(c, &cmd))?;
    Ok(cmd)
}

#[tauri::command]
pub fn commands_update(state: State<AppState>, cmd: CommandDef, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| cmd_repo::update(c, &cmd))
}

#[tauri::command]
pub fn commands_delete(state: State<AppState>, id: String, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| cmd_repo::delete(c, &id))
}

#[tauri::command]
pub fn groups_list(state: State<AppState>) -> Result<Vec<Group>> {
    state.db.read(group_repo::list)
}

#[tauri::command]
pub fn groups_create(state: State<AppState>, group_name: String, confirmed: bool) -> Result<Group> {
    require(confirmed)?;
    let g = Group {
        id: Uuid::new_v4().to_string(),
        group_name,
        autostart: false,
    };
    state.db.write(|c| group_repo::insert(c, &g))?;
    Ok(g)
}

#[tauri::command]
pub fn groups_update(state: State<AppState>, group: Group, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| group_repo::update(c, &group))
}

#[tauri::command]
pub fn groups_delete(state: State<AppState>, id: String, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| group_repo::delete(c, &id))
}

#[tauri::command]
pub fn groups_memberships(state: State<AppState>, group_id: String) -> Result<Vec<Membership>> {
    state.db.read(|c| group_repo::memberships(c, &group_id))
}

#[tauri::command]
pub fn groups_set_memberships(
    state: State<AppState>,
    group_id: String,
    members: Vec<Membership>,
    confirmed: bool,
) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| group_repo::replace_memberships(c, &group_id, &members))
}

#[tauri::command]
pub fn groups_set_autostart(state: State<AppState>, group_id: String, autostart: bool, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| {
        let mut list = group_repo::list(c)?;
        let g = list
            .iter_mut()
            .find(|g| g.id == group_id)
            .ok_or_else(|| Error::msg("group not found"))?;
        g.autostart = autostart;
        group_repo::update(c, g)
    })
}

#[tauri::command]
pub fn sessions_list(state: State<AppState>) -> Result<Vec<runs::RunSession>> {
    state.db.read(runs::list_sessions)
}

#[tauri::command]
pub fn session_events(state: State<AppState>, session_id: String) -> Result<Vec<runs::RunEvent>> {
    state.db.read(|c| runs::list_events(c, &session_id))
}

#[tauri::command]
pub fn process_list(state: State<AppState>) -> Result<Vec<LiveSnapshot>> {
    Ok(state.processes.snapshot())
}

#[derive(Serialize, Clone)]
struct StatusEvent {
    run_event_id: String,
    status: String,
    exit_code: Option<i32>,
    pid: Option<u32>,
}

pub fn start_group_inner(app: &AppHandle, group_id: &str) -> Result<String> {
    let state = app.state::<AppState>();
    let members = state.db.read(|c| group_repo::memberships(c, group_id))?;
    if members.is_empty() {
        return Err(Error::msg("group has no commands"));
    }
    let session_id = Uuid::new_v4().to_string();
    let started = clock::now_unix();
    state.db.write(|c| {
        runs::insert_session(
            c,
            &runs::RunSession {
                id: session_id.clone(),
                group_id: group_id.to_string(),
                started_at: started.clone(),
                status: "running".into(),
            },
        )
    })?;

    let buf_bytes = buffer_bytes(&state);
    let ipc = state.ipc.clone();

    for m in members {
        let cmd = state
            .db
            .read(|c| cmd_repo::get(c, &m.command_id))?
            .ok_or_else(|| Error::msg("command missing from group"))?;
        let event_id = Uuid::new_v4().to_string();
        state.db.write(|c| {
            runs::insert_event(
                c,
                &runs::RunEvent {
                    id: event_id.clone(),
                    session_id: session_id.clone(),
                    command_id: cmd.id.clone(),
                    started_at: clock::now_unix(),
                    ended_at: None,
                    status: "starting".into(),
                    exit_code: None,
                    pid: None,
                },
            )
        })?;

        let app2 = app.clone();
        let event_id2 = event_id.clone();
        let session_id2 = session_id.clone();
        let spawned = spawn::spawn(
            cmd.is_shell,
            &cmd.execution_string,
            80,
            24,
            buf_bytes,
            event_id.clone(),
            ipc.clone(),
            move |code| {
                let state = app2.state::<AppState>();
                let status = match code {
                    Some(0) => "success",
                    Some(_) => "failed",
                    None => "stopped",
                };
                let _ = state.db.write(|c| {
                    runs::mark_event_ended(c, &event_id2, status, code, &clock::now_unix())
                });
                state.processes.remove(&event_id2);
                if state.processes.ids_for_session(&session_id2).is_empty() {
                    let _ = state.db.write(|c| runs::set_session_status(c, &session_id2, "ended"));
                }
                let _ = app2.emit(
                    events::PROCESS_STATUS,
                    StatusEvent {
                        run_event_id: event_id2,
                        status: status.into(),
                        exit_code: code,
                        pid: None,
                    },
                );
            },
        )?;

        state.db.write(|c| runs::mark_event_started(c, &event_id, spawned.pid as i64))?;
        let _ = app.emit(
            events::PROCESS_STATUS,
            StatusEvent {
                run_event_id: event_id.clone(),
                status: "running".into(),
                exit_code: None,
                pid: Some(spawned.pid),
            },
        );
        state.processes.insert(Live {
            run_event_id: event_id,
            command_id: cmd.id,
            session_id: session_id.clone(),
            group_id: group_id.to_string(),
            pid: spawned.pid,
            pty: spawned.pty,
        });
    }
    Ok(session_id)
}

#[tauri::command]
pub fn session_start(app: AppHandle, group_id: String) -> Result<String> {
    start_group_inner(&app, &group_id)
}

#[tauri::command]
pub fn session_stop(state: State<AppState>, session_id: String) -> Result<()> {
    for id in state.processes.ids_for_session(&session_id) {
        if let Some(live) = state.processes.get(&id) {
            shutdown::stop_pid(live.pid);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn process_stop(state: State<AppState>, run_event_id: String) -> Result<()> {
    let live = state
        .processes
        .get(&run_event_id)
        .ok_or_else(|| Error::msg("process not running"))?;
    shutdown::stop_pid(live.pid);
    Ok(())
}

#[tauri::command]
pub fn pty_resize(state: State<AppState>, run_event_id: String, cols: u16, rows: u16) -> Result<()> {
    let live = state
        .processes
        .get(&run_event_id)
        .ok_or_else(|| Error::msg("process not running"))?;
    live.pty.resize(cols, rows)
}

#[tauri::command]
pub fn pty_write(state: State<AppState>, run_event_id: String, b64: String) -> Result<()> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| Error::msg(e.to_string()))?;
    let live = state
        .processes
        .get(&run_event_id)
        .ok_or_else(|| Error::msg("process not running"))?;
    live.pty.write(&bytes)
}

#[tauri::command]
pub fn pty_reattach(state: State<AppState>, run_event_id: String) -> Result<String> {
    let live = state
        .processes
        .get(&run_event_id)
        .ok_or_else(|| Error::msg("process not running"))?;
    let snap = live.pty.snapshot()?;
    Ok(base64::engine::general_purpose::STANDARD.encode(snap))
}

#[tauri::command]
pub fn backup_export(state: State<AppState>, dest: String) -> Result<()> {
    backup::export::export(&state.db, dest.as_ref())
}

#[tauri::command]
pub fn backup_import(state: State<AppState>, src: String, confirmed: bool) -> Result<()> {
    backup::import::import(&state.db, &state.processes, src.as_ref(), confirmed)
}

#[tauri::command]
pub fn settings_get(state: State<AppState>) -> Result<Vec<(String, String)>> {
    state.db.read(settings::all)
}

#[tauri::command]
pub fn settings_set(state: State<AppState>, key: String, value: String) -> Result<()> {
    state.db.write(|c| settings::set(c, &key, &value))
}

#[tauri::command]
pub fn autostart_os_is_enabled(app: AppHandle) -> Result<bool> {
    app::autostart::os_enabled(&app)
}

#[tauri::command]
pub fn autostart_os_set(app: AppHandle, enabled: bool) -> Result<()> {
    app::autostart::os_set(&app, enabled)
}

#[tauri::command]
pub fn app_shutdown(app: AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let secs = shutdown_secs(&state);
    let _ = app.emit(events::SHUTDOWN_PROGRESS, "stopping");
    shutdown::shutdown_all(&state.processes, std::time::Duration::from_secs(secs));
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.close();
    }
    Ok(())
}
