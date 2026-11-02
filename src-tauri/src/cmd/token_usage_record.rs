use assistant_agent::chat_runtime::UsageSnapshot;
use assistant_providers::base;
use assistant_storage::{providers as provider_store, token_usage::TokenUsageRecord};

/// 写入 token_usage_records 所需的请求级上下文
pub(crate) struct UsageRecordCtx<'a> {
    pub conv_id: &'a str,
    pub msg_id: &'a str,
    pub attempt_id: &'a str,
    pub branch_leaf_id: Option<&'a str>,
    pub kind: &'a str,
    pub provider_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub cache_capable: bool,
}

/// 适配层声明的缓存能力，不能靠「是否上报缓存字段」或 0 值反推
pub(crate) fn cache_capable_of(provider_row: &provider_store::ProviderRow) -> bool {
    base::provider_kind_from_str(&provider_row.type_, provider_row.base_url.clone())
        .cache_capability()
        .is_capable()
}

/// 单条快照的输入/输出取值。
///
/// `prompt_tokens` 未上报时按 `total - completion` 回退；回退值不是 provider 上报值，
/// 因此必须置 `estimated`，否则它会以「精确值」身份抢占占用锚点（M3）。
fn normalize_record_tokens(u: &UsageSnapshot) -> (u32, u32, bool) {
    let completion = u.completion_tokens.unwrap_or(0);
    match u.prompt_tokens {
        Some(prompt) => (prompt, completion, u.estimated),
        None => {
            let derived = u
                .total_tokens
                .map(|total| total.saturating_sub(completion))
                .unwrap_or(0);
            (derived, completion, true)
        }
    }
}

/// 每次带 usage 的 attempt 各落一条记录，重试不覆盖
pub(crate) fn build_usage_records(
    snapshots: &[UsageSnapshot],
    ctx: &UsageRecordCtx<'_>,
) -> Vec<TokenUsageRecord> {
    let now = chrono::Utc::now().timestamp_millis();
    snapshots
        .iter()
        .map(|u| {
            let (prompt_tokens, completion_tokens, estimated) = normalize_record_tokens(u);
            TokenUsageRecord {
                conversation_id: ctx.conv_id.to_string(),
                message_id: Some(ctx.msg_id.to_string()),
                attempt_id: Some(ctx.attempt_id.to_string()),
                branch_leaf_id: ctx.branch_leaf_id.map(str::to_string),
                kind: ctx.kind.to_string(),
                provider_id: ctx.provider_id.map(str::to_string),
                model_id: ctx.model_id.map(str::to_string),
                prompt_tokens,
                completion_tokens,
                cache_read_tokens: u.cache_read_tokens,
                cache_write_tokens: u.cache_write_tokens,
                reasoning_tokens: u.reasoning_tokens,
                cache_capable: ctx.cache_capable,
                context_window_tokens: u.context_window_tokens,
                estimated,
                created_at: now,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(prompt: Option<u32>, completion: Option<u32>, total: Option<u32>) -> UsageSnapshot {
        UsageSnapshot {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
            ..Default::default()
        }
    }

    #[test]
    fn reported_prompt_keeps_provider_flag() {
        let (prompt, completion, estimated) =
            normalize_record_tokens(&snapshot(Some(100), Some(20), Some(120)));
        assert_eq!((prompt, completion), (100, 20));
        assert!(!estimated);
    }

    #[test]
    fn derived_prompt_is_marked_estimated() {
        let (prompt, completion, estimated) =
            normalize_record_tokens(&snapshot(None, Some(20), Some(120)));
        assert_eq!((prompt, completion), (100, 20));
        assert!(estimated);
    }

    #[test]
    fn missing_prompt_and_total_is_zero_and_estimated() {
        let (prompt, completion, estimated) = normalize_record_tokens(&snapshot(None, None, None));
        assert_eq!((prompt, completion), (0, 0));
        assert!(estimated);
    }

    #[test]
    fn derived_prompt_never_goes_negative() {
        let (prompt, _, estimated) = normalize_record_tokens(&snapshot(None, Some(200), Some(120)));
        assert_eq!(prompt, 0);
        assert!(estimated);
    }

    fn ctx(kind: &'static str) -> UsageRecordCtx<'static> {
        UsageRecordCtx {
            conv_id: "c1",
            msg_id: "m1",
            attempt_id: "c1:1",
            branch_leaf_id: Some("b1"),
            kind,
            provider_id: Some("p1"),
            model_id: Some("gpt-4"),
            cache_capable: true,
        }
    }

    #[test]
    fn every_attempt_produces_its_own_record() {
        let snapshots = vec![
            snapshot(Some(100), Some(10), Some(110)),
            snapshot(Some(120), Some(20), Some(140)),
        ];
        let records = build_usage_records(&snapshots, &ctx("chat"));

        assert_eq!(
            records.len(),
            2,
            "retry must not overwrite the first attempt"
        );
        assert_eq!(records[0].prompt_tokens, 100);
        assert_eq!(records[1].prompt_tokens, 120);
        assert_eq!(records[0].attempt_id.as_deref(), Some("c1:1"));
    }

    #[test]
    fn compaction_produces_single_record_with_kind() {
        let snapshots = vec![snapshot(Some(800), Some(40), Some(840))];
        let records = build_usage_records(&snapshots, &ctx("compaction"));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, "compaction");
        assert_eq!(records[0].prompt_tokens, 800);
        assert_eq!(records[0].completion_tokens, 40);
    }

    #[test]
    fn record_carries_all_snapshot_fields() {
        let snapshot = UsageSnapshot {
            prompt_tokens: Some(100),
            completion_tokens: Some(20),
            total_tokens: Some(130),
            cache_read_tokens: Some(60),
            cache_write_tokens: Some(10),
            reasoning_tokens: Some(5),
            estimated: false,
            context_window_tokens: Some(128_000),
        };
        let records = build_usage_records(&[snapshot], &ctx("chat"));
        let r = &records[0];

        assert_eq!(r.conversation_id, "c1");
        assert_eq!(r.message_id.as_deref(), Some("m1"));
        assert_eq!(r.branch_leaf_id.as_deref(), Some("b1"));
        assert_eq!(r.provider_id.as_deref(), Some("p1"));
        assert_eq!(r.model_id.as_deref(), Some("gpt-4"));
        assert_eq!(r.cache_read_tokens, Some(60));
        assert_eq!(r.cache_write_tokens, Some(10));
        assert_eq!(r.reasoning_tokens, Some(5));
        assert_eq!(r.context_window_tokens, Some(128_000));
        assert!(r.cache_capable);
        assert!(!r.estimated);
    }
}
