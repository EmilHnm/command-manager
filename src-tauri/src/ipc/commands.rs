use crate::app;
use crate::backup;
use crate::clock;
use crate::db::repos::commands::CommandDef;
use crate::db::repos::groups::{Group, Membership};
use crate::db::repos::{
    commands as cmd_repo, groups as group_repo, history, runs, settings, templates as template_repo,
};
use crate::error::{Error, Result};
use crate::ipc::events;
use crate::process::manager::{Live, LiveSnapshot};
use crate::process::{shutdown, spawn};
use crate::template;
use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use super::AppState;

#[cfg(all(test, feature = "ipc-tests"))]
mod terminal_ipc_tests;

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

fn history_shell_kind(is_shell: bool) -> &'static str {
    if is_shell {
        "shell"
    } else {
        "argv"
    }
}

#[tauri::command]
pub fn commands_list(state: State<AppState>) -> Result<Vec<CommandDef>> {
    state.db.read(cmd_repo::list)
}

#[tauri::command(rename_all = "snake_case")]
pub fn commands_create(
    state: State<AppState>,
    name: String,
    execution_string: String,
    is_shell: bool,
    shell_kind: Option<String>,
    confirmed: bool,
) -> Result<CommandDef> {
    require(confirmed)?;
    let cmd = CommandDef {
        id: Uuid::new_v4().to_string(),
        name,
        execution_string,
        is_shell,
        shell_kind,
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

fn template_record_from_payload(
    payload: template_repo::TemplatePayload,
    id: String,
    last_run_at: Option<String>,
) -> template_repo::TemplateRecord {
    template_repo::TemplateRecord {
        id,
        name: payload.name,
        description: payload.description,
        template_string: payload.template_string,
        is_shell: payload.is_shell,
        last_run_at,
        params: payload.params,
        presets: Vec::new(),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn templates_list(state: State<AppState>) -> Result<Vec<template_repo::TemplateRecord>> {
    state.db.read(template_repo::list)
}

#[derive(Serialize, Clone)]
pub struct TemplatePreview {
    pub rendered: String,
    pub masked_rendered: String,
    pub is_shell: bool,
    pub validation_errors: Option<HashMap<String, String>>,
    pub warnings: Vec<String>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn template_preview(
    state: State<AppState>,
    template_id: String,
    values: HashMap<String, String>,
) -> Result<TemplatePreview> {
    let template = state
        .db
        .read(|c| template_repo::get(c, &template_id))?
        .ok_or_else(|| Error::msg("template not found"))?;
    let validation_errors = template::validation_errors(&template, &values);
    if !validation_errors.is_empty() {
        return Ok(TemplatePreview {
            rendered: template.template_string.clone(),
            masked_rendered: template.template_string,
            is_shell: template.is_shell,
            validation_errors: Some(validation_errors),
            warnings: Vec::new(),
        });
    }
    match template::render(&template, &values) {
        Ok(rendered) => Ok(TemplatePreview {
            rendered: rendered.masked_command_line.clone(),
            masked_rendered: rendered.masked_command_line,
            is_shell: template.is_shell,
            validation_errors: None,
            warnings: rendered.warnings,
        }),
        Err(error) => Ok(TemplatePreview {
            rendered: template.template_string.clone(),
            masked_rendered: template.template_string,
            is_shell: template.is_shell,
            validation_errors: Some(HashMap::from([(
                String::from("_template"),
                error.to_string(),
            )])),
            warnings: Vec::new(),
        }),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn templates_create(
    state: State<AppState>,
    template: template_repo::TemplatePayload,
    confirmed: bool,
) -> Result<template_repo::TemplateRecord> {
    require(confirmed)?;
    if template.name.trim().is_empty() || template.template_string.trim().is_empty() {
        return Err(Error::msg("template name and template string are required"));
    }
    let record = template_record_from_payload(template, Uuid::new_v4().to_string(), None);
    state.db.write(|c| template_repo::insert(c, &record))?;
    Ok(record)
}

#[tauri::command(rename_all = "snake_case")]
pub fn templates_update(
    state: State<AppState>,
    template: template_repo::TemplatePayload,
    confirmed: bool,
) -> Result<()> {
    require(confirmed)?;
    let id = template
        .id
        .clone()
        .ok_or_else(|| Error::msg("template id is required"))?;
    if template.name.trim().is_empty() || template.template_string.trim().is_empty() {
        return Err(Error::msg("template name and template string are required"));
    }
    state.db.write(|c| {
        let existing =
            template_repo::get(c, &id)?.ok_or_else(|| Error::msg("template not found"))?;
        template_repo::update(
            c,
            &template_record_from_payload(template, id, existing.last_run_at),
        )
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn templates_delete(state: State<AppState>, id: String, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| template_repo::delete(c, &id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn template_preset_save(
    state: State<AppState>,
    template_id: String,
    name: String,
    values: HashMap<String, String>,
    confirmed: bool,
) -> Result<template_repo::TemplatePreset> {
    require(confirmed)?;
    let preset = state.db.write(|c| {
        let template =
            template_repo::get(c, &template_id)?.ok_or_else(|| Error::msg("template not found"))?;
        let allowed: std::collections::HashSet<String> = template
            .params
            .into_iter()
            .filter(|param| !param.is_secret)
            .map(|param| param.name)
            .collect();
        let clean_values = values
            .into_iter()
            .filter(|(key, _)| allowed.contains(key))
            .collect();
        let preset = template_repo::TemplatePreset {
            id: Uuid::new_v4().to_string(),
            template_id,
            name: name.trim().to_string(),
            values: clean_values,
            last_used_at: Some(clock::now_unix()),
        };
        if preset.name.is_empty() {
            return Err(Error::msg("preset name is required"));
        }
        template_repo::save_preset(c, &preset)?;
        Ok(preset)
    })?;
    Ok(preset)
}

#[tauri::command(rename_all = "snake_case")]
pub fn template_preset_delete(state: State<AppState>, id: String, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| template_repo::delete_preset(c, &id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn template_run(
    app: AppHandle,
    template_id: String,
    values: HashMap<String, String>,
    confirmed: bool,
) -> Result<TerminalInfo> {
    require(confirmed)?;
    let state = app.state::<AppState>();
    let template = state
        .db
        .read(|c| template_repo::get(c, &template_id))?
        .ok_or_else(|| Error::msg("template not found"))?;
    let rendered = template::render(&template, &values)?;
    let run_event_id = Uuid::new_v4().to_string();
    let session_id = format!("template-session:{run_event_id}");
    let command_id = format!("template:{template_id}:{run_event_id}");
    let buf_bytes = buffer_bytes(&state);
    let ipc = state.ipc.clone();

    let history_id = match state.db.write(|c| {
        runs::insert_session(
            c,
            &runs::RunSession {
                id: session_id.clone(),
                group_id: None,
                template_id: Some(template_id.clone()),
                template_name: Some(template.name.clone()),
                started_at: clock::now_unix(),
                status: "running".into(),
            },
        )?;
        runs::insert_event(
            c,
            &runs::RunEvent {
                id: run_event_id.clone(),
                session_id: session_id.clone(),
                command_id: command_id.clone(),
                command_name: None,
                started_at: clock::now_unix(),
                ended_at: None,
                status: "starting".into(),
                exit_code: None,
                pid: None,
            },
        )?;
        history::record(
            c,
            &rendered.masked_command_line,
            history_shell_kind(template.is_shell),
            None,
            "template",
            &clock::now_unix(),
        )
    }) {
        Ok(history_id) => history_id,
        Err(error) => {
            let _ = state.db.write(|c| {
                let ended_at = clock::now_unix();
                runs::mark_event_ended(c, &run_event_id, "failed", None, &ended_at)?;
                runs::set_session_status(c, &session_id, "failed")
            });
            return Err(error);
        }
    };
    if let Err(error) = state
        .db
        .write(|c| template_repo::mark_run(c, &template_id, &clock::now_unix()))
    {
        let _ = state.db.write(|c| {
            let ended_at = clock::now_unix();
            runs::mark_event_ended(c, &run_event_id, "failed", None, &ended_at)?;
            let _ = history::finish(c, &history_id, None, &ended_at);
            runs::set_session_status(c, &session_id, "failed")
        });
        return Err(error);
    }

    let app2 = app.clone();
    let run_event_id2 = run_event_id.clone();
    let command_id2 = command_id.clone();
    let session_id2 = session_id.clone();
    let history_id2 = history_id.clone();
    let on_exit = move |code| {
        let state = app2.state::<AppState>();
        let status = match code {
            Some(0) => "success",
            Some(_) => "failed",
            None => "stopped",
        };
        let _ = state.db.write(|c| {
            let ended_at = clock::now_unix();
            runs::mark_event_ended(c, &run_event_id2, status, code, &ended_at)?;
            let _ = history::finish(c, &history_id2, code, &ended_at);
            runs::finalize_session(c, &session_id2)
        });
        state.processes.remove(&run_event_id2);
        let _ = app2.emit(
            events::PROCESS_STATUS,
            StatusEvent {
                run_event_id: run_event_id2,
                command_id: command_id2,
                session_id: session_id2,
                status: status.into(),
                exit_code: code,
                pid: None,
            },
        );
    };
    state.processes.reserve(&run_event_id);
    let spawned = match if let Some(argv) = rendered.argv.as_ref() {
        spawn::spawn_argv(argv, 100, 30, buf_bytes, run_event_id.clone(), ipc, on_exit)
    } else {
        spawn::spawn(
            true,
            &rendered.command_line,
            None,
            100,
            30,
            buf_bytes,
            run_event_id.clone(),
            ipc,
            on_exit,
        )
    } {
        Ok(spawned) => spawned,
        Err(error) => {
            state.processes.cancel(&run_event_id);
            let _ = state.db.write(|c| {
                let ended_at = clock::now_unix();
                runs::mark_event_ended(c, &run_event_id, "failed", None, &ended_at)?;
                let _ = history::finish(c, &history_id, None, &ended_at);
                runs::finalize_session(c, &session_id)
            });
            return Err(error);
        }
    };

    let pid = spawned.pid;
    let started = match state
        .db
        .write(|c| runs::mark_event_started(c, &run_event_id, pid as i64))
    {
        Ok(started) => started,
        Err(error) => {
            let mut killer = spawned.killer;
            let _ = killer.kill();
            state.processes.cancel(&run_event_id);
            return Err(error);
        }
    };
    if !started {
        state.processes.cancel(&run_event_id);
        return Ok(TerminalInfo {
            run_event_id,
            command_id,
            session_id,
            pid,
            shell_kind: spawned.shell_kind,
            history_level: spawned.history_level,
        });
    }
    state.processes.insert(Live {
        run_event_id: run_event_id.clone(),
        command_id: command_id.clone(),
        session_id: session_id.clone(),
        group_id: "template".into(),
        pid,
        pty: spawned.pty,
        shell_kind: spawned.shell_kind.clone(),
        history_level: spawned.history_level,
    });
    let _ = app.emit(
        events::PROCESS_STATUS,
        StatusEvent {
            run_event_id: run_event_id.clone(),
            command_id: command_id.clone(),
            session_id: session_id.clone(),
            status: "running".into(),
            exit_code: None,
            pid: Some(pid),
        },
    );

    Ok(TerminalInfo {
        run_event_id,
        command_id,
        session_id,
        pid,
        shell_kind: spawned.shell_kind.clone(),
        history_level: spawned.history_level,
    })
}

#[cfg(test)]
mod template_tests {
    use super::template_repo;
    use crate::template;
    use std::collections::HashMap;

    fn render_template(
        template: &template_repo::TemplateRecord,
        values: &HashMap<String, String>,
    ) -> crate::error::Result<String> {
        template::render(template, values).map(|rendered| rendered.command_line)
    }

    #[test]
    fn render_template_applies_defaults_and_quotes_values() {
        let template = template_repo::TemplateRecord {
            id: "template-test".into(),
            name: "Template test".into(),
            description: None,
            template_string: "echo {{message}} {{count}}".into(),
            is_shell: true,
            last_run_at: None,
            params: vec![
                template_repo::TemplateParam {
                    name: "message".into(),
                    label: "Message".into(),
                    kind: "string".into(),
                    default_value: Some("hello".into()),
                    required: true,
                    options: None,
                    is_secret: false,
                    param_order: 1,
                },
                template_repo::TemplateParam {
                    name: "count".into(),
                    label: "Count".into(),
                    kind: "number".into(),
                    default_value: Some("1".into()),
                    required: false,
                    options: None,
                    is_secret: false,
                    param_order: 2,
                },
            ],
            presets: Vec::new(),
        };
        let mut values = HashMap::new();
        values.insert("message".into(), "hello world".into());

        let rendered = render_template(&template, &values).expect("template should render");
        assert!(!rendered.contains("{{"));
        assert!(rendered.contains("hello world"));
        assert!(rendered.contains('1'));
    }

    #[test]
    fn render_template_rejects_missing_required_values() {
        let template = template_repo::TemplateRecord {
            id: "template-test".into(),
            name: "Template test".into(),
            description: None,
            template_string: "echo {{message}}".into(),
            is_shell: true,
            last_run_at: None,
            params: vec![template_repo::TemplateParam {
                name: "message".into(),
                label: "Message".into(),
                kind: "string".into(),
                default_value: None,
                required: true,
                options: None,
                is_secret: false,
                param_order: 1,
            }],
            presets: Vec::new(),
        };

        let error =
            render_template(&template, &HashMap::new()).expect_err("required value must fail");
        assert!(error
            .to_string()
            .contains("missing required parameter: message"));
    }

    #[test]
    fn render_template_does_not_nest_quotes_around_existing_path_quotes() {
        let template = template_repo::TemplateRecord {
            id: "template-rclone".into(),
            name: "Rclone copy".into(),
            description: None,
            template_string: "rclone copy \"{{source}}\" \"{{destination}}\"".into(),
            is_shell: true,
            last_run_at: None,
            params: vec![
                template_repo::TemplateParam {
                    name: "source".into(),
                    label: "Source".into(),
                    kind: "path".into(),
                    default_value: None,
                    required: true,
                    options: None,
                    is_secret: false,
                    param_order: 1,
                },
                template_repo::TemplateParam {
                    name: "destination".into(),
                    label: "Destination".into(),
                    kind: "path".into(),
                    default_value: None,
                    required: true,
                    options: None,
                    is_secret: false,
                    param_order: 2,
                },
            ],
            presets: Vec::new(),
        };
        let values = HashMap::from([
            ("source".into(), r"F:\[BDMV] Re ZERO Starting Life".into()),
            (
                "destination".into(),
                "hmnw:bd/[BDMV] Re ZERO Starting Life".into(),
            ),
        ]);

        let rendered = render_template(&template, &values).expect("template should render");
        assert!(!rendered.contains("{{"));
        assert!(!rendered.contains("'F:\\"));
        assert!(!rendered.contains("\"'hmnw:"));
    }
}

#[tauri::command]
pub fn groups_list(state: State<AppState>) -> Result<Vec<Group>> {
    state.db.read(group_repo::list)
}

#[tauri::command(rename_all = "snake_case")]
pub fn groups_create(
    state: State<AppState>,
    group_name: String,
    execution_mode: Option<String>,
    confirmed: bool,
) -> Result<Group> {
    require(confirmed)?;
    let g = Group {
        id: Uuid::new_v4().to_string(),
        group_name,
        autostart: false,
        execution_mode: match execution_mode.as_deref() {
            Some("sequential") => "sequential".into(),
            _ => "startup".into(),
        },
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

#[tauri::command(rename_all = "snake_case")]
pub fn groups_memberships(state: State<AppState>, group_id: String) -> Result<Vec<Membership>> {
    state.db.read(|c| group_repo::memberships(c, &group_id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn groups_set_memberships(
    state: State<AppState>,
    group_id: String,
    members: Vec<Membership>,
    confirmed: bool,
) -> Result<()> {
    require(confirmed)?;
    state
        .db
        .write(|c| group_repo::replace_memberships(c, &group_id, &members))
}

#[tauri::command(rename_all = "snake_case")]
pub fn groups_set_autostart(
    state: State<AppState>,
    group_id: String,
    autostart: bool,
    confirmed: bool,
) -> Result<()> {
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

#[tauri::command(rename_all = "snake_case")]
pub fn session_events(state: State<AppState>, session_id: String) -> Result<Vec<runs::RunEvent>> {
    state.db.read(|c| runs::list_events(c, &session_id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn history_list(
    state: State<AppState>,
    query: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<history::CommandHistory>> {
    state
        .db
        .read(|c| history::list(c, query.as_deref(), limit.unwrap_or(500)))
}

#[tauri::command(rename_all = "snake_case")]
pub fn history_delete(state: State<AppState>, id: String, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| history::delete(c, &id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn history_clear(state: State<AppState>, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    state.db.write(history::clear)
}

#[derive(Serialize, Clone)]
pub struct OsHistorySource {
    pub shell_kinds: Vec<String>,
    pub path: String,
    pub entries: usize,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct OsHistoryImport {
    pub shell_kind: String,
    pub path: String,
    pub imported: usize,
    pub skipped: usize,
}

/// History files of the user's own shells, with the number of distinct
/// commands each would contribute.
#[tauri::command]
pub async fn history_os_sources() -> Result<Vec<OsHistorySource>> {
    tauri::async_runtime::spawn_blocking(|| {
        crate::os_history::detect()
            .into_iter()
            .map(|source| {
                let parsed = crate::os_history::read(&source);
                OsHistorySource {
                    shell_kinds: source.shell_kinds.iter().map(|s| s.to_string()).collect(),
                    path: source.path.to_string_lossy().into_owned(),
                    entries: parsed.as_ref().map(Vec::len).unwrap_or(0),
                    error: parsed.err().map(|error| error.to_string()),
                }
            })
            .collect()
    })
    .await
    .map_err(|error| Error::msg(format!("history worker failed: {error}")))
}

/// Copy the OS shell history into `command_history` so suggestions can use
/// it. The shell's own files are only read.
#[tauri::command(rename_all = "snake_case")]
pub async fn history_import_os(app: AppHandle, confirmed: bool) -> Result<Vec<OsHistoryImport>> {
    require(confirmed)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut report = Vec::new();
        for source in crate::os_history::detect() {
            let entries = crate::os_history::read(&source)
                .map_err(|error| Error::msg(format!("{}: {error}", source.path.display())))?;
            for shell_kind in &source.shell_kinds {
                let counts = state
                    .db
                    .write(|c| history::import(c, shell_kind, &entries))?;
                report.push(OsHistoryImport {
                    shell_kind: shell_kind.to_string(),
                    path: source.path.to_string_lossy().into_owned(),
                    imported: counts.imported,
                    skipped: counts.skipped,
                });
            }
        }
        if report.iter().any(|row| row.imported > 0) {
            let _ = app.emit(events::HISTORY_ADDED, serde_json::json!({ "id": null }));
        }
        Ok(report)
    })
    .await
    .map_err(|error| Error::msg(format!("history worker failed: {error}")))?
}

/// Record a command typed directly into a level-2 terminal. Rich shells emit
/// OSC 633 E/C/D records and are recorded by the PTY tracker instead.
#[tauri::command(rename_all = "snake_case")]
pub fn history_record_typed(
    app: AppHandle,
    run_event_id: String,
    command_line: String,
    cwd: Option<String>,
) -> Result<()> {
    let state = app.state::<AppState>();
    let live = state
        .processes
        .get(&run_event_id)
        .ok_or_else(|| Error::msg("terminal not running"))?;
    if live.history_level != 2 {
        return Ok(());
    }
    let at = clock::now_unix();
    let id = state.db.write(|c| {
        history::record(
            c,
            &command_line,
            &live.shell_kind,
            cwd.as_deref(),
            "typed",
            &at,
        )
    })?;
    if !id.is_empty() {
        let _ = app.emit(events::HISTORY_ADDED, serde_json::json!({ "id": id }));
    }
    Ok(())
}

#[tauri::command]
pub fn process_list(state: State<AppState>) -> Result<Vec<LiveSnapshot>> {
    Ok(state.processes.snapshot())
}

#[derive(Serialize, Clone)]
struct StatusEvent {
    run_event_id: String,
    command_id: String,
    session_id: String,
    status: String,
    exit_code: Option<i32>,
    pid: Option<u32>,
}

#[derive(Serialize, Clone)]
pub struct TerminalInfo {
    pub run_event_id: String,
    pub command_id: String,
    pub session_id: String,
    pub pid: u32,
    pub shell_kind: String,
    pub history_level: u8,
}

fn start_group_member(
    app: &AppHandle,
    session_id: &str,
    group_id: &str,
    member: Membership,
) -> Result<std::sync::mpsc::Receiver<Option<i32>>> {
    let state = app.state::<AppState>();
    let cmd = state
        .db
        .read(|c| cmd_repo::get(c, &member.command_id))?
        .ok_or_else(|| Error::msg("command missing from group"))?;
    let event_id = Uuid::new_v4().to_string();
    let history_id = state.db.write(|c| {
        runs::insert_event(
            c,
            &runs::RunEvent {
                id: event_id.clone(),
                session_id: session_id.to_string(),
                command_id: cmd.id.clone(),
                command_name: None,
                started_at: clock::now_unix(),
                ended_at: None,
                status: "starting".into(),
                exit_code: None,
                pid: None,
            },
        )?;
        history::record(
            c,
            &cmd.execution_string,
            cmd.shell_kind
                .as_deref()
                .unwrap_or_else(|| history_shell_kind(cmd.is_shell)),
            None,
            "command",
            &clock::now_unix(),
        )
    })?;
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let app2 = app.clone();
    let event_id2 = event_id.clone();
    let session_id2 = session_id.to_string();
    let command_id2 = cmd.id.clone();
    let history_id2 = history_id.clone();
    state.processes.reserve(&event_id);
    let spawned = match spawn::spawn(
        cmd.is_shell,
        &cmd.execution_string,
        cmd.shell_kind.as_deref(),
        80,
        24,
        buffer_bytes(&state),
        event_id.clone(),
        state.ipc.clone(),
        move |code| {
            let state = app2.state::<AppState>();
            let status = match code {
                Some(0) => "success",
                Some(_) => "failed",
                None => "stopped",
            };
            let _ = state.db.write(|c| {
                let ended_at = clock::now_unix();
                runs::mark_event_ended(c, &event_id2, status, code, &ended_at)?;
                let _ = history::finish(c, &history_id2, code, &ended_at);
                Ok(())
            });
            state.processes.remove(&event_id2);
            let _ = app2.emit(
                events::PROCESS_STATUS,
                StatusEvent {
                    run_event_id: event_id2,
                    command_id: command_id2,
                    session_id: session_id2,
                    status: status.into(),
                    exit_code: code,
                    pid: None,
                },
            );
            let _ = done_tx.send(code);
        },
    ) {
        Ok(spawned) => spawned,
        Err(error) => {
            state.processes.cancel(&event_id);
            let ended_at = clock::now_unix();
            let _ = state.db.write(|c| {
                runs::mark_event_ended(c, &event_id, "failed", None, &ended_at)?;
                let _ = history::finish(c, &history_id, None, &ended_at);
                Ok(())
            });
            return Err(error);
        }
    };
    let started = match state
        .db
        .write(|c| runs::mark_event_started(c, &event_id, spawned.pid as i64))
    {
        Ok(started) => started,
        Err(error) => {
            let mut killer = spawned.killer;
            let _ = killer.kill();
            state.processes.cancel(&event_id);
            return Err(error);
        }
    };
    if !started {
        state.processes.cancel(&event_id);
        return Ok(done_rx);
    }
    let pid = spawned.pid;
    state.processes.insert(Live {
        run_event_id: event_id.clone(),
        command_id: cmd.id.clone(),
        session_id: session_id.to_string(),
        group_id: group_id.to_string(),
        pid,
        pty: spawned.pty,
        shell_kind: spawned.shell_kind,
        history_level: spawned.history_level,
    });
    let _ = app.emit(
        events::PROCESS_STATUS,
        StatusEvent {
            run_event_id: event_id,
            command_id: cmd.id,
            session_id: session_id.to_string(),
            status: "running".into(),
            exit_code: None,
            pid: Some(pid),
        },
    );
    Ok(done_rx)
}

pub fn start_group_inner(app: &AppHandle, group_id: &str) -> Result<String> {
    let state = app.state::<AppState>();
    let members = state.db.read(|c| group_repo::memberships(c, group_id))?;
    let execution_mode = state
        .db
        .read(group_repo::list)?
        .into_iter()
        .find(|group| group.id == group_id)
        .map(|group| group.execution_mode)
        .unwrap_or_else(|| "startup".into());
    if members.is_empty() {
        return Err(Error::msg("group has no commands"));
    }
    let session_id = Uuid::new_v4().to_string();
    state.db.write(|c| {
        runs::insert_session(
            c,
            &runs::RunSession {
                id: session_id.clone(),
                group_id: Some(group_id.to_string()),
                template_id: None,
                template_name: None,
                started_at: clock::now_unix(),
                status: "running".into(),
            },
        )
    })?;

    let app2 = app.clone();
    let session_id2 = session_id.clone();
    let group_id2 = group_id.to_string();
    std::thread::spawn(move || {
        let sequential = execution_mode == "sequential";
        let mut completions = Vec::new();

        for member in members {
            let state = app2.state::<AppState>();
            let still_running = state
                .db
                .read(|c| runs::session_status(c, &session_id2))
                .ok()
                .flatten()
                .is_some_and(|status| status == "running");
            if !still_running {
                return;
            }

            let done = match start_group_member(&app2, &session_id2, &group_id2, member) {
                Ok(done) => done,
                Err(error) => {
                    let state = app2.state::<AppState>();
                    let _ = state.db.write(|c| {
                        if runs::session_status(c, &session_id2)?.as_deref() == Some("running") {
                            runs::set_session_status(c, &session_id2, "failed")
                        } else {
                            Ok(())
                        }
                    });
                    eprintln!("[group {group_id2}] không thể khởi chạy bước: {error}");
                    return;
                }
            };

            if sequential {
                let code = done.recv().ok().flatten();
                if code != Some(0) {
                    let state = app2.state::<AppState>();
                    let status = if code.is_none() { "stopped" } else { "failed" };
                    let _ = state.db.write(|c| {
                        if runs::session_status(c, &session_id2)?.as_deref() == Some("running") {
                            runs::set_session_status(c, &session_id2, status)
                        } else {
                            Ok(())
                        }
                    });
                    return;
                }
            } else {
                // Startup mode launches in execution_order but does not wait
                // for long-running daemons before starting the next member.
                completions.push(done);
            }
        }

        if !sequential {
            let mut failed = false;
            let mut stopped = false;
            for done in completions {
                match done.recv().ok().flatten() {
                    Some(0) => {}
                    Some(_) => failed = true,
                    None => stopped = true,
                }
            }
            let state = app2.state::<AppState>();
            let _ = state.db.write(|c| {
                let current = runs::session_status(c, &session_id2)?.unwrap_or_default();
                if current == "running" {
                    if stopped {
                        runs::set_session_status(c, &session_id2, "stopped")
                    } else if failed {
                        runs::set_session_status(c, &session_id2, "failed")
                    } else {
                        runs::finalize_session(c, &session_id2)
                    }
                } else {
                    Ok(())
                }
            });
        } else {
            let state = app2.state::<AppState>();
            let _ = state.db.write(|c| runs::finalize_session(c, &session_id2));
        }
    });
    Ok(session_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn session_start(app: AppHandle, group_id: String) -> Result<String> {
    start_group_inner(&app, &group_id)
}

/// Run one saved command immediately from the Command Library. This uses the
/// same in-memory PTY/process lifecycle as group runs without inventing a
/// database group just for a single command.
#[tauri::command(rename_all = "snake_case")]
pub fn command_run(app: AppHandle, command_id: String) -> Result<TerminalInfo> {
    let state = app.state::<AppState>();
    let cmd = state
        .db
        .read(|c| cmd_repo::get(c, &command_id))?
        .ok_or_else(|| Error::msg("command not found"))?;
    let run_event_id = Uuid::new_v4().to_string();
    let session_id = format!("command-session:{run_event_id}");
    let buf_bytes = buffer_bytes(&state);
    let ipc = state.ipc.clone();

    let history_id = match state.db.write(|c| {
        runs::insert_session(
            c,
            &runs::RunSession {
                id: session_id.clone(),
                group_id: None,
                template_id: None,
                template_name: None,
                started_at: clock::now_unix(),
                status: "running".into(),
            },
        )?;
        runs::insert_event(
            c,
            &runs::RunEvent {
                id: run_event_id.clone(),
                session_id: session_id.clone(),
                command_id: command_id.clone(),
                command_name: None,
                started_at: clock::now_unix(),
                ended_at: None,
                status: "starting".into(),
                exit_code: None,
                pid: None,
            },
        )?;
        history::record(
            c,
            &cmd.execution_string,
            cmd.shell_kind
                .as_deref()
                .unwrap_or_else(|| history_shell_kind(cmd.is_shell)),
            None,
            "command",
            &clock::now_unix(),
        )
    }) {
        Ok(history_id) => history_id,
        Err(error) => {
            let _ = state.db.write(|c| {
                let ended_at = clock::now_unix();
                runs::mark_event_ended(c, &run_event_id, "failed", None, &ended_at)?;
                runs::set_session_status(c, &session_id, "failed")
            });
            return Err(error);
        }
    };

    let app2 = app.clone();
    let run_event_id2 = run_event_id.clone();
    let command_id2 = command_id.clone();
    let session_id2 = session_id.clone();
    let history_id2 = history_id.clone();
    state.processes.reserve(&run_event_id);
    let spawned = match spawn::spawn(
        cmd.is_shell,
        &cmd.execution_string,
        cmd.shell_kind.as_deref(),
        100,
        30,
        buf_bytes,
        run_event_id.clone(),
        ipc,
        move |code| {
            let state = app2.state::<AppState>();
            let status = match code {
                Some(0) => "success",
                Some(_) => "failed",
                None => "stopped",
            };
            let _ = state.db.write(|c| {
                let ended_at = clock::now_unix();
                runs::mark_event_ended(c, &run_event_id2, status, code, &ended_at)?;
                let _ = history::finish(c, &history_id2, code, &ended_at);
                runs::finalize_session(c, &session_id2)
            });
            state.processes.remove(&run_event_id2);
            let _ = app2.emit(
                events::PROCESS_STATUS,
                StatusEvent {
                    run_event_id: run_event_id2,
                    command_id: command_id2,
                    session_id: session_id2,
                    status: status.into(),
                    exit_code: code,
                    pid: None,
                },
            );
        },
    ) {
        Ok(spawned) => spawned,
        Err(error) => {
            state.processes.cancel(&run_event_id);
            let _ = state.db.write(|c| {
                let ended_at = clock::now_unix();
                runs::mark_event_ended(c, &run_event_id, "failed", None, &ended_at)?;
                let _ = history::finish(c, &history_id, None, &ended_at);
                runs::finalize_session(c, &session_id)
            });
            return Err(error);
        }
    };

    let pid = spawned.pid;
    let started = match state
        .db
        .write(|c| runs::mark_event_started(c, &run_event_id, pid as i64))
    {
        Ok(started) => started,
        Err(error) => {
            let mut killer = spawned.killer;
            let _ = killer.kill();
            state.processes.cancel(&run_event_id);
            return Err(error);
        }
    };
    if !started {
        state.processes.cancel(&run_event_id);
        return Ok(TerminalInfo {
            run_event_id,
            command_id,
            session_id,
            pid,
            shell_kind: spawned.shell_kind,
            history_level: spawned.history_level,
        });
    }
    state.processes.insert(Live {
        run_event_id: run_event_id.clone(),
        command_id: command_id.clone(),
        session_id: session_id.clone(),
        group_id: "command".into(),
        pid,
        pty: spawned.pty,
        shell_kind: spawned.shell_kind.clone(),
        history_level: spawned.history_level,
    });
    let _ = app.emit(
        events::PROCESS_STATUS,
        StatusEvent {
            run_event_id: run_event_id.clone(),
            command_id: command_id.clone(),
            session_id: session_id.clone(),
            status: "running".into(),
            exit_code: None,
            pid: Some(pid),
        },
    );

    Ok(TerminalInfo {
        run_event_id,
        command_id,
        session_id,
        pid,
        shell_kind: spawned.shell_kind,
        history_level: spawned.history_level,
    })
}

/// Open an interactive shell that is not persisted as a saved command or run
/// history event. It still uses the same in-memory process manager and PTY
/// stream as command-backed terminals, so input, resize, reattach, and stop
/// behave consistently.
#[tauri::command]
pub async fn terminal_open(app: AppHandle, cwd: Option<String>) -> Result<TerminalInfo> {
    tauri::async_runtime::spawn_blocking(move || terminal_open_sync(app, cwd))
        .await
        .map_err(|error| Error::msg(format!("terminal worker failed: {error}")))?
}

fn terminal_open_sync(app: AppHandle, cwd: Option<String>) -> Result<TerminalInfo> {
    let cwd = cwd
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_dir());
    let state = app.state::<AppState>();
    let terminal_id = Uuid::new_v4().to_string();
    let run_event_id = format!("terminal:{terminal_id}");
    let command_id = run_event_id.clone();
    let session_id = format!("terminal-session:{terminal_id}");
    let buf_bytes = buffer_bytes(&state);
    let ipc = state.ipc.clone();
    let integration_root = app.path().app_data_dir().ok();
    let preferred_shell = state
        .db
        .read(|c| settings::get(c, "terminal_shell"))
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty());
    let load_powershell_profile = state
        .db
        .read(|c| settings::get(c, "terminal_load_profile"))
        .ok()
        .flatten()
        .is_none_or(|value| value != "false");

    let app2 = app.clone();
    let run_event_id2 = run_event_id.clone();
    let command_id2 = command_id.clone();
    let session_id2 = session_id.clone();
    state.processes.reserve(&run_event_id);
    let spawned = match spawn::spawn_interactive(
        100,
        30,
        buf_bytes,
        run_event_id.clone(),
        ipc,
        integration_root,
        preferred_shell,
        load_powershell_profile,
        cwd,
        move |code| {
            let state = app2.state::<AppState>();
            state.processes.remove(&run_event_id2);
            let status = match code {
                Some(0) => "success",
                Some(_) => "failed",
                None => "stopped",
            };
            let _ = app2.emit(
                events::PROCESS_STATUS,
                StatusEvent {
                    run_event_id: run_event_id2,
                    command_id: command_id2,
                    session_id: session_id2,
                    status: status.into(),
                    exit_code: code,
                    pid: None,
                },
            );
        },
        {
            let app = app.clone();
            move |record| {
                let state = app.state::<AppState>();
                let at = clock::now_unix();
                let record_id = state.db.write(|c| {
                    let id = history::record(
                        c,
                        &record.command_line,
                        &record.shell_kind,
                        record.cwd.as_deref(),
                        "shell",
                        &at,
                    )?;
                    history::finish(c, &id, record.exit_code, &at)?;
                    Ok(id)
                });
                if let Ok(id) = record_id {
                    if !id.is_empty() {
                        let _ = app.emit(events::HISTORY_ADDED, serde_json::json!({ "id": id }));
                    }
                }
            }
        },
    ) {
        Ok(spawned) => spawned,
        Err(error) => {
            state.processes.cancel(&run_event_id);
            return Err(error);
        }
    };

    let pid = spawned.pid;
    state.processes.insert(Live {
        run_event_id: run_event_id.clone(),
        command_id: command_id.clone(),
        session_id: session_id.clone(),
        group_id: "terminal".into(),
        pid,
        pty: spawned.pty,
        shell_kind: spawned.shell_kind.clone(),
        history_level: spawned.history_level,
    });
    let _ = app.emit(
        events::PROCESS_STATUS,
        StatusEvent {
            run_event_id: run_event_id.clone(),
            command_id: command_id.clone(),
            session_id: session_id.clone(),
            status: "running".into(),
            exit_code: None,
            pid: Some(pid),
        },
    );

    Ok(TerminalInfo {
        run_event_id,
        command_id,
        session_id,
        pid,
        shell_kind: spawned.shell_kind,
        history_level: spawned.history_level,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn session_stop(app: AppHandle, session_id: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .db
            .write(|c| runs::set_session_status(c, &session_id, "stopped"))?;
        let session_pids: Vec<u32> = state
            .processes
            .ids_for_session(&session_id)
            .into_iter()
            .filter_map(|id| state.processes.get(&id).map(|live| live.pid))
            .collect();

        // Signal all processes in the session up front so shutdown happens concurrently
        for pid in &session_pids {
            shutdown::stop_pid(*pid);
        }

        let mut first_error = None;
        for pid in session_pids {
            if let Err(error) = shutdown::stop_and_wait(pid, false) {
                first_error.get_or_insert(error);
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    })
    .await
    .map_err(|error| Error::msg(format!("session_stop worker failed: {error}")))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn process_stop(app: AppHandle, run_event_id: String, force: Option<bool>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let live = state
            .processes
            .get(&run_event_id)
            .ok_or_else(|| Error::msg("process not running"))?;
        shutdown::stop_and_wait(live.pid, force.unwrap_or(false))?;
        Ok(())
    })
    .await
    .map_err(|error| Error::msg(format!("process_stop worker failed: {error}")))?
}

#[tauri::command(rename_all = "snake_case")]
pub fn pty_resize(
    state: State<AppState>,
    run_event_id: String,
    cols: u16,
    rows: u16,
) -> Result<()> {
    let live = state
        .processes
        .get(&run_event_id)
        .ok_or_else(|| Error::msg("process not running"))?;
    live.pty.resize(cols, rows)
}

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
pub fn pty_reattach(state: State<AppState>, run_event_id: String) -> Result<String> {
    let live = state
        .processes
        .get(&run_event_id)
        .ok_or_else(|| Error::msg("process not running"))?;
    let snap = live.pty.snapshot()?;
    Ok(base64::engine::general_purpose::STANDARD.encode(snap))
}

#[tauri::command]
pub fn backup_export(
    app: AppHandle,
    state: State<AppState>,
    dest: Option<String>,
) -> Result<String> {
    let destination = dest
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            app.path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(format!(
                    "command-manager-backup-{}.sqlite",
                    clock::now_unix()
                ))
        });
    backup::export::export(&state.db, &destination)?;
    Ok(destination.to_string_lossy().to_string())
}

#[derive(Serialize, Clone)]
pub struct BackupInfo {
    pub schema_version: i64,
    pub command_count: i64,
    pub group_count: i64,
    pub history_count: i64,
    pub integrity_ok: bool,
}

#[tauri::command(rename_all = "snake_case")]
pub fn backup_verify_bytes(b64: String) -> Result<BackupInfo> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|error| Error::msg(format!("backup data không hợp lệ: {error}")))?;
    let staged = std::env::temp_dir().join(format!(
        "command-manager-backup-inspect-{}.sqlite",
        Uuid::new_v4()
    ));
    std::fs::write(&staged, bytes)?;
    let result = (|| {
        let integrity_ok = backup::import::inspect_integrity(&staged)?;
        if !integrity_ok {
            return Ok(BackupInfo {
                schema_version: 0,
                command_count: 0,
                group_count: 0,
                history_count: 0,
                integrity_ok: false,
            });
        }
        backup::import::migrate_staged(&staged)?;
        let conn = rusqlite::Connection::open(&staged)?;
        Ok(BackupInfo {
            schema_version: crate::db::pool::read_schema_version(&staged)?,
            command_count: conn.query_row(
                "SELECT COUNT(*) FROM command_definition",
                [],
                |row| row.get(0),
            )?,
            group_count: conn
                .query_row("SELECT COUNT(*) FROM command_group", [], |row| row.get(0))?,
            history_count: conn
                .query_row("SELECT COUNT(*) FROM command_history", [], |row| row.get(0))?,
            integrity_ok: crate::db::pool::integrity_ok(&staged)?,
        })
    })();
    let _ = std::fs::remove_file(&staged);
    result
}

#[tauri::command]
pub fn backup_info(state: State<AppState>) -> Result<BackupInfo> {
    state.db.read(|conn| {
        let command_count =
            conn.query_row("SELECT COUNT(*) FROM command_definition", [], |row| {
                row.get(0)
            })?;
        let group_count =
            conn.query_row("SELECT COUNT(*) FROM command_group", [], |row| row.get(0))?;
        let history_count =
            conn.query_row("SELECT COUNT(*) FROM command_history", [], |row| row.get(0))?;
        Ok(BackupInfo {
            schema_version: crate::db::SCHEMA_VERSION,
            command_count,
            group_count,
            history_count,
            integrity_ok: true,
        })
    })
}

#[tauri::command]
pub fn backup_import(state: State<AppState>, src: String, confirmed: bool) -> Result<()> {
    backup::import::import(&state.db, &state.processes, src.as_ref(), confirmed)
}

#[tauri::command(rename_all = "snake_case")]
pub fn backup_import_bytes(state: State<AppState>, b64: String, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|error| Error::msg(format!("backup data không hợp lệ: {error}")))?;
    let staged =
        std::env::temp_dir().join(format!("command-manager-upload-{}.sqlite", Uuid::new_v4()));
    std::fs::write(&staged, bytes)?;
    let result = backup::import::import(&state.db, &state.processes, &staged, true);
    let _ = std::fs::remove_file(&staged);
    result
}

#[tauri::command]
pub fn settings_get(state: State<AppState>) -> Result<Vec<(String, String)>> {
    state.db.read(settings::all)
}

#[tauri::command]
pub fn settings_set(
    state: State<AppState>,
    key: String,
    value: String,
    confirmed: bool,
) -> Result<()> {
    require(confirmed)?;
    state.db.write(|c| settings::set(c, &key, &value))
}

#[tauri::command]
pub fn autostart_os_is_enabled(app: AppHandle) -> Result<bool> {
    app::autostart::os_enabled(&app)
}

#[tauri::command]
pub fn autostart_os_set(app: AppHandle, enabled: bool, confirmed: bool) -> Result<()> {
    require(confirmed)?;
    app::autostart::os_set(&app, enabled)
}

#[tauri::command]
pub fn app_hide(app: AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .hide()
            .map_err(|error| Error::msg(error.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub fn app_shutdown(app: AppHandle, force: Option<bool>) -> Result<()> {
    app::shutdown::request(&app, force.unwrap_or(false));
    Ok(())
}

#[tauri::command]
pub fn open_url(url: String) -> Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(Error::msg("Chỉ hỗ trợ mở đường dẫn http hoặc https"));
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let wide_url: Vec<u16> = std::ffi::OsStr::new(&url)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let wide_open: Vec<u16> = std::ffi::OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let ret = ShellExecuteW(
                None,
                PCWSTR(wide_open.as_ptr()),
                PCWSTR(wide_url.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
            if (ret.0 as usize) <= 32 {
                return Err(Error::msg(format!(
                    "Không thể mở trình duyệt: mã lỗi {}",
                    ret.0 as usize
                )));
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| Error::msg(e.to_string()))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| Error::msg(e.to_string()))?;
    }

    Ok(())
}
