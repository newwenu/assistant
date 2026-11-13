use crate::capability::{CapabilityFact, CapabilityFeature, Provenance, Support};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::time::Duration;

/// 逐项容错解析 JSON 数组：整体解析失败 → 记 error 并返回空；
/// 单项解析失败 → 记 warn 并跳过该项，其余项保留。
/// `ctx` 用于日志定位，形如 `models_json:<provider_id>`。
pub fn parse_items_lossy<T: DeserializeOwned>(json: &str, ctx: &str) -> Vec<T> {
    let values: Vec<serde_json::Value> = match serde_json::from_str(json) {
        Ok(values) => values,
        Err(e) => {
            log::error!("{ctx}: failed to parse json array: {e}");
            return Vec::new();
        }
    };
    values
        .into_iter()
        .enumerate()
        .filter_map(|(idx, value)| match serde_json::from_value::<T>(value) {
            Ok(item) => Some(item),
            Err(e) => {
                log::warn!("{ctx}: skipping invalid item at index {idx}: {e}");
                None
            }
        })
        .collect()
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<super::tool::ToolSpec>>,
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
    /// 视觉能力事实；存量数据无此字段，读取时回退旧字段派生
    #[serde(default)]
    vision: Option<CapabilityFact>,
    /// 原生 PDF 能力事实；存量数据无此字段，读取时回退旧字段派生
    #[serde(default)]
    pdf: Option<CapabilityFact>,
    /// 附件能力事实；旧字段无法派生，缺失即 `Unknown / Fallback`
    #[serde(default)]
    attachment: Option<CapabilityFact>,
}

fn parse_stored_models(models_json: &str) -> Vec<StoredModelEntry> {
    parse_items_lossy(models_json, "models_json")
}

pub fn resolve_context_window_from_stored(model_id: &str, models_json: &str) -> Option<u32> {
    let stored = parse_stored_models(models_json)
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.context_window)
        .filter(|&c| c > 0);
    resolve_context_window(model_id, stored, None)
}

/// 视觉能力事实：优先读取快照里的 `vision`；存量数据无该字段时按旧口径派生
/// （`inputModalities` 非空以其为准，否则回退 `supportVision` 布尔），`provenance = Snapshot`。
/// 模型不在快照列表 → `Unknown / Fallback`（不再写成 `false`）。
pub fn resolve_vision_from_stored(model_id: &str, models_json: &str) -> CapabilityFact {
    resolve_feature_from_stored(model_id, models_json, CapabilityFeature::Vision)
}

/// 原生 PDF 能力事实：优先读取快照里的 `pdf`，存量数据按模态列表是否含 `pdf` 派生。
pub fn resolve_pdf_from_stored(model_id: &str, models_json: &str) -> CapabilityFact {
    resolve_feature_from_stored(model_id, models_json, CapabilityFeature::Pdf)
}

/// 附件能力事实：快照无该字段时无法派生，返回 `Unknown / Fallback`。
pub fn resolve_attachment_from_stored(model_id: &str, models_json: &str) -> CapabilityFact {
    resolve_feature_from_stored(model_id, models_json, CapabilityFeature::Attachment)
}

/// 输入模态列表：`inputModalities` 非空时以其为准，存量数据无该字段时由
/// `supportVision` 派生；模型不在快照列表 → `None`（不再兜底 `["text"]`）。
pub fn resolve_input_modalities_from_stored(
    model_id: &str,
    models_json: &str,
) -> Option<Vec<String>> {
    parse_stored_models(models_json)
        .iter()
        .find(|m| m.id == model_id)
        .map(resolve_entry_modalities)
}

fn resolve_feature_from_stored(
    model_id: &str,
    models_json: &str,
    feature: CapabilityFeature,
) -> CapabilityFact {
    let models = parse_stored_models(models_json);
    let Some(entry) = models.iter().find(|m| m.id == model_id) else {
        return CapabilityFact::unknown(Provenance::Fallback);
    };
    match feature {
        CapabilityFeature::Vision => entry
            .vision
            .unwrap_or_else(|| legacy_modality_fact(entry, "image")),
        CapabilityFeature::Pdf => entry
            .pdf
            .unwrap_or_else(|| legacy_modality_fact(entry, "pdf")),
        CapabilityFeature::Attachment => entry
            .attachment
            .unwrap_or_else(|| CapabilityFact::unknown(Provenance::Fallback)),
    }
}

/// 存量兼容：旧字段派生的结论记为 `Snapshot`（未记录原始来源，保持旧行为不变）。
fn legacy_modality_fact(entry: &StoredModelEntry, modality: &str) -> CapabilityFact {
    let modalities = resolve_entry_modalities(entry);
    CapabilityFact::new(
        Support::from_bool(modalities.iter().any(|m| m == modality)),
        Provenance::Snapshot,
    )
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

    /// 与 `src-tauri/src/cmd/provider.rs` 单测共用同一样本：两处解析点必须给出相同项数
    const MIXED_MODELS_JSON: &str = r#"[
        {"id":"good1","name":"Good 1","ownedBy":"x"},
        {"name":"broken"},
        {"id":"good2","name":"Good 2","ownedBy":"x"}
    ]"#;

    #[test]
    fn parse_items_lossy_keeps_good_items() {
        let items: Vec<StoredModelEntry> = parse_items_lossy(MIXED_MODELS_JSON, "models_json:test");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn parse_items_lossy_broken_or_empty_json_yields_no_items() {
        let items: Vec<StoredModelEntry> = parse_items_lossy("not-json", "models_json:test");
        assert!(items.is_empty());
        let items: Vec<StoredModelEntry> = parse_items_lossy("[]", "models_json:test");
        assert!(items.is_empty());
    }

    #[test]
    fn one_bad_item_does_not_clear_the_rest() {
        // 中间一项缺 id（必填）→ 只跳过该项，其余项照常生效
        let json = r#"[
            {"id":"gpt-4o","contextWindow":128000},
            {"name":"broken"},
            {"id":"m2","contextWindow":4096,"supportVision":true}
        ]"#;
        assert_eq!(
            resolve_context_window_from_stored("gpt-4o", json),
            Some(128_000)
        );
        assert_eq!(resolve_context_window_from_stored("m2", json), Some(4096));
        assert!(resolve_vision_from_stored("m2", json).is_supported());
    }

    #[test]
    fn vision_from_stored_three_states() {
        assert!(resolve_vision_from_stored("gpt-4o", SAMPLE).is_supported());
        // 缺省 supportVision 字段（serde default）→ Unsupported（存量派生，非 Unknown）
        let mini = resolve_vision_from_stored("gpt-4o-mini", SAMPLE);
        assert_eq!(mini.support, Support::Unsupported);
        assert_eq!(mini.provenance, Provenance::Snapshot);
        // 模型缺失 → Unknown（不再写成 false）
        let missing = resolve_vision_from_stored("no-such-model", SAMPLE);
        assert!(missing.is_unknown());
        assert_eq!(missing.provenance, Provenance::Fallback);
        // 解析失败 → Unknown
        assert!(resolve_vision_from_stored("gpt-4o", "not-json").is_unknown());
    }

    #[test]
    fn input_modalities_win_over_support_vision_bool() {
        // inputModalities 含 image → Supported（即便 supportVision 为 false）
        let json = r#"[{"id":"m1","contextWindow":4096,"supportVision":false,"inputModalities":["text","image"]}]"#;
        assert!(resolve_vision_from_stored("m1", json).is_supported());
        // inputModalities 非空且不含 image → Unsupported（权威来源，即便 supportVision 为 true）
        let json2 =
            r#"[{"id":"m1","contextWindow":4096,"supportVision":true,"inputModalities":["text"]}]"#;
        assert_eq!(
            resolve_vision_from_stored("m1", json2).support,
            Support::Unsupported
        );
    }

    #[test]
    fn empty_modalities_falls_back_to_bool() {
        // 空数组视同未声明 → 回退 supportVision 布尔
        let json =
            r#"[{"id":"m1","contextWindow":4096,"supportVision":true,"inputModalities":[]}]"#;
        assert!(resolve_vision_from_stored("m1", json).is_supported());
    }

    #[test]
    fn unknown_fields_are_tolerated() {
        // 前端未来新增字段不应破坏解析
        let json = r#"[{"id":"m1","name":"M1","ownedBy":"x","contextWindow":4096,"supportVision":true,"newField":"whatever"}]"#;
        assert_eq!(resolve_context_window_from_stored("m1", json), Some(4096));
        assert!(resolve_vision_from_stored("m1", json).is_supported());
    }

    #[test]
    fn snake_case_keys_do_not_win() {
        // base::ModelConfig 的 snake_case 格式不是存储契约，此类数据应回退
        let json = r#"[{"id":"m1","name":"M1","owned_by":"x","context_window":4096,"support_vision":true}]"#;
        assert_eq!(
            resolve_vision_from_stored("m1", json).support,
            Support::Unsupported
        );
        // 存储契约未命中且目录内无此模型 → 窗口未知，返回 None
        assert!(resolve_context_window_from_stored("m1", json).is_none());
    }

    #[test]
    fn stored_fact_wins_over_legacy_derivation() {
        // 快照里已有三态事实 → 直接采用，不再按模态派生
        let json = r#"[{"id":"m1","supportVision":true,"inputModalities":["text"],"vision":{"support":"unsupported","source":"api"}}]"#;
        let fact = resolve_vision_from_stored("m1", json);
        assert_eq!(fact.support, Support::Unsupported);
        assert_eq!(fact.provenance, Provenance::ApiDeclared);
    }

    #[test]
    fn pdf_and_attachment_facts_from_stored() {
        let json = r#"[{"id":"m1","inputModalities":["text","pdf"],"pdf":{"support":"unsupported","source":"api"},"attachment":{"support":"supported","source":"catalog"}}]"#;
        // 快照里的 pdf 事实优先于模态派生
        assert_eq!(
            resolve_pdf_from_stored("m1", json).support,
            Support::Unsupported
        );
        assert!(resolve_attachment_from_stored("m1", json).is_supported());

        // 存量数据：pdf 由模态派生，attachment 无旧字段可派生 → Unknown
        let legacy = r#"[{"id":"m2","inputModalities":["text","pdf"]}]"#;
        let pdf = resolve_pdf_from_stored("m2", legacy);
        assert!(pdf.is_supported());
        assert_eq!(pdf.provenance, Provenance::Snapshot);
        assert!(resolve_attachment_from_stored("m2", legacy).is_unknown());
    }

    #[test]
    fn same_model_id_in_different_providers_differs() {
        // 两个 provider 各自的 models_json 对同一 model_id 得出不同结论（不再「后写者胜」）
        let provider_a =
            r#"[{"id":"shared-model","vision":{"support":"supported","source":"api"}}]"#;
        let provider_b =
            r#"[{"id":"shared-model","vision":{"support":"unsupported","source":"catalog"}}]"#;
        assert!(resolve_vision_from_stored("shared-model", provider_a).is_supported());
        assert_eq!(
            resolve_vision_from_stored("shared-model", provider_b).support,
            Support::Unsupported
        );
    }

    #[test]
    fn input_modalities_returns_none_when_model_absent() {
        assert_eq!(
            resolve_input_modalities_from_stored("gpt-4o", SAMPLE),
            Some(vec!["text".to_string(), "image".to_string()])
        );
        assert_eq!(
            resolve_input_modalities_from_stored("no-such-model", SAMPLE),
            None
        );
    }
}
