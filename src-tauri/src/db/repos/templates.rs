use crate::error::{Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateParam {
    pub name: String,
    pub label: String,
    pub kind: String,
    pub default_value: Option<String>,
    pub required: bool,
    pub options: Option<Vec<String>>,
    pub is_secret: bool,
    pub param_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePreset {
    pub id: String,
    pub template_id: String,
    pub name: String,
    pub values: HashMap<String, String>,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub template_string: String,
    pub is_shell: bool,
    pub last_run_at: Option<String>,
    pub params: Vec<TemplateParam>,
    pub presets: Vec<TemplatePreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePayload {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub template_string: String,
    pub is_shell: bool,
    pub params: Vec<TemplateParam>,
}

fn decode_options(value: Option<String>) -> Result<Option<Vec<String>>> {
    value
        .map(|json| serde_json::from_str(&json).map_err(|e| Error::msg(e.to_string())))
        .transpose()
}

fn decode_values(json: String) -> Result<HashMap<String, String>> {
    serde_json::from_str(&json).map_err(|e| Error::msg(e.to_string()))
}

fn read_params(conn: &Connection, template_id: &str) -> Result<Vec<TemplateParam>> {
    let mut stmt = conn.prepare(
        "SELECT name, label, kind, default_value, required, options_json, is_secret, param_order
         FROM template_param WHERE template_id = ?1 ORDER BY param_order, name",
    )?;
    let rows = stmt.query_map([template_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, i64>(4)? != 0,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, i64>(6)? != 0,
            row.get::<_, i64>(7)?,
        ))
    })?;

    rows.map(|row| {
        let (name, label, kind, default_value, required, options_json, is_secret, param_order) =
            row?;
        Ok(TemplateParam {
            name,
            label,
            kind,
            default_value,
            required,
            options: decode_options(options_json)?,
            is_secret,
            param_order,
        })
    })
    .collect()
}

fn read_presets(conn: &Connection, template_id: &str) -> Result<Vec<TemplatePreset>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, values_json, last_used_at
         FROM template_preset WHERE template_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map([template_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    rows.map(|row| {
        let (id, name, values_json, last_used_at) = row?;
        Ok(TemplatePreset {
            id,
            template_id: template_id.to_string(),
            name,
            values: decode_values(values_json)?,
            last_used_at,
        })
    })
    .collect()
}

pub fn list(conn: &Connection) -> Result<Vec<TemplateRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, template_string, is_shell, last_run_at
         FROM command_template ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)? != 0,
            row.get::<_, Option<String>>(5)?,
        ))
    })?;

    rows.map(|row| {
        let (id, name, description, template_string, is_shell, last_run_at) = row?;
        Ok(TemplateRecord {
            params: read_params(conn, &id)?,
            presets: read_presets(conn, &id)?,
            id,
            name,
            description,
            template_string,
            is_shell,
            last_run_at,
        })
    })
    .collect()
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<TemplateRecord>> {
    list(conn).map(|items| items.into_iter().find(|item| item.id == id))
}

fn insert_params(
    conn: &Connection,
    template_id: &str,
    params_list: &[TemplateParam],
) -> Result<()> {
    for param in params_list {
        let options_json = param
            .options
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| Error::msg(e.to_string()))?;
        conn.execute(
            "INSERT INTO template_param
             (template_id, name, label, kind, default_value, required, options_json, is_secret, param_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![template_id, param.name, param.label, param.kind, param.default_value,
                param.required as i64, options_json, param.is_secret as i64, param.param_order],
        )?;
    }
    Ok(())
}

pub fn insert(conn: &Connection, template: &TemplateRecord) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO command_template (id, name, description, template_string, is_shell, last_run_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![template.id, template.name, template.description, template.template_string,
            template.is_shell as i64, template.last_run_at],
    )?;
    insert_params(&tx, &template.id, &template.params)?;
    tx.commit()?;
    Ok(())
}

pub fn update(conn: &Connection, template: &TemplateRecord) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute(
        "UPDATE command_template SET name = ?2, description = ?3, template_string = ?4, is_shell = ?5
         WHERE id = ?1",
        params![template.id, template.name, template.description, template.template_string, template.is_shell as i64],
    )?;
    if changed == 0 {
        return Err(Error::msg("template not found"));
    }
    tx.execute(
        "DELETE FROM template_param WHERE template_id = ?1",
        [&template.id],
    )?;
    insert_params(&tx, &template.id, &template.params)?;
    tx.commit()?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM command_template WHERE id = ?1", [id])?;
    Ok(())
}

pub fn mark_run(conn: &Connection, id: &str, at: &str) -> Result<()> {
    conn.execute(
        "UPDATE command_template SET last_run_at = ?2 WHERE id = ?1",
        params![id, at],
    )?;
    Ok(())
}

pub fn save_preset(conn: &Connection, preset: &TemplatePreset) -> Result<()> {
    let values_json =
        serde_json::to_string(&preset.values).map_err(|e| Error::msg(e.to_string()))?;
    conn.execute(
        "INSERT INTO template_preset (id, template_id, name, values_json, last_used_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            preset.id,
            preset.template_id,
            preset.name,
            values_json,
            preset.last_used_at
        ],
    )?;
    Ok(())
}

pub fn delete_preset(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM template_preset WHERE id = ?1", [id])?;
    Ok(())
}

pub fn preset(conn: &Connection, id: &str) -> Result<Option<TemplatePreset>> {
    let row = conn.query_row(
        "SELECT id, template_id, name, values_json, last_used_at FROM template_preset WHERE id = ?1",
        [id],
        |row| Ok((
            row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
            row.get::<_, String>(3)?, row.get::<_, Option<String>>(4)?,
        )),
    ).optional()?;
    row.map(|(id, template_id, name, values_json, last_used_at)| {
        Ok(TemplatePreset {
            id,
            template_id,
            name,
            values: decode_values(values_json)?,
            last_used_at,
        })
    })
    .transpose()
}
