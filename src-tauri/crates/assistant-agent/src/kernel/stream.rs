use std::time::{Duration, Instant};

use assistant_providers::base::{ChatDelta, ProviderError, TokenUsage};
use futures::Stream;
use tokio_util::sync::CancellationToken;

use super::ports::Clock;

/// 单次模型调用的 token 快照；未知一律 `None`，`estimated` 表示 total 是本地推导值。
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

    /// 落库用的 `(prompt, completion, estimated)`；prompt 缺失时按 `total - completion` 回退并标记估算。
    pub fn record_tokens(&self) -> (u32, u32, bool) {
        let completion = self.completion_tokens.unwrap_or(0);
        match self.prompt_tokens {
            Some(prompt) => (prompt, completion, self.estimated),
            None => {
                let derived = self
                    .total_tokens
                    .map(|total| total.saturating_sub(completion))
                    .unwrap_or(0);
                (derived, completion, true)
            }
        }
    }

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

#[derive(Debug, Clone, Default)]
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
    /// 每个 attempt 一条快照（重试各产一条，不覆盖）；最后一条即最终 attempt。
    pub attempt_usages: Vec<UsageSnapshot>,
    /// 首个 delta 距请求发起的时间（毫秒）；全程无 delta 时为 `None`。
    pub ttft_ms: Option<u64>,
    pub finish_reason: Option<String>,
    pub has_effective_output: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PartialOutcome {
    pub full_content: String,
    pub full_reasoning: String,
    pub attempt_usages: Vec<UsageSnapshot>,
    pub ttft_ms: Option<u64>,
    pub has_effective_output: bool,
}

impl PartialOutcome {
    pub fn last_usage(&self) -> UsageSnapshot {
        self.attempt_usages.last().cloned().unwrap_or_default()
    }
}

impl StreamOutcome {
    pub fn last_usage(&self) -> UsageSnapshot {
        self.attempt_usages.last().cloned().unwrap_or_default()
    }
}

/// 输出上限截断判定（D22）：截断不是「模型没说话」，同预算重发必然再次截断。
///
/// 取值覆盖三家 adapter 的 `finish_reason` 原生拼写：
/// OpenAI `length`、Anthropic `max_tokens`、Gemini `MAX_TOKENS`、Responses `max_output_tokens`。
pub fn is_output_limit_reached(finish_reason: Option<&str>) -> bool {
    matches!(
        finish_reason,
        Some("length" | "max_tokens" | "MAX_TOKENS" | "max_output_tokens")
    )
}

#[derive(Debug)]
pub enum StreamError {
    Cancelled(PartialOutcome),
    Timeout(PartialOutcome),
    Provider(ProviderError, PartialOutcome),
}

#[derive(Debug, Clone, Default)]
pub struct StreamAccumulator {
    full_content: String,
    full_reasoning: String,
    current_usage: UsageSnapshot,
    attempt_usages: Vec<UsageSnapshot>,
    ttft_ms: Option<u64>,
    finish_reason: Option<String>,
    has_effective_output: bool,
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    fn flush_attempt(&mut self) {
        if self.current_usage.is_empty() {
            return;
        }
        self.attempt_usages
            .push(std::mem::take(&mut self.current_usage));
    }

    /// 只记首个 delta 的耗时；重试 attempt 不会覆盖它。
    fn record_ttft(&mut self, clock: &dyn Clock, request_started: Instant) {
        if self.ttft_ms.is_none() {
            self.ttft_ms = Some(clock.now().duration_since(request_started).as_millis() as u64);
        }
    }

    pub fn apply_delta(&mut self, delta: &ChatDelta) {
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
    }

    pub fn into_outcome(mut self) -> StreamOutcome {
        self.flush_attempt();
        let usage = self.attempt_usages.last().cloned().unwrap_or_default();

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
            ttft_ms: self.ttft_ms,
            finish_reason: self.finish_reason,
            has_effective_output: self.has_effective_output,
        }
    }

    pub fn into_partial(mut self) -> PartialOutcome {
        self.flush_attempt();
        PartialOutcome {
            full_content: self.full_content,
            full_reasoning: self.full_reasoning,
            attempt_usages: self.attempt_usages,
            ttft_ms: self.ttft_ms,
            has_effective_output: self.has_effective_output,
        }
    }

    pub fn has_effective_output(&self) -> bool {
        self.has_effective_output
    }

    pub fn full_content(&self) -> &str {
        &self.full_content
    }
}

enum TimedStreamItem<T> {
    Item(T),
    End,
    TimedOut,
}

/// 消费 provider 流：累积正文 / 推理 / 工具片段 / usage，`seq` 从 0 起每次回调前 +1（首条为 1）。
///
/// 不做重试、不落库、不发事件、不判空响应——这些都在 `TurnRunner` 与其协作模块。
///
/// `timeout` 是**整段封顶**预算（H6）：每次等待 chunk 的超时额度 = 剩余预算，
/// 故单次调用总耗时不超过该值，持续有 chunk 到达也不例外。
pub async fn consume<S, F>(
    stream: S,
    token: CancellationToken,
    timeout: Option<Duration>,
    clock: &dyn Clock,
    request_started: Instant,
    mut on_delta: F,
) -> Result<StreamOutcome, StreamError>
where
    S: Stream<Item = Result<ChatDelta, ProviderError>> + Unpin,
    F: FnMut(u32, &ChatDelta) + Send,
{
    let mut stream = stream;
    let mut accum = StreamAccumulator::new();
    let mut seq: u32 = 0;
    // 封顶预算从本次调用入口起算，避免把请求构建耗时重复计入。
    let call_started = clock.now();

    loop {
        let budget = match timeout {
            Some(total) => {
                let remaining = total.saturating_sub(clock.now().duration_since(call_started));
                if remaining.is_zero() {
                    return Err(StreamError::Timeout(accum.into_partial()));
                }
                Some(remaining)
            }
            None => None,
        };

        let item = next_item(&mut stream, &token, budget).await;

        if token.is_cancelled() {
            return Err(StreamError::Cancelled(accum.into_partial()));
        }

        match item {
            TimedStreamItem::Item(Ok(delta)) => {
                accum.record_ttft(clock, request_started);
                accum.apply_delta(&delta);
                seq += 1;
                on_delta(seq, &delta);
                if delta.finish_reason.is_some() {
                    return Ok(accum.into_outcome());
                }
            }
            TimedStreamItem::Item(Err(error)) => {
                return Err(StreamError::Provider(error, accum.into_partial()));
            }
            TimedStreamItem::End => return Ok(accum.into_outcome()),
            TimedStreamItem::TimedOut => {
                return Err(StreamError::Timeout(accum.into_partial()));
            }
        }
    }
}

async fn next_item<S>(
    stream: &mut S,
    token: &CancellationToken,
    budget: Option<Duration>,
) -> TimedStreamItem<S::Item>
where
    S: Stream + Unpin,
{
    use futures::StreamExt;

    let next = async {
        match budget {
            Some(duration) => match tokio::time::timeout(duration, stream.next()).await {
                Ok(Some(item)) => TimedStreamItem::Item(item),
                Ok(None) => TimedStreamItem::End,
                Err(_) => TimedStreamItem::TimedOut,
            },
            None => match stream.next().await {
                Some(item) => TimedStreamItem::Item(item),
                None => TimedStreamItem::End,
            },
        }
    };

    tokio::select! {
        biased;
        _ = token.cancelled() => TimedStreamItem::TimedOut,
        item = next => item,
    }
}
