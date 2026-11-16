use assistant_storage::conversations::MessageRow;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::super::agent::ToolResultCompactDto;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum MessageToolCallStatusDto {
    Success,
    Error,
    Cancelled,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageToolCallDto {
    pub call_id: String,
    pub name: String,
    #[ts(type = "unknown")]
    pub arguments: serde_json::Value,
    pub status: MessageToolCallStatusDto,
    pub result: Option<String>,
    /// 展示投影：历史重载的渲染事实源（旧数据由后端重建，重建不出则为 `null`）。
    pub compact: Option<ToolResultCompactDto>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatusDto {
    Streaming,
    Completed,
    Interrupted,
    Failed,
}

impl MessageStatusDto {
    /// 存储层 `messages.status` → DTO 封闭集合；未知取值一律按 `completed` 兜底。
    pub fn from_db(raw: &str) -> Self {
        match raw {
            "streaming" => Self::Streaming,
            "interrupted" => Self::Interrupted,
            "failed" => Self::Failed,
            _ => Self::Completed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageDto {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub tokens: Option<i64>,
    pub timestamp: i64,
    pub parent_id: Option<String>,
    pub is_pinned: bool,
    pub reasoning: Option<String>,
    #[serde(default)]
    pub sibling_index: i64,
    #[serde(default)]
    pub sibling_count: Option<i64>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    /// 该消息落库时生效的模式 id（I43 §3.5）：键恒在，`null` = 未知来源，
    /// 渲染侧走降级链（内置头像 → 通用助手头像）。
    pub mode_id: Option<String>,
    /// 消息终态（I41 §1）：取消落 `interrupted`，前端据此呈现「已中断」，
    /// 不再依赖 `error` 承载取消语义。
    pub status: MessageStatusDto,
    #[serde(default)]
    pub tool_calls: Vec<MessageToolCallDto>,
    /// 会话内单调 turn 序号（从 1 起）。`null` = 未知（存量行 / 未确认的乐观行）。
    pub turn_index: Option<u32>,
    /// turn 内迭代序号。`0` = 非迭代行（user / compact_summary）。
    pub iteration: u32,
}

impl From<MessageRow> for MessageDto {
    fn from(r: MessageRow) -> Self {
        Self {
            id: r.id,
            conversation_id: r.conversation_id,
            role: r.role,
            content: r.content,
            tokens: r.tokens,
            timestamp: r.timestamp,
            parent_id: r.parent_id,
            is_pinned: r.is_pinned,
            reasoning: r.reasoning,
            sibling_index: r.sibling_index,
            sibling_count: None,
            provider_id: r.provider_id,
            model_id: r.model_id,
            mode_id: r.mode_id,
            status: MessageStatusDto::from_db(&r.status),
            tool_calls: Vec::new(),
            turn_index: r.turn_index.and_then(|v| u32::try_from(v).ok()),
            iteration: u32::try_from(r.iteration).unwrap_or(0),
        }
    }
}

/// 落库原文视图（I51 §6.1）：字段与 `MessageRow` 一一对应，`content` / `tool_calls_json`
/// 均为未解析的存储原串，供历史详情页「原始来源」与 JSONL 导出使用。
/// 输出专用 DTO，禁止作为任何命令入参。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageRecordDto {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    /// 落库原文：可能是纯文本，也可能是 content blocks 的 JSON 串。
    pub content: String,
    pub reasoning: Option<String>,
    /// `tool_calls_json` 落库原串，未解析。
    pub tool_calls_json: Option<String>,
    pub tokens: Option<i64>,
    pub timestamp: i64,
    pub parent_id: Option<String>,
    pub sibling_index: i64,
    pub is_pinned: bool,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub mode_id: Option<String>,
    /// 存储层原始状态串（`completed` / `interrupted` / `failed` / `pending` / `running`），
    /// 不做封闭集合收窄，保留落库事实。
    pub status: String,
    pub tool_call_id: Option<String>,
    pub is_error: bool,
    pub turn_index: Option<u32>,
    pub iteration: u32,
}

impl From<MessageRow> for MessageRecordDto {
    fn from(r: MessageRow) -> Self {
        Self {
            id: r.id,
            conversation_id: r.conversation_id,
            role: r.role,
            content: r.content,
            reasoning: r.reasoning,
            tool_calls_json: r.tool_calls_json,
            tokens: r.tokens,
            timestamp: r.timestamp,
            parent_id: r.parent_id,
            sibling_index: r.sibling_index,
            is_pinned: r.is_pinned,
            provider_id: r.provider_id,
            model_id: r.model_id,
            mode_id: r.mode_id,
            status: r.status,
            tool_call_id: r.tool_call_id,
            is_error: r.is_error,
            turn_index: r.turn_index.and_then(|v| u32::try_from(v).ok()),
            iteration: u32::try_from(r.iteration).unwrap_or(0),
        }
    }
}
