use serde_json::Value;

use crate::os::output::human_bytes;
use crate::result::{ToolErrorKind, ToolResult};

use super::types::{HEAD_MAX_CHARS, HEAD_SEPARATOR, SUBJECT_MAX_CHARS};
use super::{args_str, data_bool, data_str, data_u64, truncate_end, truncate_middle};

/// head：`ok <tool> · <subject> · <scale>` / `err <kind> <tool> · <subject> · <reason>`。
pub fn build_head(
    tool: &str,
    arguments: &Value,
    data: Option<&Value>,
    result: &ToolResult,
) -> String {
    if result.ok {
        ok_head(tool, arguments, data, result)
    } else {
        err_head(tool, arguments, data, result)
    }
}

fn ok_head(tool: &str, arguments: &Value, data: Option<&Value>, result: &ToolResult) -> String {
    let mut parts = vec![format!("ok {}", tool)];
    if let Some(subject) = subject(tool, arguments, data) {
        parts.push(truncate_middle(&subject, SUBJECT_MAX_CHARS));
    }
    if let Some(scale) = scale(tool, data, result) {
        parts.push(scale);
    }
    truncate_end(&parts.join(HEAD_SEPARATOR), HEAD_MAX_CHARS)
}

fn err_head(tool: &str, arguments: &Value, data: Option<&Value>, result: &ToolResult) -> String {
    let kind = result.error_kind.unwrap_or(ToolErrorKind::Business);
    let reason = result
        .error
        .as_deref()
        .unwrap_or("failed")
        .lines()
        .next()
        .unwrap_or("failed")
        .trim()
        .to_string();
    let mut parts = vec![format!("err {} {}", kind_word(kind), tool)];
    if let Some(subject) = subject(tool, arguments, data) {
        parts.push(truncate_middle(&subject, SUBJECT_MAX_CHARS));
    }
    parts.push(reason);
    truncate_end(&parts.join(HEAD_SEPARATOR), HEAD_MAX_CHARS)
}

fn kind_word(kind: ToolErrorKind) -> &'static str {
    match kind {
        ToolErrorKind::Business => "business",
        ToolErrorKind::Timeout => "timeout",
        ToolErrorKind::Cancelled => "cancelled",
        ToolErrorKind::Internal => "internal",
        ToolErrorKind::Denied => "denied",
    }
}

/// subject 优先取结果事实（`data`），失败结果没有 `data` 时回退到参数。
fn subject(tool: &str, arguments: &Value, data: Option<&Value>) -> Option<String> {
    match tool {
        "read" => {
            let path = data_str(data, "path").or_else(|| args_str(arguments, "path"))?;
            let start = data_u64(data, "startLine");
            let end = data_u64(data, "endLine");
            Some(match (start, end) {
                (Some(start), Some(end)) => format!("{}:{}-{}", path, start, end),
                _ => path,
            })
        }
        "write" | "edit" | "mkdir" | "delete" | "list" => {
            data_str(data, "path").or_else(|| args_str(arguments, "path"))
        }
        "grep" => {
            let pattern = data_str(data, "pattern").or_else(|| args_str(arguments, "pattern"))?;
            let path = data_str(data, "path")
                .or_else(|| args_str(arguments, "path"))
                .unwrap_or_else(|| ".".to_string());
            Some(format!("{} @ {}", pattern, path))
        }
        "glob" => data_str(data, "pattern").or_else(|| args_str(arguments, "pattern")),
        "move" => {
            let source = data_str(data, "source").or_else(|| args_str(arguments, "source"))?;
            let target = data_str(data, "target").or_else(|| args_str(arguments, "target"))?;
            Some(format!("{} -> {}", source, target))
        }
        _ => args_str(arguments, "path")
            .or_else(|| args_str(arguments, "pattern"))
            .or_else(|| args_str(arguments, "query")),
    }
}

fn scale(tool: &str, data: Option<&Value>, result: &ToolResult) -> Option<String> {
    match tool {
        "read" => {
            let total = data_u64(data, "totalLines")?;
            let start = data_u64(data, "startLine").unwrap_or(1);
            let end = data_u64(data, "endLine").unwrap_or(total);
            let returned = end.saturating_sub(start).saturating_add(1);
            Some(format!(
                "{}/{} lines · {}",
                returned,
                total,
                human_bytes(result.assistant_text().len())
            ))
        }
        "write" => {
            let lines = data_u64(data, "lines")?;
            let state = if data_bool(data, "created") == Some(true) {
                "created"
            } else if data_bool(data, "appended") == Some(true) {
                "appended"
            } else if data_bool(data, "unchanged") == Some(true) {
                "unchanged"
            } else {
                "updated"
            };
            Some(format!("{} · {} lines", state, lines))
        }
        "edit" => {
            let count = data_u64(data, "matchCount")?;
            Some(if count == 1 {
                "1 replacement".to_string()
            } else {
                format!("{} replacements", count)
            })
        }
        "grep" => {
            let hits = data_u64(data, "totalMatches")?;
            Some(match data_u64(data, "files") {
                Some(files) => format!("{} hits · {} files", hits, files),
                None => format!("{} hits", hits),
            })
        }
        "glob" => Some(format!("{} paths", data_u64(data, "totalMatches")?)),
        "list" => {
            let entries = data_u64(data, "totalEntries")?;
            Some(match data_u64(data, "depth") {
                Some(depth) => format!("{} entries · depth {}", entries, depth),
                None => format!("{} entries", entries),
            })
        }
        "mkdir" => Some(if data_bool(data, "created") == Some(true) {
            "created".to_string()
        } else {
            "already exists".to_string()
        }),
        "move" => Some(if data_bool(data, "overwritten") == Some(true) {
            "moved (overwritten)".to_string()
        } else {
            "moved".to_string()
        }),
        "delete" => {
            let entries = data_u64(data, "entriesDeleted")?;
            let bytes = data_u64(data, "bytesDeleted");
            Some(match bytes {
                Some(bytes) => format!("{} entries · {}", entries, human_bytes(bytes as usize)),
                None => format!("{} entries", entries),
            })
        }
        "ask_user" => {
            let count = data
                .and_then(|value| value.get("questions"))
                .and_then(Value::as_array)
                .map(|questions| questions.len() as u64)?;
            Some(format!("{} questions", count))
        }
        _ => None,
    }
}
