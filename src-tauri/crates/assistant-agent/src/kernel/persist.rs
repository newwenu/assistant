use std::sync::Arc;

use assistant_storage::token_usage::TokenUsageRecord;

use super::ids::TurnId;
use super::outcome::{TurnOutcome, TurnStatus};
use super::ports::{MessageStatus, MessageStore, StoreError, UsageRecord};
use super::stream::{PartialOutcome, UsageSnapshot};
use super::tasks::TurnTasks;

/// 落库上下文：一条助手消息 + 一次 turn 的固定字段。
pub struct PersistCtx {
    pub conversation_id: String,
    pub turn_id: TurnId,
    pub message_id: String,
    pub branch_leaf_id: Option<String>,
    pub kind: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub cache_capable: bool,
    /// usage 记录的落库时间戳（Unix 毫秒），由 runner 从注入 `Clock` 取得，保证可复现。
    pub created_at_ms: i64,
}

impl PersistCtx {
    pub fn usage_records(&self, snapshots: &[UsageSnapshot]) -> Vec<UsageRecord> {
        build_usage_records(snapshots, self)
    }
}

/// 与宿主 `cache_capable_of` 等价，内核按 provider 种类判定缓存能力。
pub fn cache_capable_of(provider_row: &assistant_storage::providers::ProviderRow) -> bool {
    assistant_providers::base::provider_kind_from_str(
        &provider_row.type_,
        provider_row.base_url.clone(),
    )
    .cache_capability()
    .is_capable()
}

/// 每次带 usage 的 attempt 各落一条记录，重试不覆盖。
pub fn build_usage_records(snapshots: &[UsageSnapshot], ctx: &PersistCtx) -> Vec<UsageRecord> {
    let now = ctx.created_at_ms;
    snapshots
        .iter()
        .map(|usage| {
            let (prompt_tokens, completion_tokens, estimated) = usage.record_tokens();
            TokenUsageRecord {
                conversation_id: ctx.conversation_id.clone(),
                message_id: Some(ctx.message_id.clone()),
                attempt_id: Some(ctx.turn_id.clone()),
                branch_leaf_id: ctx.branch_leaf_id.clone(),
                kind: ctx.kind.clone(),
                provider_id: ctx.provider_id.clone(),
                model_id: ctx.model_id.clone(),
                prompt_tokens,
                completion_tokens,
                cache_read_tokens: usage.cache_read_tokens,
                cache_write_tokens: usage.cache_write_tokens,
                reasoning_tokens: usage.reasoning_tokens,
                cache_capable: ctx.cache_capable,
                context_window_tokens: usage.context_window_tokens,
                estimated,
                created_at: now,
            }
        })
        .collect()
}

pub fn message_status_of(status: TurnStatus) -> MessageStatus {
    match status {
        TurnStatus::Completed => MessageStatus::Completed,
        TurnStatus::Cancelled => MessageStatus::Interrupted,
        TurnStatus::Interrupted => MessageStatus::Interrupted,
        TurnStatus::Failed => MessageStatus::Failed,
    }
}

/// 终态与部分结果的写入者。所有写侧都做代际校验（spec §2.2 第二写侧）。
pub struct TurnPersister {
    store: Arc<dyn MessageStore>,
    tasks: Arc<TurnTasks>,
    ctx: PersistCtx,
    /// `persist_partial` 已写过 usage；终态不再重复写，避免同一次 attempt 落两条记录。
    usage_recorded: std::sync::atomic::AtomicBool,
}

impl TurnPersister {
    pub fn new(store: Arc<dyn MessageStore>, tasks: Arc<TurnTasks>, ctx: PersistCtx) -> Self {
        Self {
            store,
            tasks,
            ctx,
            usage_recorded: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn ctx(&self) -> &PersistCtx {
        &self.ctx
    }

    /// 输入就绪后补齐 usage 记录所需的来源信息（分支 / provider / model / 缓存能力）。
    /// 缓存能力由内核按 provider 行自行判定，宿主不再重复上报同一事实。
    pub fn set_source(
        &mut self,
        provider_row: &assistant_storage::providers::ProviderRow,
        branch_leaf_id: Option<String>,
        model_id: Option<String>,
    ) {
        self.ctx.branch_leaf_id = branch_leaf_id;
        self.ctx.provider_id = Some(provider_row.id.clone());
        self.ctx.model_id = model_id;
        self.ctx.cache_capable = cache_capable_of(provider_row);
    }

    fn is_current(&self) -> bool {
        self.tasks
            .is_current(&self.ctx.conversation_id, &self.ctx.turn_id)
    }

    /// 取消 / 超时 / provider 错误后保存已累积内容与 usage。
    pub async fn persist_partial(&self, partial: &PartialOutcome) -> Result<(), StoreError> {
        if !self.is_current() {
            log::warn!(
                "persist_partial skipped: turn={} is not the current generation",
                self.ctx.turn_id
            );
            return Ok(());
        }

        let tokens = partial.last_usage().total_tokens;
        let reasoning =
            (!partial.full_reasoning.is_empty()).then_some(partial.full_reasoning.as_str());
        self.store
            .save_content(
                &self.ctx.message_id,
                &partial.full_content,
                tokens,
                reasoning,
            )
            .await?;
        self.store
            .record_usage(&self.ctx.usage_records(&partial.attempt_usages))
            .await?;
        self.usage_recorded
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// 终态：先写内容与 usage，再写 `messages.status`。
    ///
    /// `messages.tokens` 只接受 provider 上报值，无 usage 时保持 `NULL`。
    pub async fn persist_terminal(&self, outcome: &TurnOutcome) -> Result<(), StoreError> {
        if !self.is_current() {
            log::warn!(
                "persist_terminal skipped: turn={} is not the current generation",
                self.ctx.turn_id
            );
            return Ok(());
        }

        let reasoning = (!outcome.reasoning.is_empty()).then_some(outcome.reasoning.as_str());
        let mut first_error: Option<StoreError> = None;

        if let Err(err) = self
            .store
            .save_content(
                &self.ctx.message_id,
                &outcome.text,
                outcome.total_tokens,
                reasoning,
            )
            .await
        {
            log::error!("save terminal content failed: {err}");
            first_error = Some(err);
        }

        if self
            .usage_recorded
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            log::debug!(
                "terminal usage skipped: turn={} already recorded by partial persist",
                self.ctx.turn_id
            );
        } else if let Err(err) = self
            .store
            .record_usage(&self.ctx.usage_records(&outcome.attempt_usages))
            .await
        {
            log::error!("record terminal usage failed: {err}");
            first_error = first_error.or(Some(err));
        }

        // 内容写失败也要写终态，否则消息会永远停在 `streaming`（验收 4）。
        let status = match &first_error {
            Some(_) => MessageStatus::Failed,
            None => message_status_of(outcome.status),
        };
        if let Err(err) = self.store.finalize(&self.ctx.message_id, status).await {
            log::error!("finalize terminal status failed: {err}");
            first_error = first_error.or(Some(err));
        }

        match first_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }
}
