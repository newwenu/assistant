use assistant_providers::base::{self, ProviderConfig};
use assistant_storage::providers::ProviderRow;

/// `ProviderRow`（storage）→ `ProviderConfig`（providers）桥接。
/// assistant-agent 同时依赖两者，是该转换的自然宿主；
/// providers 保持不依赖 storage（分层纯净考虑，非循环依赖约束）。
///
/// `ProviderConfig` 不含 API key；凭证由调用方在使用时注入，
/// 不随 config 层层传递。
pub fn build_provider_config(row: &ProviderRow) -> ProviderConfig {
    ProviderConfig {
        kind: base::provider_kind_from_str(&row.type_, row.base_url.clone()),
        base_url: Some(row.base_url.clone()),
        default_model: row.default_model.clone(),
        max_tokens: 4096,
        temperature: 0.7,
    }
}
