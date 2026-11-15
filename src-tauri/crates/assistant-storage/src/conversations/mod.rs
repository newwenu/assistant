use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub mod branch;
pub mod compression;
pub mod crud;
pub mod msg_ops;
pub mod ops;
pub mod tree_ops;

const CONV_SELECT: &str = "id, title, title_source, provider_id, model_id, system_prompt, is_pinned, sort_order, generation, created_at, updated_at, is_full, active_branch_id, mode_id, tool_config_json, workspace_root";
const MSG_SELECT: &str =
    "id, conversation_id, role, content, tokens, timestamp, parent_id, is_pinned, reasoning, tool_calls_json, sibling_index, provider_id, model_id, tool_call_id, is_error, status, mode_id, turn_index, iteration";

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
    pub mode_id: Option<String>,
    pub tool_config_json: Option<String>,
    pub workspace_root: String,
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
    pub status: String,
    /// 该消息落库时生效的模式 id（I43 §3.5）。`NULL` = 未知来源
    /// （存量行 / 压缩摘要 / user 行），渲染侧走降级链。
    pub mode_id: Option<String>,
    /// 会话内单调 turn 序号（I46 D2）。`NULL` = 未知来源（存量行 / 压缩摘要），
    /// 渲染侧据此走平铺降级分支，禁止用相邻关系推断。
    pub turn_index: Option<i64>,
    /// turn 内迭代序号（I46 D2）。0 = 非迭代行（user / compact_summary）。
    pub iteration: i64,
}

/// 工具结果行的两个在途状态（D27）：`pending` = 已声明、未进入执行队列；
/// `running` = 已进入执行队列、结果未落库。`completed` = 有真实结果。
///
/// 只有 `pending` 能断言「工具没跑过」，其余一律按「结果未知」保守处理——中断可能落在
/// 执行中的任意一点，无法判断副作用是否已发生。参考 dsh 的
/// `TOOL_NOT_STARTED` / `TOOL_OUTCOME_UNKNOWN` 两级语义。
pub const TOOL_ROW_PENDING: &str = "pending";
pub const TOOL_ROW_RUNNING: &str = "running";

/// 该状态是否属于「占位行」：有行、但还没有真实结果。
pub fn is_tool_row_in_flight(status: &str) -> bool {
    status == TOOL_ROW_PENDING || status == TOOL_ROW_RUNNING
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
        mode_id: row.get(13)?,
        tool_config_json: row.get(14)?,
        workspace_root: row.get(15)?,
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
        status: row.get("status")?,
        mode_id: row.get("mode_id")?,
        turn_index: row.get("turn_index")?,
        iteration: row.get("iteration")?,
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

/// 孤立工具调用的中断结果文案（D27）：调用已进入执行队列，但没有结果。
///
/// 刻意不写「未执行」：中断可能落在「已进入队列 → 执行中 → 结果落库」之间任意一点，
/// 仅凭「有调用、无结果」无法判断工具是否已产生副作用。参考 dsh 的
/// `TOOL_NOT_STARTED` / `TOOL_OUTCOME_UNKNOWN` 两级语义，本项目用占位行的 `status`
/// 记录阶段，本常量只服务后者；前者见 `TOOL_NOT_STARTED_RESULT`。
///
/// 单一定义在 storage：投影期合成与复制/分叉承接共用同一句，模型看到的历史不会分叉。
/// 保持一句话：每个缺失调用各写一份，长文案会按批大小成倍占用上下文。
pub const INTERRUPTED_TOOL_RESULT: &str = "本轮中断，结果未知：只读或幂等可重试，有副作用先核实。";

/// 孤立工具调用的中断结果文案（D27）：已声明、但从未进入执行队列，可安全重试。
pub const TOOL_NOT_STARTED_RESULT: &str = "本轮中断，未开始执行：可按需重试。";

pub use branch::{
    advance_active_branch, create_branch, find_branch_leaf, get_message_siblings, switch_branch,
    trace_active_branch, update_active_branch_id, MaxSiblingsExceededError, SiblingInfo,
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
    allocate_iteration, allocate_turn_index, fill_tool_row, find_tool_placeholder,
    get_conversation_id_by_message, get_message_by_id, get_message_count, get_message_status,
    get_messages, get_messages_paginated, get_sibling_count, get_sibling_counts_batch,
    get_tool_results, insert_message, insert_tool_placeholders, mark_tool_rows_running,
    mark_tool_rows_unknown, update_message_content, update_message_status,
    update_message_tool_calls, TURNS_PER_PAGE,
};
pub use ops::{
    batch_delete, duplicate_conversation, export_conversation_json, export_conversation_markdown,
    fork_to_new_conversation, increment_generation, purge_all, rename_conversation,
    search_conversations, toggle_pin, CONVERSATION_RAW_SCHEMA, COPY_TITLE_SOURCE,
};
pub use tree_ops::{delete_subtree, get_subtree_size};
