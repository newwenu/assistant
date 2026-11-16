use assistant_storage::conversations::ConversationRow;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::super::agent::ConversationToolConfigDto;
use super::message::{MessageDto, MessageRecordDto};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationDto {
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
    #[serde(default)]
    pub active_branch_id: Option<String>,
    #[serde(default)]
    pub mode_id: Option<String>,
    /// 会话绑定的工作区根；空串表示未配置，wire 上永不出现 `null`（I38h §8.1）。
    #[serde(default)]
    pub workspace_root: String,
    /// 会话级工具配置；键恒在，未配置时为 `null`（agent 链路回退到模式初始值）。
    #[serde(default)]
    pub tool_config: Option<ConversationToolConfigDto>,
}

impl From<ConversationRow> for ConversationDto {
    fn from(r: ConversationRow) -> Self {
        let tool_config = r
            .tool_config_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok());
        Self {
            id: r.id,
            title: r.title,
            title_source: r.title_source,
            provider_id: r.provider_id,
            model_id: r.model_id,
            system_prompt: r.system_prompt,
            is_pinned: r.is_pinned,
            sort_order: r.sort_order,
            generation: r.generation,
            created_at: r.created_at,
            updated_at: r.updated_at,
            is_full: r.is_full,
            active_branch_id: r.active_branch_id,
            mode_id: r.mode_id,
            workspace_root: r.workspace_root,
            tool_config,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationDetailDto {
    pub conversation: ConversationDto,
    pub messages: Vec<MessageDto>,
    pub total_count: i64,
    /// 落库原文行（I51 §6.3）：仅当 `ConversationQueryOptsDto::include_records` 为真时填充，
    /// 其余查询模式下键恒在、值为 `null`。历史详情页的「原始来源」视图消费该字段，
    /// 避免为每行单独发一次 IPC。
    pub records: Option<Vec<MessageRecordDto>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationPatchDto {
    #[ts(optional = nullable)]
    pub title: Option<String>,
    #[ts(optional = nullable)]
    pub title_source: Option<String>,
    #[ts(optional = nullable)]
    pub model_id: Option<String>,
    #[ts(optional = nullable)]
    pub provider_id: Option<String>,
    /// 会话当前模式；切换只影响下一次发送，不回溯已有消息（I38d §4.8）。
    #[ts(optional = nullable)]
    pub mode_id: Option<String>,
    /// 会话级工具配置；缺省或 `null` 表示不改。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub tool_config: Option<ConversationToolConfigDto>,
    /// 工作区根（输入三态）：缺省或 `null` = 不改；有值 = 校验后写入；空串一律 `Err`。
    /// 清除绑定请用 `clear_workspace_root`（I38h §8.2）。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub workspace_root: Option<String>,
    /// 显式清除工作区绑定（写回空串）；与 `workspace_root` 同时出现视为歧义。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub clear_workspace_root: Option<bool>,
}

/// 会话详情查询模式（封闭集合）。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
pub enum ConversationQueryModeDto {
    #[default]
    Paginated,
    All,
}

impl ConversationQueryModeDto {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paginated => "paginated",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationQueryOptsDto {
    #[serde(default)]
    pub mode: ConversationQueryModeDto,
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// 分页游标（I46 §7.1）：取该 turn 序号之前的页；`null` = 最新一页。
    /// 由 `beforeTimestamp` 改为 turn 边界，保证一页内不切开 turn。
    #[ts(optional = nullable)]
    pub before_turn_index: Option<u32>,
    /// 是否随详情一并返回落库原文行（I51 §6.3）。默认 `false`，只有历史详情页的
    /// 原始视图需要时才置 `true`，避免主会话链路为空载字段付出体积成本。
    #[serde(default)]
    pub include_records: bool,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendChatMessageArgsDto {
    pub content: String,
    #[ts(optional = nullable)]
    pub mode_id: Option<String>,
    #[ts(optional = nullable)]
    pub branch_point_message_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    #[ts(optional = nullable)]
    pub reasoning_level: Option<String>,
    /// 每轮有效工具配置（agent 链路 = 会话持久值 ?? 模式初始值；chat 链路恒 `null`）。
    /// 缺省即 deny-by-default：模型看不到任何工具。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub tool_config: Option<ConversationToolConfigDto>,
}
