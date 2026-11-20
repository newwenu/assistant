use serde_json::Value;

use crate::os::output::human_bytes;
use crate::result::{ToolErrorKind, ToolResult};

use super::access::{args_str, data_bool, data_str, data_u64};
use super::text::{truncate_end, truncate_middle};
use super::types::{HEAD_MAX_CHARS, HEAD_SEPARATOR, SUBJECT_MAX_CHARS};

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
        // `shell` 的 subject 只取参数里的命令：结果 `data` 不含命令原文（§8.4 十项）。
        // 先折空白：命令常写成多行续行式，换行会让 head 变成两行（`head` 恒单行的门禁）。
        "shell" => args_str(arguments, "command").map(|command| collapse_whitespace(&command)),
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
        // `exit <code> · N lines · <size>`：退出码是 shell 的头号事实，排在最前。
        // 行数带 `+` 表示落盘触达上限（`outputLines` 是截至上限的计数，不是全量）。
        "shell" => {
            let lines = data_u64(data, "outputLines")?;
            let bytes = data_u64(data, "outputBytes")?;
            let status = match (
                data.and_then(|value| value.get("exitCode"))
                    .and_then(Value::as_i64),
                data_u64(data, "signal"),
            ) {
                (Some(code), _) => format!("exit {}", code),
                // Unix 被信号杀死时没有退出码：报信号号，不用 `-1` 哨兵（§8.4）。
                (None, Some(signal)) => format!("signal {}", signal),
                (None, None) => "no exit status".to_string(),
            };
            let counted = if data_bool(data, "outputTruncated") == Some(true) {
                format!("{}+ lines", lines)
            } else {
                format!("{} lines", lines)
            };
            Some(format!(
                "{} · {} · {}",
                status,
                counted,
                human_bytes(bytes as usize)
            ))
        }
        _ => None,
    }
}

/// 折空白：把任意空白串压成单个空格，首尾不留。
///
/// 只用于 head 的展示口径；命令原文（审批键、执行体）**不得**经过本函数。
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
