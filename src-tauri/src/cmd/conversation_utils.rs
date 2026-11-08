use std::collections::HashMap;

use assistant_providers::base;
use assistant_providers::types::resolve_context_window_from_stored;
use assistant_storage::{
    conversations,
    providers::{self as provider_store},
};

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
                None => MessageToolCallStatusDto::Cancelled,
            };
            MessageToolCallDto {
                call_id: call.id,
                name: call.name,
                arguments: serde_json::from_str(&call.arguments).unwrap_or(serde_json::Value::Null),
                status,
                result: found.map(|(content, _)| content.clone()),
            }
        })
        .collect()
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
            let mut dto = MessageDto::from(m.clone());
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
