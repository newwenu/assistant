use serde_json::Value;

use super::truncate_end;
use super::types::{ARGS_PREVIEW_MAX_CHARS, PREVIEW_VALUE_MAX_CHARS};

/// `ArgsDelta` 的受控回退：只抽关键参数，总长 ≤ 200 字符。
pub fn args_preview(tool: &str, arguments: &Value) -> Option<String> {
    let object = arguments.as_object()?;
    let mut parts: Vec<String> = Vec::new();
    for key in preview_keys(tool) {
        let Some(value) = object.get(*key) else {
            continue;
        };
        let Some(rendered) = render_value(value) else {
            continue;
        };
        parts.push(format!("{}={}", key, rendered));
    }
    if parts.is_empty() {
        return None;
    }
    Some(truncate_end(&parts.join(", "), ARGS_PREVIEW_MAX_CHARS))
}

fn preview_keys(tool: &str) -> &'static [&'static str] {
    match tool {
        "read" => &["path", "offset", "limit"],
        "write" => &["path", "append"],
        "edit" => &["path", "replace_all"],
        "grep" => &["pattern", "path", "include"],
        "glob" => &["pattern", "path", "exclude"],
        "list" => &["path", "depth"],
        "mkdir" => &["path", "parents"],
        "move" => &["source", "target"],
        "delete" => &["path", "recursive"],
        "ask_user" => &["questions"],
        _ => &["path", "pattern", "query", "command", "source", "target"],
    }
}

fn render_value(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(truncate_end(text, PREVIEW_VALUE_MAX_CHARS)),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Array(items) => Some(format!("[{} items]", items.len())),
        Value::Object(map) => Some(format!("{{{} keys}}", map.len())),
    }
}
