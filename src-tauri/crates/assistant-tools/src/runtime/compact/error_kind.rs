use crate::result::ToolErrorKind;

/// 历史错误的分类：写入侧 `ToolResult::from_exec_error_kind` 的文案前缀是稳定契约，
/// 读取侧据此还原同一分类（门禁 9）。未知文案归 `Business`。
pub fn error_kind_from_text(text: &str) -> Option<ToolErrorKind> {
    let text = text.trim_start();
    if !text.starts_with("tool '") {
        return Some(ToolErrorKind::Business);
    }
    let message = text.split_once("': ").map(|(_, rest)| rest).unwrap_or(text);
    if message.starts_with("permission denied for") || message.starts_with("rejected by user") {
        return Some(ToolErrorKind::Denied);
    }
    if message.starts_with("approval timed out") {
        return Some(ToolErrorKind::Timeout);
    }
    Some(ToolErrorKind::Internal)
}
