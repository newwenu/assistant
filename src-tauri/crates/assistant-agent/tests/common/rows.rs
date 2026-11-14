//! storage 行最小构造：只保留决定行为的字段，其余取固定缺省值。
//!
//! 与 storage 的 `ConversationRow` / `MessageRow` 字段增删解耦：新增字段不必逐个改测试。

#![allow(dead_code)]

use assistant_storage::conversations::{ConversationRow, MessageRow};

pub fn conversation_row(id: &str, model_id: &str) -> ConversationRow {
    ConversationRow {
        id: id.to_string(),
        title: "fixture".to_string(),
        title_source: "user".to_string(),
        provider_id: None,
        model_id: model_id.to_string(),
        system_prompt: None,
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: 0,
        updated_at: 0,
        is_full: false,
        active_branch_id: None,
        mode_id: None,
        tool_config_json: None,
        workspace_root: String::new(),
    }
}

pub fn message_row(id: &str, conversation_id: &str, role: &str, content: &str) -> MessageRow {
    MessageRow {
        id: id.to_string(),
        conversation_id: conversation_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        tokens: None,
        timestamp: 0,
        parent_id: None,
        is_pinned: false,
        reasoning: None,
        tool_calls_json: None,
        sibling_index: 0,
        provider_id: None,
        model_id: None,
        tool_call_id: None,
        is_error: false,
        status: "completed".to_string(),
        mode_id: None,
        turn_index: None,
        iteration: 0,
    }
}
