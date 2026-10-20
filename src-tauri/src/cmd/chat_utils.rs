use log::info;

use assistant_agent::chat_runtime::{self, PartialOutcome, StreamOutcome};
use assistant_providers::base;
use assistant_storage::providers::{self as provider_store};

pub(crate) fn resolve_context_window(
    model: &str,
    provider_row: &provider_store::ProviderRow,
) -> u32 {
    base::resolve_context_window_from_stored(model, &provider_row.models_json)
}

pub(crate) fn resolve_final_tokens(outcome: &StreamOutcome) -> Option<i64> {
    if outcome.total_tokens.is_none() && !outcome.full_content.is_empty() {
        let estimated = chat_runtime::estimate_tokens(&outcome.full_content).unwrap();
        info!(
            "[chat-delta] stream finished: reason={:?} no_usage=>estimated={} (content_len={})",
            outcome.finish_reason,
            estimated,
            outcome.full_content.len()
        );
        Some(estimated)
    } else {
        info!(
            "[chat-delta] stream finished: reason={:?} total_tokens={:?}",
            outcome.finish_reason, outcome.total_tokens
        );
        outcome.total_tokens.map(|t| t as i64)
    }
}

pub(crate) fn resolve_partial_tokens(partial: &PartialOutcome) -> Option<i64> {
    partial
        .total_tokens
        .map(|t| t as i64)
        .or_else(|| chat_runtime::estimate_tokens(&partial.full_content))
}
