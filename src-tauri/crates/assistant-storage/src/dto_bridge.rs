// 域类型 → DTO 桥接（I62 §4.4）：按 Rust 孤儿规则归属本 crate。

use crate::conversations::ConversationRow;
use crate::conversations::MessageRow;
use crate::conversations::SiblingInfo;
use assistant_protocol::*;

impl From<SiblingInfo> for SiblingInfoDto {
    fn from(s: SiblingInfo) -> Self {
        Self {
            id: s.id,
            sibling_index: s.sibling_index,
            content_preview: s.content_preview,
        }
    }
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

impl From<crate::token_usage::OccupancySummary> for TokenUsageOccupancyDto {
    fn from(s: crate::token_usage::OccupancySummary) -> Self {
        Self {
            used_tokens: s.used_tokens,
            completion_tokens: s.completion_tokens,
            cache_read_tokens: s.cache_read_tokens,
            cache_write_tokens: s.cache_write_tokens,
            reasoning_tokens: s.reasoning_tokens,
            context_window_tokens: s.context_window_tokens,
            percentage: s.percentage,
            estimated: s.estimated,
            estimated_source: s.estimated.then_some(TokenUsageEstimatedSourceDto::Record),
        }
    }
}

impl From<crate::token_usage::CumulativeSummary> for TokenUsageCumulativeDto {
    fn from(s: crate::token_usage::CumulativeSummary) -> Self {
        Self {
            total_tokens: s.total_tokens,
            prompt_tokens: s.prompt_tokens,
            completion_tokens: s.completion_tokens,
        }
    }
}

impl From<crate::token_usage::CacheStats> for TokenUsageCacheDto {
    fn from(s: crate::token_usage::CacheStats) -> Self {
        Self {
            hit_tokens: s.hit_tokens,
            denominator_tokens: s.denominator_tokens,
            hit_rate: s.hit_rate(),
        }
    }
}

impl From<crate::token_usage::TokenUsageSummary> for TokenUsageSummaryDto {
    fn from(s: crate::token_usage::TokenUsageSummary) -> Self {
        Self {
            occupancy: s.occupancy.into(),
            cumulative: s.cumulative.map(Into::into),
            cache: s.cache.map(Into::into),
        }
    }
}

impl From<&crate::agent_modes::AgentModeRow> for AgentModeDto {
    fn from(row: &crate::agent_modes::AgentModeRow) -> Self {
        // 旧三列（`tools` / `max_tool_tier` / `approval_level`）已手动删除，代码侧不再创建、不再读取：
        // `tool_config_json` 缺失时按契约缺省兜底；内置行的模式初始值由 storage 种子落库。
        let tool_config = row
            .tool_config_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_default();
        Self {
            id: row.id.clone(),
            name: row.name.clone(),
            description: row.description.clone(),
            kind: AgentModeKindDto::from(row.kind.as_str()),
            prompt: row.prompt.clone(),
            model: row.model.clone(),
            temperature: row.temperature,
            max_tokens: row.max_tokens,
            reasoning_enabled: row.reasoning_enabled,
            reasoning_effort: row.reasoning_effort.clone(),
            presentation_rank: row.presentation_rank,
            is_builtin: row.is_builtin,
            created_at: row.created_at,
            updated_at: row.updated_at,
            tool_config,
            avatar: row.avatar.clone(),
        }
    }
}
