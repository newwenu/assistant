use assistant_providers::base;
use assistant_storage::providers as provider_store;

/// 适配层声明的缓存能力，不能靠「是否上报缓存字段」或 0 值反推
pub(crate) fn cache_capable_of(provider_row: &provider_store::ProviderRow) -> bool {
    base::provider_kind_from_str(&provider_row.type_, provider_row.base_url.clone())
        .cache_capability()
        .is_capable()
}
