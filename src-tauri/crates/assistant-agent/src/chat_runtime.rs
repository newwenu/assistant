use std::time::Duration;

use assistant_providers::base::{ChatDelta, ProviderError, TokenUsage, ToolCall};
use futures::Stream;
use log::info;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

/// 单次请求（attempt）的完整 token 快照。
///
/// 未知一律用 `None` 表达，不用 `0`；`estimated` 表示 `total_tokens` 是本地推导值
/// 而非 provider 上报值（`total < P + C` 或 provider 未上报 total）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageSnapshot {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub cache_write_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub estimated: bool,
    pub context_window_tokens: Option<u32>,
}

impl UsageSnapshot {
    pub fn is_empty(&self) -> bool {
        self.prompt_tokens.is_none()
            && self.completion_tokens.is_none()
            && self.total_tokens.is_none()
            && self.cache_read_tokens.is_none()
            && self.cache_write_tokens.is_none()
            && self.reasoning_tokens.is_none()
    }

    /// 合并一次 provider usage 上报：上报值优先，`total` 冲突时按 §4 / M12 裁决。
    fn apply(&mut self, usage: &TokenUsage) {
        if usage.prompt_tokens > 0 {
            self.prompt_tokens = Some(usage.prompt_tokens);
        }
        if usage.completion_tokens > 0 {
            self.completion_tokens = Some(usage.completion_tokens);
        }
        if usage.cache_read_tokens.is_some() {
            self.cache_read_tokens = usage.cache_read_tokens;
        }
        if usage.cache_write_tokens.is_some() {
            self.cache_write_tokens = usage.cache_write_tokens;
        }
        if usage.reasoning_tokens.is_some() {
            self.reasoning_tokens = usage.reasoning_tokens;
        }
        if usage.context_window_tokens.is_some() {
            self.context_window_tokens = usage.context_window_tokens;
        }

        let p_plus_c = usage.prompt_tokens.saturating_add(usage.completion_tokens);
        if usage.total_tokens > 0 && usage.total_tokens >= p_plus_c {
            self.total_tokens = Some(usage.total_tokens);
            self.estimated = false;
        } else if p_plus_c > 0 {
            self.total_tokens = Some(p_plus_c);
            self.estimated = true;
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamOutcome {
    pub full_content: String,
    pub full_reasoning: String,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub cache_write_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub estimated: bool,
    pub context_window_tokens: Option<u32>,
    /// 每个 attempt 一条快照（重试时各产一条，不覆盖）；最后一条即最终 attempt。
    pub attempt_usages: Vec<UsageSnapshot>,
    pub finish_reason: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub has_effective_output: bool,
    pub partial_recovery_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PartialOutcome {
    pub full_content: String,
    pub full_reasoning: String,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub cache_write_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub estimated: bool,
    pub context_window_tokens: Option<u32>,
    pub attempt_usages: Vec<UsageSnapshot>,
    pub has_effective_output: bool,
}

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("{0}")]
    Provider(ProviderError, PartialOutcome),
    #[error("Cancelled")]
    Cancelled(PartialOutcome),
    #[error("Timeout")]
    Timeout(PartialOutcome),
}

#[derive(Debug, Clone)]
pub struct ApplyDeltaOutcome {
    pub finalized_previous: Option<ToolCall>,
    pub params_partial: Option<ToolCallParamsChunk>,
}

#[derive(Debug, Clone)]
pub struct ToolCallParamsChunk {
    pub index: usize,
    pub chunk: String,
}

#[derive(Debug, Clone)]
pub struct PartialToolCall {
    pub id: String,
    pub name: String,
    pub arguments_buf: String,
}

#[derive(Debug, Clone)]
pub struct ToolCallAccumulator {
    pending: Vec<PartialToolCall>,
}

#[derive(Debug, Clone)]
pub struct ToolCallDeltaOutcome {
    pub finalized_previous: Option<ToolCall>,
    pub params_partial: Option<ToolCallParamsChunk>,
}

#[derive(Error, Debug)]
pub enum ToolCallAccumulateError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
    #[error("Truncated tool call (max_tokens)")]
    Truncated,
}

pub struct StreamAccumulator {
    full_content: String,
    full_reasoning: String,
    current_usage: UsageSnapshot,
    attempt_usages: Vec<UsageSnapshot>,
    finish_reason: Option<String>,
    tool_calls: ToolCallAccumulator,
    has_effective_output: bool,
}

enum TimedStreamItem<T> {
    Item(T),
    End,
    TimedOut,
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self {
            full_content: String::new(),
            full_reasoning: String::new(),
            current_usage: UsageSnapshot::default(),
            attempt_usages: Vec::new(),
            finish_reason: None,
            tool_calls: ToolCallAccumulator::new(),
            has_effective_output: false,
        }
    }

    /// 一次 attempt 结束（重试 / 流终止）时把当前快照归档，后续上报归入新 attempt。
    fn flush_attempt(&mut self) {
        if self.current_usage.is_empty() {
            return;
        }
        self.attempt_usages
            .push(std::mem::take(&mut self.current_usage));
    }

    pub fn apply_delta(&mut self, delta: &ChatDelta) -> ApplyDeltaOutcome {
        if delta.delta_retry.is_some() {
            self.flush_attempt();
        }

        if let Some(text) = &delta.delta_text {
            if !text.is_empty() {
                self.full_content.push_str(text);
                self.has_effective_output = true;
            }
        }

        if let Some(reasoning) = &delta.delta_reasoning {
            if !reasoning.is_empty() {
                self.full_reasoning.push_str(reasoning);
            }
        }

        if let Some(ref usage) = delta.usage {
            self.current_usage.apply(usage);
        }

        if delta.finish_reason.is_some() {
            self.finish_reason = delta.finish_reason.clone();
        }

        let tc_outcome = self.tool_calls.apply_delta(delta);

        if tc_outcome.finalized_previous.is_some()
            || tc_outcome.params_partial.is_some()
            || delta
                .delta_tool_call_id
                .as_deref()
                .is_some_and(|id| !id.is_empty())
        {
            self.has_effective_output = true;
        }

        ApplyDeltaOutcome {
            finalized_previous: tc_outcome.finalized_previous,
            params_partial: tc_outcome.params_partial,
        }
    }

    pub fn into_outcome(mut self) -> StreamOutcome {
        self.flush_attempt();
        let usage = self.attempt_usages.last().cloned().unwrap_or_default();

        let tool_calls_result = self.tool_calls.finish();
        let tool_calls = match tool_calls_result {
            Ok(calls) => {
                if calls.is_empty() {
                    None
                } else {
                    Some(calls)
                }
            }
            Err(ToolCallAccumulateError::Truncated) => None,
            Err(ToolCallAccumulateError::InvalidJson(_)) => None,
        };

        StreamOutcome {
            full_content: self.full_content,
            full_reasoning: self.full_reasoning,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            reasoning_tokens: usage.reasoning_tokens,
            total_tokens: usage.total_tokens,
            estimated: usage.estimated,
            context_window_tokens: usage.context_window_tokens,
            attempt_usages: self.attempt_usages,
            finish_reason: self.finish_reason,
            tool_calls,
            has_effective_output: self.has_effective_output,
            partial_recovery_reason: None,
        }
    }

    pub fn into_partial(mut self) -> PartialOutcome {
        self.flush_attempt();
        let usage = self.attempt_usages.last().cloned().unwrap_or_default();

        PartialOutcome {
            full_content: self.full_content,
            full_reasoning: self.full_reasoning,
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            reasoning_tokens: usage.reasoning_tokens,
            total_tokens: usage.total_tokens,
            estimated: usage.estimated,
            context_window_tokens: usage.context_window_tokens,
            attempt_usages: self.attempt_usages,
            has_effective_output: self.has_effective_output,
        }
    }

    pub fn has_effective_output(&self) -> bool {
        self.has_effective_output
    }

    pub fn full_content(&self) -> &str {
        &self.full_content
    }

    pub fn total_tokens(&self) -> Option<u32> {
        self.current_usage.total_tokens
    }

    pub fn prompt_tokens(&self) -> Option<u32> {
        self.current_usage.prompt_tokens
    }
}

impl Default for StreamAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ToolCallAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallAccumulator {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    pub fn apply_delta(&mut self, delta: &ChatDelta) -> ToolCallDeltaOutcome {
        let mut finalized_previous = None;
        let mut params_partial = None;

        if let Some(id) = &delta.delta_tool_call_id {
            if !id.is_empty() {
                if !self.pending.is_empty() {
                    let prev = self.pending.pop().unwrap();
                    finalized_previous = Some(ToolCall {
                        id: prev.id,
                        name: prev.name,
                        arguments: prev.arguments_buf,
                    });
                }
                self.pending.push(PartialToolCall {
                    id: id.clone(),
                    name: String::new(),
                    arguments_buf: String::new(),
                });
            }
        }

        if !self.pending.is_empty() {
            let last_idx = self.pending.len() - 1;
            let last = self.pending.last_mut().unwrap();
            if let Some(name) = &delta.delta_tool_call_name {
                last.name.push_str(name);
                self_mark_effective(&mut params_partial, last_idx, name);
            }
            if let Some(args) = &delta.delta_tool_call_arguments {
                last.arguments_buf.push_str(args);
                params_partial = Some(ToolCallParamsChunk {
                    index: last_idx,
                    chunk: args.clone(),
                });
            }
        }

        ToolCallDeltaOutcome {
            finalized_previous,
            params_partial,
        }
    }

    pub fn finish(self) -> Result<Vec<ToolCall>, ToolCallAccumulateError> {
        let mut result = Vec::with_capacity(self.pending.len());
        for partial in self.pending {
            let _validated: serde_json::Value = serde_json::from_str(&partial.arguments_buf)
                .map_err(|e| {
                    ToolCallAccumulateError::InvalidJson(format!(
                        "tool call '{}' arguments: {}",
                        partial.name, e
                    ))
                })?;
            result.push(ToolCall {
                id: partial.id,
                name: partial.name,
                arguments: partial.arguments_buf,
            });
        }
        Ok(result)
    }

    pub fn finish_with_truncation(
        self,
        was_max_tokens: bool,
    ) -> Result<Vec<ToolCall>, ToolCallAccumulateError> {
        if !was_max_tokens {
            return self.finish();
        }

        let total = self.pending.len();
        let mut result = Vec::new();
        for partial in self.pending {
            if serde_json::from_str::<serde_json::Value>(&partial.arguments_buf).is_ok() {
                result.push(ToolCall {
                    id: partial.id,
                    name: partial.name,
                    arguments: partial.arguments_buf,
                });
            }
        }
        if result.len() < total {
            return Err(ToolCallAccumulateError::Truncated);
        }
        Ok(result)
    }
}

fn self_mark_effective(
    params_partial: &mut Option<ToolCallParamsChunk>,
    index: usize,
    chunk: &str,
) {
    if params_partial.is_none() && !chunk.is_empty() {
        *params_partial = Some(ToolCallParamsChunk {
            index,
            chunk: chunk.to_string(),
        });
    }
}

pub async fn consume_stream<S, F>(
    stream: S,
    cancel_token: CancellationToken,
    on_delta: F,
    timeout: Option<Duration>,
) -> Result<StreamOutcome, StreamError>
where
    S: Stream<Item = Result<ChatDelta, ProviderError>> + Unpin,
    F: Fn(u32, &ChatDelta),
{
    let mut stream = stream;
    let mut accum = StreamAccumulator::new();
    let mut seq: u32 = 0;

    loop {
        let item = next_item(&mut stream, timeout).await;

        if cancel_token.is_cancelled() {
            return Err(StreamError::Cancelled(accum.into_partial()));
        }

        match item {
            TimedStreamItem::Item(Ok(delta)) => {
                let _outcome = accum.apply_delta(&delta);
                seq += 1;
                on_delta(seq, &delta);
                if delta.finish_reason.is_some() {
                    return Ok(accum.into_outcome());
                }
            }
            TimedStreamItem::Item(Err(e)) => {
                return Err(StreamError::Provider(e, accum.into_partial()));
            }
            TimedStreamItem::End => {
                return Ok(accum.into_outcome());
            }
            TimedStreamItem::TimedOut => {
                return Err(StreamError::Timeout(accum.into_partial()));
            }
        }
    }
}

async fn next_item<S>(stream: &mut S, timeout: Option<Duration>) -> TimedStreamItem<S::Item>
where
    S: Stream + Unpin,
{
    use futures::StreamExt;
    match timeout {
        Some(d) => match tokio::time::timeout(d, stream.next()).await {
            Ok(Some(item)) => TimedStreamItem::Item(item),
            Ok(None) => TimedStreamItem::End,
            Err(_) => TimedStreamItem::TimedOut,
        },
        None => match stream.next().await {
            Some(item) => TimedStreamItem::Item(item),
            None => TimedStreamItem::End,
        },
    }
}

pub use crate::token_estimate::estimate_text_tokens;

/// 流结束后解析 `messages.tokens`：只接受 provider 上报的 `total_tokens`。
///
/// 契约 §7-7 要求无 usage 时该列为 `NULL` 而非估算值；估算只用于占用兜底，
/// 不得冒充账单口径写入消息行。
pub fn resolve_final_tokens(outcome: &StreamOutcome) -> Option<i64> {
    info!(
        "[chat-delta] stream finished: reason={:?} total_tokens={:?}",
        outcome.finish_reason, outcome.total_tokens
    );
    outcome.total_tokens.map(|t| t as i64)
}

/// 流中断时解析 `messages.tokens`：同样只接受 provider 上报值，无 usage 时为 `NULL`。
pub fn resolve_partial_tokens(partial: &PartialOutcome) -> Option<i64> {
    partial.total_tokens.map(|t| t as i64)
}
