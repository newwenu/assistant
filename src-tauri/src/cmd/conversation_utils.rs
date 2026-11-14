use std::collections::HashMap;

use assistant_providers::base;
use assistant_providers::types::resolve_context_window_from_stored;
use assistant_storage::{
    conversations,
    providers::{self as provider_store},
};
use assistant_tools::compact::rebuild_compact;

use crate::protocol::{MessageDto, MessageToolCallDto, MessageToolCallStatusDto};

#[derive(serde::Deserialize)]
struct StoredToolCall {
    id: String,
    name: String,
    #[serde(default)]
    arguments: String,
}

fn collect_tool_results(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> HashMap<String, (String, bool)> {
    conversations::get_tool_results(conn, conversation_id)
        .unwrap_or_default()
        .into_iter()
        .map(|(call_id, content, is_error)| (call_id, (content, is_error)))
        .collect()
}

fn message_tool_calls(
    row: &conversations::MessageRow,
    results: &HashMap<String, (String, bool)>,
) -> Vec<MessageToolCallDto> {
    let Some(json) = row.tool_calls_json.as_deref() else {
        return Vec::new();
    };
    let Ok(stored) = serde_json::from_str::<Vec<StoredToolCall>>(json) else {
        return Vec::new();
    };
    stored
        .into_iter()
        .map(|call| {
            let found = results.get(&call.id);
            let status = match found {
                Some((_, true)) => MessageToolCallStatusDto::Error,
                Some((_, false)) => MessageToolCallStatusDto::Success,
                None => MessageToolCallStatusDto::Missing,
            };
            let arguments =
                serde_json::from_str(&call.arguments).unwrap_or(serde_json::Value::Null);
            let result = found.map(|(content, _)| content.clone());
            let compact = found.map(|(content, is_error)| {
                rebuild_compact(&call.name, &arguments, Some(content.as_str()), *is_error).into()
            });
            MessageToolCallDto {
                call_id: call.id,
                name: call.name,
                arguments,
                status,
                result,
                compact,
            }
        })
        .collect()
}

/// wire 契约（I46 §2.4 D4）：`role = "tool"` 行的正文只在库里保留，
/// 下发时恒为空串；结果经 assistant 行 `toolCalls[].result` 送达前端（门禁 5）。
pub(crate) fn wire_message_dto(row: conversations::MessageRow) -> MessageDto {
    let mut dto = MessageDto::from(row);
    if dto.role == "tool" {
        dto.content.clear();
    }
    dto
}

pub(crate) fn messages_with_sibling_counts(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    messages: &[conversations::MessageRow],
) -> Result<Vec<MessageDto>, String> {
    let msg_ids: Vec<String> = messages.iter().map(|m| m.id.clone()).collect();
    let sibling_counts = conversations::get_sibling_counts_batch(conn, conversation_id, &msg_ids)?;
    let tool_results = collect_tool_results(conn, conversation_id);
    Ok(messages
        .iter()
        .map(|m| {
            let mut dto = wire_message_dto(m.clone());
            dto.sibling_count = sibling_counts.get(&dto.id).copied();
            dto.tool_calls = message_tool_calls(m, &tool_results);
            dto
        })
        .collect())
}

pub(crate) fn resolve_context_window_for_conversation(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<u32> {
    let conv = conversations::get_conversation_by_id(conn, conversation_id)
        .ok()
        .flatten()?;
    let model_id = &conv.model_id;
    if let Some(ref provider_id) = conv.provider_id {
        if let Ok(Some(provider_row)) = provider_store::get_provider_by_id(conn, provider_id) {
            return resolve_context_window_from_stored(model_id, &provider_row.models_json);
        }
    }
    base::get_context_window_tokens(model_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ToolErrorKindDto;

    fn row(
        role: &str,
        content: &str,
        tool_calls_json: Option<String>,
    ) -> conversations::MessageRow {
        conversations::MessageRow {
            id: "m1".into(),
            conversation_id: "c1".into(),
            role: role.into(),
            content: content.into(),
            tokens: None,
            timestamp: 1,
            parent_id: None,
            is_pinned: false,
            reasoning: None,
            tool_calls_json,
            sibling_index: 0,
            provider_id: None,
            model_id: None,
            tool_call_id: None,
            is_error: false,
            status: "completed".into(),
            mode_id: None,
            turn_index: None,
            iteration: 0,
        }
    }

    fn results(entries: &[(&str, &str, bool)]) -> HashMap<String, (String, bool)> {
        entries
            .iter()
            .map(|(call_id, content, is_error)| {
                ((*call_id).to_string(), ((*content).to_string(), *is_error))
            })
            .collect()
    }

    /// 门禁 5：tool 行正文只在库里保留，wire 上恒为空串。
    #[test]
    fn tool_row_content_is_empty_on_wire() {
        let dto = wire_message_dto(row("tool", "raw tool output", None));
        assert_eq!(dto.content, "");
    }

    #[test]
    fn assistant_row_keeps_content_on_wire() {
        let dto = wire_message_dto(row("assistant", "answer", None));
        assert_eq!(dto.content, "answer");
    }

    /// 门禁 1 / 9：历史 result 与落库正文逐字节相等，且 compact 可回放。
    #[test]
    fn history_rebuilds_compact_and_keeps_result_bytes() {
        let json = r#"[{"id":"call_1","name":"fs_read","arguments":"{\"path\":\"a.txt\"}"}]"#;
        let message = row("assistant", "done", Some(json.to_string()));
        let found = results(&[("call_1", "file body", false)]);
        let calls = message_tool_calls(&message, &found);
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.status, MessageToolCallStatusDto::Success);
        assert_eq!(call.result.as_deref(), Some("file body"));
        let compact = call.compact.as_ref().expect("compact 由历史重建");
        assert!(compact.head.starts_with("ok fs_read"), "{}", compact.head);
        assert!(compact.error_kind.is_none());
    }

    /// 门禁 9：写入时的失败分类能在历史重载后原样还原。
    #[test]
    fn history_rebuild_keeps_error_kind_from_text() {
        let json = r#"[{"id":"call_1","name":"shell","arguments":"{\"command\":[\"ls\"]}"}]"#;
        let message = row("assistant", "", Some(json.to_string()));
        let found = results(&[("call_1", "tool 'shell': permission denied for ls", true)]);
        let calls = message_tool_calls(&message, &found);
        let call = &calls[0];
        assert_eq!(call.status, MessageToolCallStatusDto::Error);
        assert_eq!(
            call.result.as_deref(),
            Some("tool 'shell': permission denied for ls")
        );
        let compact = call.compact.as_ref().expect("compact 由历史重建");
        assert_eq!(compact.error_kind, Some(ToolErrorKindDto::Denied));
    }

    #[test]
    fn history_without_result_row_has_no_compact() {
        let json = r#"[{"id":"call_1","name":"fs_read","arguments":"{}"}]"#;
        let message = row("assistant", "", Some(json.to_string()));
        let calls = message_tool_calls(&message, &HashMap::new());
        let call = &calls[0];
        assert_eq!(call.status, MessageToolCallStatusDto::Missing);
        assert!(call.result.is_none());
        assert!(call.compact.is_none());
    }
}
