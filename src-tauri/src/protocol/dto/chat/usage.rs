use assistant_providers::types::chat_delta::TokenUsage;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageDto {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub cache_read_tokens: Option<u32>,
    #[serde(default)]
    pub cache_write_tokens: Option<u32>,
    #[serde(default)]
    pub reasoning_tokens: Option<u32>,
    #[serde(default)]
    pub context_window_tokens: Option<u32>,
}

impl From<TokenUsage> for TokenUsageDto {
    fn from(u: TokenUsage) -> Self {
        Self {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            cache_read_tokens: u.cache_read_tokens,
            cache_write_tokens: u.cache_write_tokens,
            reasoning_tokens: u.reasoning_tokens,
            context_window_tokens: u.context_window_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageOccupancyDto {
    /// `None` = 该分支无记录，UI 显示「—」
    #[serde(default)]
    pub used_tokens: Option<i64>,
    #[serde(default)]
    pub completion_tokens: Option<i64>,
    #[serde(default)]
    pub cache_read_tokens: Option<i64>,
    #[serde(default)]
    pub cache_write_tokens: Option<i64>,
    #[serde(default)]
    pub reasoning_tokens: Option<i64>,
    /// 窗口未知时必须缺省，不得填 0 或硬编码常量
    #[serde(default)]
    pub context_window_tokens: Option<u32>,
    /// 与绝对数互斥：窗口未知时缺省
    #[serde(default)]
    pub percentage: Option<f32>,
    #[serde(default)]
    pub estimated: bool,
    /// `estimated = true` 时标明来源；`false` 时缺省
    #[serde(default)]
    pub estimated_source: Option<TokenUsageEstimatedSourceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageCumulativeDto {
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

/// 估算来源：区分「记录表的估算记录」与「分支零记录时的出站投影」
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum TokenUsageEstimatedSourceDto {
    /// provider 未上报输入 token，由 `total - completion` 回退后落库
    Record,
    /// 该分支零记录，按出站投影估算（只读、不落库）
    Projection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageCacheDto {
    pub hit_tokens: i64,
    pub denominator_tokens: i64,
    /// 分母为 0 时不可判定，缺省而不是 0
    #[serde(default)]
    pub hit_rate: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageSummaryDto {
    pub occupancy: TokenUsageOccupancyDto,
    /// 该会话零记录时缺省（UI 显示「—」），不得用 0 冒充「没花过」
    #[serde(default)]
    pub cumulative: Option<TokenUsageCumulativeDto>,
    /// 无「具备缓存能力且上报缓存字段」的请求时缺省
    #[serde(default)]
    pub cache: Option<TokenUsageCacheDto>,
}

impl From<assistant_storage::token_usage::OccupancySummary> for TokenUsageOccupancyDto {
    fn from(s: assistant_storage::token_usage::OccupancySummary) -> Self {
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

impl From<assistant_storage::token_usage::CumulativeSummary> for TokenUsageCumulativeDto {
    fn from(s: assistant_storage::token_usage::CumulativeSummary) -> Self {
        Self {
            total_tokens: s.total_tokens,
            prompt_tokens: s.prompt_tokens,
            completion_tokens: s.completion_tokens,
        }
    }
}

impl From<assistant_storage::token_usage::CacheStats> for TokenUsageCacheDto {
    fn from(s: assistant_storage::token_usage::CacheStats) -> Self {
        Self {
            hit_tokens: s.hit_tokens,
            denominator_tokens: s.denominator_tokens,
            hit_rate: s.hit_rate(),
        }
    }
}

impl From<assistant_storage::token_usage::TokenUsageSummary> for TokenUsageSummaryDto {
    fn from(s: assistant_storage::token_usage::TokenUsageSummary) -> Self {
        Self {
            occupancy: s.occupancy.into(),
            cumulative: s.cumulative.map(Into::into),
            cache: s.cache.map(Into::into),
        }
    }
}
