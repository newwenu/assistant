use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub mod branch;
pub mod compression;
pub mod crud;
pub mod msg_ops;
pub mod ops;
pub mod tree_ops;

const CONV_SELECT: &str = "id, title, title_source, provider_id, model_id, system_prompt, is_pinned, sort_order, generation, created_at, updated_at, is_full, active_branch_id";
const MSG_SELECT: &str =
    "id, conversation_id, role, content, tokens, timestamp, parent_id, is_pinned, reasoning, tool_calls_json, sibling_index, provider_id, model_id, tool_call_id, is_error";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationRow {
    pub id: String,
    pub title: String,
    pub title_source: String,
    pub provider_id: Option<String>,
    pub model_id: String,
    pub system_prompt: Option<String>,
    pub is_pinned: bool,
    pub sort_order: i64,
    pub generation: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_full: bool,
    pub active_branch_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub tokens: Option<i64>,
    pub timestamp: i64,
    pub parent_id: Option<String>,
    pub is_pinned: bool,
    pub reasoning: Option<String>,
    pub tool_calls_json: Option<String>,
    pub sibling_index: i64,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub is_error: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationDetail {
    pub conversation: ConversationRow,
    pub messages: Vec<MessageRow>,
}

fn read_conv_row(row: &rusqlite::Row) -> rusqlite::Result<ConversationRow> {
    Ok(ConversationRow {
        id: row.get(0)?,
        title: row.get(1)?,
        title_source: row.get(2)?,
        provider_id: row.get(3)?,
        model_id: row.get(4)?,
        system_prompt: row.get(5)?,
        is_pinned: row.get::<_, i64>(6)? != 0,
        sort_order: row.get(7)?,
        generation: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        is_full: row.get::<_, i64>(11)? != 0,
        active_branch_id: row.get(12)?,
    })
}

fn read_msg_row(row: &rusqlite::Row) -> rusqlite::Result<MessageRow> {
    Ok(MessageRow {
        id: row.get("id")?,
        conversation_id: row.get("conversation_id")?,
        role: row.get("role")?,
        content: row.get("content")?,
        tokens: row.get("tokens")?,
        timestamp: row.get("timestamp")?,
        parent_id: row.get("parent_id")?,
        is_pinned: row.get::<_, i64>("is_pinned")? != 0,
        reasoning: row.get("reasoning")?,
        tool_calls_json: row.get("tool_calls_json")?,
        sibling_index: row.get("sibling_index")?,
        provider_id: row.get("provider_id")?,
        model_id: row.get("model_id")?,
        tool_call_id: row.get("tool_call_id")?,
        is_error: row.get::<_, i64>("is_error")? != 0,
    })
}

pub fn get_conversation_detail(
    conn: &Connection,
    id: &str,
) -> Result<Option<ConversationDetail>, String> {
    let conversation = crud::get_conversation_by_id(conn, id)?;
    match conversation {
        Some(conv) => {
            let messages = if let Some(ref leaf_id) = conv.active_branch_id {
                branch::trace_active_branch(conn, id, leaf_id)?
            } else {
                msg_ops::get_messages(conn, id)?
            };
            Ok(Some(ConversationDetail {
                conversation: conv,
                messages,
            }))
        }
        None => Ok(None),
    }
}

pub use branch::{
    create_branch, find_branch_leaf, get_message_siblings, switch_branch, trace_active_branch,
    update_active_branch_id, MaxSiblingsExceededError, SiblingInfo,
};
pub use compression::{
    get_compression_state, is_conversation_full, set_conversation_is_full,
    upsert_compression_state, CompressionStateRow,
};
pub use crud::{
    delete_conversation, get_conversation_by_id, get_conversations, insert_conversation,
    update_conversation,
};
pub use msg_ops::{
    get_conversation_id_by_message, get_message_by_id, get_message_count, get_message_status,
    get_messages, get_messages_paginated, get_sibling_count, get_sibling_counts_batch,
    insert_message, mark_streaming_messages_interrupted, update_message_content,
    update_message_status, update_message_tool_calls,
};
pub use ops::{
    batch_delete, duplicate_conversation, export_conversation_json, export_conversation_markdown,
    fork_to_new_conversation, increment_generation, purge_all, rename_conversation,
    search_conversations, toggle_pin, COPY_TITLE_SOURCE,
};
pub use tree_ops::{delete_subtree, get_subtree_size};
