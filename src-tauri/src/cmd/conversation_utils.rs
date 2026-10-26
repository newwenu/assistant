use assistant_providers::base;
use assistant_providers::types::resolve_context_window_from_stored;
use assistant_storage::{
    conversations,
    providers::{self as provider_store},
};

pub(crate) fn messages_with_sibling_counts(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    messages: &[conversations::MessageRow],
) -> Result<Vec<crate::protocol::MessageDto>, String> {
    let msg_ids: Vec<String> = messages.iter().map(|m| m.id.clone()).collect();
    let sibling_counts = conversations::get_sibling_counts_batch(conn, conversation_id, &msg_ids)?;
    Ok(messages
        .iter()
        .map(|m| {
            let mut dto = crate::protocol::MessageDto::from(m.clone());
            dto.sibling_count = sibling_counts.get(&dto.id).copied();
            dto
        })
        .collect())
}

pub(crate) fn resolve_context_window_for_conversation(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> u32 {
    let conv = match conversations::get_conversation_by_id(conn, conversation_id) {
        Ok(Some(c)) => c,
        _ => return 128_000,
    };
    let model_id = &conv.model_id;
    if let Some(ref provider_id) = conv.provider_id {
        if let Ok(Some(provider_row)) = provider_store::get_provider_by_id(conn, provider_id) {
            return resolve_context_window_from_stored(model_id, &provider_row.models_json);
        }
    }
    base::get_context_window_tokens(model_id)
}
