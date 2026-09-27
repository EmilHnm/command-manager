use crate::argv::split_argv;
use crate::db::repos::templates::{TemplateParam, TemplateRecord};
use crate::error::{Error, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct RenderedTemplate {
    pub command_line: String,
    pub masked_command_line: String,
    pub argv: Option<Vec<String>>,
    pub masked_argv: Option<Vec<String>>,
    pub warnings: Vec<String>,
}

/// Returns field-level validation errors for the preview form without rendering
/// any user-controlled value into a command string.
pub fn validation_errors(
    template: &TemplateRecord,
    values: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut errors = HashMap::new();
    let placeholders = match parse_placeholders(&template.template_string) {
        Ok(placeholders) => placeholders,
        Err(error) => {
            errors.insert(String::from("_template"), error.to_string());
            return errors;
        }
    };

    let declared: HashSet<&str> = template
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    for placeholder in placeholders {
        if !declared.contains(placeholder.name.as_str()) {
            errors.insert(
                String::from("_template"),
                format!("placeholder '{}' chưa được khai báo", placeholder.name),
            );
        }
    }

    for param in &template.params {
        let value = values
            .get(&param.name)
            .cloned()
            .or_else(|| param.default_value.clone())
            .unwrap_or_default();
        if let Err(error) = validate_param(param, &value) {
            errors.insert(param.name.clone(), error.to_string());
        }
    }
    errors
}

#[derive(Debug, Clone)]
struct Placeholder {
    start: usize,
    end: usize,
    name: String,
}

#[derive(Debug, Clone)]
struct ResolvedValue {
    value: String,
    secret: bool,
}

pub fn render(
    template: &TemplateRecord,
    values: &HashMap<String, String>,
) -> Result<RenderedTemplate> {
    let placeholders = parse_placeholders(&template.template_string)?;
    let params: HashMap<&str, &TemplateParam> = template
        .params
        .iter()
        .map(|param| (param.name.as_str(), param))
        .collect();
    let resolved = resolve_values(&template.params, values)?;
    let used: HashSet<&str> = placeholders.iter().map(|item| item.name.as_str()).collect();

    for item in &placeholders {
        if !params.contains_key(item.name.as_str()) {
            return Err(Error::msg(format!(
                "placeholder '{}' chưa được khai báo",
                item.name
            )));
        }
    }

    let warnings = template
        .params
        .iter()
        .filter(|param| !used.contains(param.name.as_str()))
        .map(|param| format!("Tham số '{}' chưa được dùng trong template", param.name))
        .collect();

    if template.is_shell {
        let command_line =
            render_shell(&template.template_string, &placeholders, &resolved, false)?;
        let masked_command_line =
            render_shell(&template.template_string, &placeholders, &resolved, true)?;
        Ok(RenderedTemplate {
            command_line,
            masked_command_line,
            argv: None,
            masked_argv: None,
            warnings,
        })
    } else {
        let normalized = normalize_placeholders(&template.template_string, &placeholders);
        let tokens = split_argv(&normalized)
            .map_err(|error| Error::Argv(format!("template argv không hợp lệ: {error}")))?;
        let argv = replace_sentinels(tokens, &resolved, false)?;
        let masked_tokens = split_argv(&normalized)
            .map_err(|error| Error::Argv(format!("template argv không hợp lệ: {error}")))?;
        let masked_argv = replace_sentinels(masked_tokens, &resolved, true)?;
        Ok(RenderedTemplate {
            command_line: shell_words::join(argv.iter().map(String::as_str)),
            masked_command_line: shell_words::join(masked_argv.iter().map(String::as_str)),
            argv: Some(argv),
            masked_argv: Some(masked_argv),
            warnings,
        })
    }
}

fn parse_placeholders(template: &str) -> Result<Vec<Placeholder>> {
    let mut result = Vec::new();
    let mut cursor = 0;
    while let Some(open_offset) = template[cursor..].find("{{") {
        let start = cursor + open_offset;
        let close_offset = template[start + 2..]
            .find("}}")
            .ok_or_else(|| Error::msg("placeholder thiếu dấu đóng '}}'"))?;
        let end = start + 2 + close_offset + 2;
        let name = template[start + 2..start + 2 + close_offset].trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return Err(Error::msg(format!("placeholder không hợp lệ: {name:?}")));
        }
        result.push(Placeholder {
            start,
            end,
            name: name.to_string(),
        });
        cursor = end;
    }
    Ok(result)
}

fn resolve_values<'a>(
    params: &'a [TemplateParam],
    values: &'a HashMap<String, String>,
) -> Result<HashMap<&'a str, ResolvedValue>> {
    let mut resolved = HashMap::new();
    for param in params {
        let value = values
            .get(&param.name)
            .cloned()
            .or_else(|| param.default_value.clone())
            .unwrap_or_default();
        validate_param(param, &value)?;
        resolved.insert(
            param.name.as_str(),
            ResolvedValue {
                value,
                secret: param.is_secret,
            },
        );
    }
    Ok(resolved)
}

fn validate_param(param: &TemplateParam, value: &str) -> Result<()> {
    if param.required && value.trim().is_empty() {
        return Err(Error::msg(format!(
            "missing required parameter: {}",
            param.name
        )));
    }
    if value.is_empty() {
        return Ok(());
    }
    match param.kind.as_str() {
        "number" => value
            .parse::<f64>()
            .map(|_| ())
            .map_err(|_| Error::msg(format!("tham số '{}' phải là số", param.name))),
        "bool" => {
            if matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "false" | "1" | "0"
            ) {
                Ok(())
            } else {
                Err(Error::msg(format!("tham số '{}' phải là bool", param.name)))
            }
        }
        "enum" => {
            let options = param
                .options
                .as_ref()
                .filter(|items| !items.is_empty())
                .ok_or_else(|| Error::msg(format!("enum '{}' chưa có options", param.name)))?;
            if options.iter().any(|option| option == value) {
                Ok(())
            } else {
                Err(Error::msg(format!(
                    "tham số '{}' phải thuộc: {}",
                    param.name,
                    options.join(", ")
                )))
            }
        }
        "string" | "path" => Ok(()),
        other => Err(Error::msg(format!("kiểu tham số không hỗ trợ: {other}"))),
    }
}

fn normalize_placeholders(template: &str, placeholders: &[Placeholder]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut cursor = 0;
    for placeholder in placeholders {
        output.push_str(&template[cursor..placeholder.start]);
        output.push_str(&format!("__CM_TEMPLATE_PARAM_{}__", placeholder.name));
        cursor = placeholder.end;
    }
    output.push_str(&template[cursor..]);
    output
}

fn replace_sentinels(
    tokens: Vec<String>,
    resolved: &HashMap<&str, ResolvedValue>,
    masked: bool,
) -> Result<Vec<String>> {
    let mut output = Vec::with_capacity(tokens.len());
    for mut token in tokens {
        for (name, value) in resolved {
            let replacement = if masked && value.secret {
                "******"
            } else {
                value.value.as_str()
            };
            token = token.replace(&format!("__CM_TEMPLATE_PARAM_{}__", name), replacement);
        }
        output.push(token);
    }
    Ok(output)
}

fn render_shell(
    template: &str,
    placeholders: &[Placeholder],
    resolved: &HashMap<&str, ResolvedValue>,
    masked: bool,
) -> Result<String> {
    let mut output = String::with_capacity(template.len());
    let mut cursor = 0;
    for placeholder in placeholders {
        output.push_str(&template[cursor..placeholder.start]);
        let value = resolved
            .get(placeholder.name.as_str())
            .ok_or_else(|| Error::msg(format!("missing parameter: {}", placeholder.name)))?;
        let value = if masked && value.secret {
            "******"
        } else {
            value.value.as_str()
        };
        let context = shell_quote_context(template, placeholder.start);
        let rendered = match context {
            QuoteContext::Single => {
                return Err(Error::msg(format!(
                    "placeholder '{}' không được đặt trong dấu nháy đơn; hãy bỏ quote để engine tự quote",
                    placeholder.name
                )))
            }
            QuoteContext::Double => escape_double_quoted(value)?,
            _ => quote_shell_value(value)?,
        };
        output.push_str(&rendered);
        cursor = placeholder.end;
    }
    output.push_str(&template[cursor..]);
    Ok(output)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteContext {
    None,
    Single,
    Double,
}

fn shell_quote_context(template: &str, end: usize) -> QuoteContext {
    let mut context = QuoteContext::None;
    let mut escaped = false;
    for ch in template[..end].chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && context != QuoteContext::Single {
            escaped = true;
            continue;
        }
        match (context, ch) {
            (QuoteContext::None, '\'') => context = QuoteContext::Single,
            (QuoteContext::None, '"') => context = QuoteContext::Double,
            (QuoteContext::Single, '\'') => context = QuoteContext::None,
            (QuoteContext::Double, '"') => context = QuoteContext::None,
            _ => {}
        }
    }
    context
}

#[cfg(unix)]
fn quote_shell_value(value: &str) -> Result<String> {
    Ok(shell_words::quote(value).to_string())
}

#[cfg(windows)]
fn quote_shell_value(value: &str) -> Result<String> {
    if value.contains('"') || value.contains('\r') || value.contains('\n') {
        return Err(Error::msg(
            "giá trị shell Windows không được chứa quote hoặc xuống dòng",
        ));
    }
    Ok(format!("\"{value}\""))
}

#[cfg(unix)]
fn escape_double_quoted(value: &str) -> Result<String> {
    Ok(value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`"))
}

#[cfg(windows)]
fn escape_double_quoted(value: &str) -> Result<String> {
    if value.contains('"') || value.contains('\r') || value.contains('\n') {
        return Err(Error::msg(
            "giá trị shell Windows không được chứa quote hoặc xuống dòng",
        ));
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(name: &str, kind: &str, secret: bool) -> TemplateParam {
        TemplateParam {
            name: name.into(),
            label: name.into(),
            kind: kind.into(),
            default_value: None,
            required: true,
            options: None,
            is_secret: secret,
            param_order: 1,
        }
    }

    #[test]
    fn argv_values_with_spaces_remain_one_argument() {
        let template = TemplateRecord {
            id: "id".into(),
            name: "name".into(),
            description: None,
            template_string: "rclone copy \"{{src}}\" dst".into(),
            is_shell: false,
            last_run_at: None,
            params: vec![param("src", "path", false)],
            presets: Vec::new(),
        };
        let values = HashMap::from([(String::from("src"), String::from(r"F:\BDMV Re ZERO"))]);
        let rendered = render(&template, &values).unwrap();
        assert_eq!(rendered.argv.unwrap()[2], r"F:\BDMV Re ZERO");
    }

    #[cfg(unix)]
    #[test]
    fn unix_double_quote_placeholder_is_escaped() {
        let template = TemplateRecord {
            id: "id".into(),
            name: "name".into(),
            description: None,
            template_string: "echo \"{{msg}}\"".into(),
            is_shell: true,
            last_run_at: None,
            params: vec![param("msg", "string", false)],
            presets: Vec::new(),
        };
        let values = HashMap::from([(String::from("msg"), String::from("$(id)"))]);
        let rendered = render(&template, &values).unwrap();
        assert!(rendered.command_line.contains("\\$(id)"));
    }

    #[test]
    fn enum_is_validated() {
        let mut method = param("method", "enum", false);
        method.options = Some(vec!["GET".into(), "POST".into()]);
        let template = TemplateRecord {
            id: "id".into(),
            name: "name".into(),
            description: None,
            template_string: "curl -X {{method}}".into(),
            is_shell: false,
            last_run_at: None,
            params: vec![method],
            presets: Vec::new(),
        };
        let values = HashMap::from([(String::from("method"), String::from("DELETE"))]);
        assert!(render(&template, &values).is_err());
    }

    #[test]
    fn argv_values_are_not_replaced_recursively() {
        let template = TemplateRecord {
            id: "id".into(),
            name: "name".into(),
            description: None,
            template_string: "echo {{message}}".into(),
            is_shell: false,
            last_run_at: None,
            params: vec![param("message", "string", false)],
            presets: Vec::new(),
        };
        let values = HashMap::from([("message".into(), "{{other}}".into())]);
        let rendered = render(&template, &values).unwrap();
        assert_eq!(rendered.argv.unwrap()[1], "{{other}}");
    }

    #[test]
    fn placeholder_inside_single_quotes_is_rejected() {
        let template = TemplateRecord {
            id: "id".into(),
            name: "name".into(),
            description: None,
            template_string: "echo '{{message}}'".into(),
            is_shell: true,
            last_run_at: None,
            params: vec![param("message", "string", false)],
            presets: Vec::new(),
        };
        let values = HashMap::from([(String::from("message"), String::from("$(id)"))]);
        let error = render(&template, &values).expect_err("single-quoted placeholder is unsafe");
        assert!(error.to_string().contains("dấu nháy đơn"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_value_does_not_get_caret_escaped() {
        let template = TemplateRecord {
            id: "id".into(),
            name: "name".into(),
            description: None,
            template_string: "echo \"{{message}}\"".into(),
            is_shell: true,
            last_run_at: None,
            params: vec![param("message", "string", false)],
            presets: Vec::new(),
        };
        let values = HashMap::from([("message".into(), "a&b".into())]);
        let rendered = render(&template, &values).unwrap();
        assert_eq!(rendered.command_line, "echo \"a&b\"");
    }
}
