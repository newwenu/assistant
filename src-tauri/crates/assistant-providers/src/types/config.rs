use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ChatOptions {
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,
    #[serde(default)]
    pub reasoning_enabled: Option<bool>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub rate_limit_base_ms: u64,
    pub rate_limit_max_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 500,
            max_delay_ms: 30_000,
            rate_limit_base_ms: 2_000,
            rate_limit_max_ms: 60_000,
        }
    }
}

impl RetryConfig {
    pub fn delay_ms(&self, attempt: u32, error: &crate::base::ProviderError) -> u64 {
        let shift = attempt.min(6);
        let is_rate_limit = matches!(error, crate::base::ProviderError::RateLimited { .. });
        let fallback = if is_rate_limit {
            (self.rate_limit_base_ms << shift).min(self.rate_limit_max_ms)
        } else {
            (self.base_delay_ms << shift).min(self.max_delay_ms)
        };
        match error.retry_after_ms() {
            Some(hint) if hint > 0 && is_rate_limit => {
                hint.max(fallback).min(self.rate_limit_max_ms)
            }
            Some(hint) if hint > 0 => hint.min(self.max_delay_ms),
            _ => fallback,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StreamTimeoutConfig {
    pub first_token_timeout: Duration,
    pub idle_timeout: Duration,
    pub total_timeout: Duration,
}

impl Default for StreamTimeoutConfig {
    fn default() -> Self {
        Self {
            first_token_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(30),
            total_timeout: Duration::from_secs(120),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SseLogConfig {
    pub max_events: usize,
    pub max_retained_bytes: usize,
}

impl Default for SseLogConfig {
    fn default() -> Self {
        Self {
            max_events: 256,
            max_retained_bytes: 1_048_576,
        }
    }
}

/// 窗口解析：模型配置 → provider 发现 → 内置目录，全部取不到时返回 `None`。
pub fn resolve_context_window(
    model_id: &str,
    stored: Option<u32>,
    api_context: Option<u32>,
) -> Option<u32> {
    stored
        .filter(|&ctx| ctx > 0)
        .or_else(|| api_context.filter(|&ctx| ctx > 0))
        .or_else(|| crate::model_catalog::get_context_window_tokens(model_id))
}

/// models_json 由前端写入（save_provider 直接入库），键为 camelCase
/// （ownedBy/contextWindow/supportVision/inputModalities），与 `base::ModelConfig`
/// （适配器发现层的 snake_case Rust 内部类型）不同，故使用局部解析结构。
/// 不带 deny_unknown_fields：前端新增字段不应导致存量解析整体失败。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredModelEntry {
    id: String,
    #[serde(default)]
    context_window: u32,
    #[serde(default)]
    support_vision: bool,
    #[serde(default)]
    input_modalities: Vec<String>,
}

fn parse_stored_models(models_json: &str) -> Vec<StoredModelEntry> {
    serde_json::from_str(models_json).unwrap_or_default()
}

pub fn resolve_context_window_from_stored(model_id: &str, models_json: &str) -> Option<u32> {
    let stored = parse_stored_models(models_json)
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.context_window)
        .filter(|&c| c > 0);
    resolve_context_window(model_id, stored, None)
}

/// 视觉能力判定：`inputModalities` 为权威来源（非空时以其为准，
/// 含 "image" 即支持）；存量数据无该字段时回退 `supportVision` 布尔。
pub fn resolve_support_vision_from_stored(model_id: &str, models_json: &str) -> bool {
    resolve_input_modalities_from_stored(model_id, models_json)
        .iter()
        .any(|m| m == "image")
}

/// 输入模态列表：`inputModalities` 非空时以其为准；
/// 存量数据无该字段时由 `supportVision` 派生为 text/image。
pub fn resolve_input_modalities_from_stored(model_id: &str, models_json: &str) -> Vec<String> {
    parse_stored_models(models_json)
        .iter()
        .find(|m| m.id == model_id)
        .map(resolve_entry_modalities)
        .unwrap_or_else(|| vec!["text".to_string()])
}

pub fn resolve_support_pdf_from_stored(model_id: &str, models_json: &str) -> bool {
    resolve_input_modalities_from_stored(model_id, models_json)
        .iter()
        .any(|m| m == "pdf")
}

fn resolve_entry_modalities(m: &StoredModelEntry) -> Vec<String> {
    if !m.input_modalities.is_empty() {
        return m.input_modalities.clone();
    }
    if m.support_vision {
        vec!["text".to_string(), "image".to_string()]
    } else {
        vec!["text".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 前端写入的真实格式样本（camelCase）
    const SAMPLE: &str = r#"[
        {"id":"gpt-4o","name":"GPT-4o","ownedBy":"openai","contextWindow":128000,"supportVision":true},
        {"id":"gpt-4o-mini","name":"GPT-4o mini","ownedBy":"openai","contextWindow":0}
    ]"#;

    #[test]
    fn stored_context_window_wins_over_catalog() {
        assert_eq!(
            resolve_context_window_from_stored("gpt-4o", SAMPLE),
            Some(128_000)
        );
    }

    #[test]
    fn zero_or_missing_context_falls_back_to_catalog() {
        // contextWindow: 0 被过滤，走内置 catalog
        assert!(resolve_context_window_from_stored("gpt-4o-mini", SAMPLE).is_some());
        // 模型不在存储列表且不在目录内 → 未知，不回退硬编码常量
        assert_eq!(
            resolve_context_window_from_stored("no-such-model", SAMPLE),
            None
        );
    }

    #[test]
    fn invalid_json_falls_back_to_catalog() {
        assert!(resolve_context_window_from_stored("gpt-4o", "not-json").is_some());
    }

    #[test]
    fn support_vision_from_stored() {
        assert!(resolve_support_vision_from_stored("gpt-4o", SAMPLE));
        // 缺省 supportVision 字段（serde default）→ false
        assert!(!resolve_support_vision_from_stored("gpt-4o-mini", SAMPLE));
        // 模型缺失 → false
        assert!(!resolve_support_vision_from_stored("no-such-model", SAMPLE));
        // 解析失败 → false
        assert!(!resolve_support_vision_from_stored("gpt-4o", "not-json"));
    }

    #[test]
    fn input_modalities_win_over_support_vision_bool() {
        // inputModalities 含 image → true（即便 supportVision 为 false）
        let json = r#"[{"id":"m1","contextWindow":4096,"supportVision":false,"inputModalities":["text","image"]}]"#;
        assert!(resolve_support_vision_from_stored("m1", json));
        // inputModalities 非空且不含 image → false（权威来源，即便 supportVision 为 true）
        let json2 =
            r#"[{"id":"m1","contextWindow":4096,"supportVision":true,"inputModalities":["text"]}]"#;
        assert!(!resolve_support_vision_from_stored("m1", json2));
    }

    #[test]
    fn empty_modalities_falls_back_to_bool() {
        // 空数组视同未声明 → 回退 supportVision 布尔
        let json =
            r#"[{"id":"m1","contextWindow":4096,"supportVision":true,"inputModalities":[]}]"#;
        assert!(resolve_support_vision_from_stored("m1", json));
    }

    #[test]
    fn unknown_fields_are_tolerated() {
        // 前端未来新增字段不应破坏解析
        let json = r#"[{"id":"m1","name":"M1","ownedBy":"x","contextWindow":4096,"supportVision":true,"newField":"whatever"}]"#;
        assert_eq!(resolve_context_window_from_stored("m1", json), Some(4096));
        assert!(resolve_support_vision_from_stored("m1", json));
    }

    #[test]
    fn snake_case_keys_do_not_win() {
        // base::ModelConfig 的 snake_case 格式不是存储契约，此类数据应回退
        let json = r#"[{"id":"m1","name":"M1","owned_by":"x","context_window":4096,"support_vision":true}]"#;
        assert!(!resolve_support_vision_from_stored("m1", json));
        // 存储契约未命中且目录内无此模型 → 窗口未知，返回 None
        assert!(resolve_context_window_from_stored("m1", json).is_none());
    }
}
