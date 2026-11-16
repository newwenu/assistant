use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolErrorKind {
    Business,
    Timeout,
    Cancelled,
    Internal,
    Denied,
}

/// 工具执行结果的双通道：
/// - `data`：给 UI 渲染的完整结果。
/// - `result_for_assistant`：回填模型的精简文本；缺省时由 `data` 的紧凑序列化兜底。
///
/// `error_kind` 由 `exec::invoke` 与 Pipeline 打标：超时 → `Timeout`、取消 → `Cancelled`、
/// 工具 `Err` → `Internal`、网关拒绝 → `Denied`、业务失败 → `Business`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_for_assistant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<ToolErrorKind>,
}

impl ToolResult {
    pub fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            result_for_assistant: None,
            error: None,
            error_kind: None,
        }
    }

    pub fn ok_with_assistant(data: Value, text: impl Into<String>) -> Self {
        Self {
            ok: true,
            data: Some(data),
            result_for_assistant: Some(text.into()),
            error: None,
            error_kind: None,
        }
    }

    /// 业务失败：工具正常执行，结果可回填给模型纠错。
    pub fn err(error: impl Into<String>) -> Self {
        Self::err_with_kind(ToolErrorKind::Business, error)
    }

    pub fn err_with_kind(kind: ToolErrorKind, error: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            result_for_assistant: None,
            error: Some(error.into()),
            error_kind: Some(kind),
        }
    }

    /// `Err` 通道归一化：统一前缀工具名，便于日志检索与模型纠错。
    pub fn from_exec_error(tool: &str, error: impl Into<String>) -> Self {
        Self::from_exec_error_kind(tool, ToolErrorKind::Internal, error)
    }

    pub fn from_exec_error_kind(tool: &str, kind: ToolErrorKind, error: impl Into<String>) -> Self {
        Self::err_with_kind(kind, format!("tool '{}': {}", tool, error.into()))
    }

    /// 回填模型的文本：显式通道优先，其次 `data` 紧凑序列化，最后错误文本。
    pub fn assistant_text(&self) -> String {
        if let Some(text) = &self.result_for_assistant {
            return text.clone();
        }
        if let Some(data) = &self.data {
            return serde_json::to_string(data).unwrap_or_default();
        }
        self.error.clone().unwrap_or_default()
    }

    /// 回填文本的字符数，用于结果持久化预算。
    pub fn assistant_char_count(&self) -> usize {
        self.assistant_text().chars().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ok_result_has_no_error_kind() {
        let result = ToolResult::ok(json!({ "value": 1 }));
        assert!(result.ok);
        assert!(result.error_kind.is_none());
        assert!(result.result_for_assistant.is_none());
    }

    #[test]
    fn business_error_is_tagged_business() {
        let result = ToolResult::err("boom");
        assert!(!result.ok);
        assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    }

    #[test]
    fn exec_error_is_tagged_internal_with_tool_prefix() {
        let result = ToolResult::from_exec_error("fs_link", "boom");
        assert_eq!(result.error_kind, Some(ToolErrorKind::Internal));
        assert_eq!(result.error.as_deref(), Some("tool 'fs_link': boom"));
    }

    #[test]
    fn assistant_text_prefers_explicit_channel() {
        let result = ToolResult::ok_with_assistant(json!({ "raw": "x" }), "summary");
        assert_eq!(result.assistant_text(), "summary");
        assert_eq!(result.assistant_char_count(), 7);
    }

    #[test]
    fn assistant_text_falls_back_to_compact_json() {
        let result = ToolResult::ok(json!({ "value": 1 }));
        assert_eq!(result.assistant_text(), r#"{"value":1}"#);
    }

    #[test]
    fn assistant_text_falls_back_to_error_message() {
        let result = ToolResult::err("boom");
        assert_eq!(result.assistant_text(), "boom");
    }

    #[test]
    fn error_kind_serializes_camel_case() {
        let value = serde_json::to_value(ToolErrorKind::Cancelled).unwrap();
        assert_eq!(value, json!("cancelled"));
    }
}
