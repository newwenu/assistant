#![cfg(feature = "test-doubles")]

mod common;

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use assistant_agent::kernel::ports::test_doubles::{FixedClock, ManualClock};
use assistant_agent::kernel::stream::{
    consume as consume_with_clock, is_output_limit_reached, StreamAccumulator, StreamError,
    StreamOutcome,
};
use assistant_agent::kernel::Clock;
use assistant_providers::base::{ChatDelta, ProviderError, TokenUsage};
use common::chat::delta_text as text_delta;
use common::stream::PacedStream;
use futures::stream::{self, Stream};
use tokio_util::sync::CancellationToken;

/// 测试包装：显式注入固定时钟，调用点只关心流与回调。
async fn consume<S, F>(
    stream: S,
    token: CancellationToken,
    timeout: Option<Duration>,
    on_delta: F,
) -> Result<StreamOutcome, StreamError>
where
    S: Stream<Item = Result<ChatDelta, ProviderError>> + Unpin,
    F: FnMut(u32, &ChatDelta) + Send,
{
    let clock = FixedClock::default();
    let request_started = clock.now();
    consume_with_clock(stream, token, timeout, &clock, request_started, on_delta).await
}

fn reasoning_delta(text: &str) -> ChatDelta {
    ChatDelta {
        delta_reasoning: Some(text.to_string()),
        ..Default::default()
    }
}

fn usage_delta(prompt: u32, completion: u32, total: u32) -> ChatDelta {
    ChatDelta {
        usage: Some(TokenUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
            cache_read_tokens: Some(0),
            cache_write_tokens: None,
            reasoning_tokens: None,
            context_window_tokens: None,
        }),
        ..Default::default()
    }
}

/// 先吐出预设片段，然后永远挂起（用于超时 / 取消分支）。
struct StallingStream {
    pending: Vec<ChatDelta>,
    index: usize,
}

impl StallingStream {
    fn new(pending: Vec<ChatDelta>) -> Self {
        Self { pending, index: 0 }
    }
}

impl Stream for StallingStream {
    type Item = Result<ChatDelta, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.index < self.pending.len() {
            let delta = self.pending[self.index].clone();
            self.index += 1;
            Poll::Ready(Some(Ok(delta)))
        } else {
            Poll::Pending
        }
    }
}

#[tokio::test]
async fn seq_starts_at_one_and_increases_monotonically() {
    let deltas = vec![
        Ok(text_delta("a")),
        Ok(reasoning_delta("think")),
        Ok(text_delta("b")),
        Ok(ChatDelta::finish("stop")),
    ];
    let seen: Arc<Mutex<Vec<u32>>> = Arc::new(Mutex::new(Vec::new()));
    let seen_clone = Arc::clone(&seen);

    let outcome = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        None,
        move |seq, _delta| seen_clone.lock().unwrap().push(seq),
    )
    .await
    .expect("stream completes");

    assert_eq!(outcome.full_content, "ab");
    assert_eq!(outcome.full_reasoning, "think");
    assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
    assert!(outcome.has_effective_output);
    assert_eq!(*seen.lock().unwrap(), vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn finish_reason_terminates_stream_before_end() {
    let deltas = vec![Ok(text_delta("hi")), Ok(ChatDelta::finish("stop"))];

    let outcome = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        None,
        |_seq, _delta| {},
    )
    .await
    .expect("stream completes");

    assert_eq!(outcome.full_content, "hi");
}

#[tokio::test]
async fn cancelled_stream_keeps_accumulated_content() {
    let token = CancellationToken::new();
    let cancel_handle = token.clone();

    let result = consume(
        StallingStream::new(vec![text_delta("partial")]),
        token,
        None,
        move |_seq, _delta| cancel_handle.cancel(),
    )
    .await;

    match result {
        Err(StreamError::Cancelled(partial)) => {
            assert_eq!(partial.full_content, "partial");
            assert!(partial.has_effective_output);
        }
        other => panic!(
            "expected Cancelled, got {:?}",
            other.map(|o| o.full_content)
        ),
    }
}

#[tokio::test(start_paused = true)]
async fn timeout_keeps_accumulated_content() {
    let result = consume(
        StallingStream::new(vec![text_delta("before-timeout")]),
        CancellationToken::new(),
        Some(Duration::from_millis(20)),
        |_seq, _delta| {},
    )
    .await;

    match result {
        Err(StreamError::Timeout(partial)) => {
            assert_eq!(partial.full_content, "before-timeout");
        }
        other => panic!("expected Timeout, got {:?}", other.map(|o| o.full_content)),
    }
}

#[tokio::test]
async fn provider_error_returns_partial_outcome() {
    let deltas = vec![
        Ok(text_delta("kept")),
        Err(ProviderError::StreamError("boom".to_string())),
    ];

    let result = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        None,
        |_seq, _delta| {},
    )
    .await;

    match result {
        Err(StreamError::Provider(_, partial)) => {
            assert_eq!(partial.full_content, "kept");
        }
        other => panic!(
            "expected Provider error, got {:?}",
            other.map(|o| o.full_content)
        ),
    }
}

#[tokio::test]
async fn tool_call_fragments_are_forwarded_to_callback_untouched() {
    let deltas = vec![
        Ok(ChatDelta {
            delta_tool_call_id: Some("call_1".to_string()),
            delta_tool_call_name: Some("read".to_string()),
            ..Default::default()
        }),
        Ok(ChatDelta {
            delta_tool_call_arguments: Some("{\"path\":".to_string()),
            ..Default::default()
        }),
        Ok(ChatDelta {
            delta_tool_call_arguments: Some("\"a.txt\"}".to_string()),
            ..Default::default()
        }),
        Ok(ChatDelta::finish("tool_calls")),
    ];

    let mut seen = Vec::new();
    let outcome = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        None,
        |_seq, delta| seen.push(delta.clone()),
    )
    .await
    .expect("stream completes");

    assert_eq!(seen.len(), 4, "raw fragments reach the caller");
    assert_eq!(seen[0].delta_tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(
        seen[1].delta_tool_call_arguments.as_deref(),
        Some("{\"path\":")
    );
    assert_eq!(
        seen[2].delta_tool_call_arguments.as_deref(),
        Some("\"a.txt\"}")
    );
    assert!(!outcome.has_effective_output);
}

#[tokio::test]
async fn retry_notification_splits_attempt_usages() {
    let deltas = vec![
        Ok(text_delta("first")),
        Ok(usage_delta(10, 2, 12)),
        Ok(ChatDelta::retry_notification(1, 3, 100, "retrying")),
        Ok(text_delta("second")),
        Ok(usage_delta(20, 4, 24)),
        Ok(ChatDelta::finish("stop")),
    ];

    let outcome = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        None,
        |_seq, _delta| {},
    )
    .await
    .expect("stream completes");

    assert_eq!(outcome.full_content, "firstsecond");
    assert_eq!(outcome.attempt_usages.len(), 2);
    assert_eq!(outcome.attempt_usages[0].prompt_tokens, Some(10));
    assert_eq!(outcome.attempt_usages[1].prompt_tokens, Some(20));
    assert_eq!(outcome.total_tokens, Some(24));
}

#[tokio::test]
async fn missing_usage_keeps_tokens_none() {
    let deltas = vec![Ok(text_delta("no usage")), Ok(ChatDelta::finish("stop"))];

    let outcome = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        None,
        |_seq, _delta| {},
    )
    .await
    .expect("stream completes");

    assert_eq!(outcome.total_tokens, None);
    assert_eq!(outcome.prompt_tokens, None);
    assert!(outcome.attempt_usages.is_empty());
}

/// H6：`timeout` 是整段封顶，不是 chunk 间隔超时。
///
/// 流每 60ms 稳定产出一个 chunk（永不「卡住」），预算 100ms：
/// 若实现只看单次间隔，永远不会超时；正确实现应在累计 100ms 后失败。
#[tokio::test]
async fn timeout_caps_whole_call_even_when_chunks_keep_arriving() {
    let clock = Arc::new(ManualClock::starting_now());
    let request_started = clock.now();
    let chunks = vec![text_delta("a"), text_delta("b"), text_delta("c")];

    let result = consume_with_clock(
        PacedStream::new(clock.clone(), Duration::from_millis(60), chunks),
        CancellationToken::new(),
        Some(Duration::from_millis(100)),
        clock.as_ref(),
        request_started,
        |_seq, _delta| {},
    )
    .await;

    match result {
        Err(StreamError::Timeout(partial)) => {
            assert_eq!(
                partial.full_content, "ab",
                "封顶预算用尽时应保留已到达的 chunk"
            );
        }
        other => panic!(
            "expected Timeout (整段封顶), got {:?}",
            other.map(|o| o.full_content)
        ),
    }
}

#[tokio::test]
async fn empty_stream_completes_without_output() {
    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![];

    let outcome = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        None,
        |_seq, _delta| {},
    )
    .await
    .expect("stream completes");

    assert_eq!(outcome.full_content, "");
    assert!(!outcome.has_effective_output);
    assert!(outcome.attempt_usages.is_empty());
}

// ─── StreamAccumulator 单测（迁自旧 chat 链路测试 b7） ───

#[test]
fn accumulator_text_only_collects_content_and_usage() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&text_delta("Hello"));
    accum.apply_delta(&text_delta(" world"));
    accum.apply_delta(&usage_delta(100, 50, 150));
    accum.apply_delta(&ChatDelta::finish("stop"));

    let outcome = accum.into_outcome();
    assert_eq!(outcome.full_content, "Hello world");
    assert_eq!(outcome.full_reasoning, "");
    assert_eq!(outcome.total_tokens, Some(150));
    assert_eq!(outcome.prompt_tokens, Some(100));
    assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
    assert!(outcome.has_effective_output);
}

#[test]
fn accumulator_reasoning_is_kept_separate_from_content() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&reasoning_delta("Let me think"));
    accum.apply_delta(&reasoning_delta("..."));
    accum.apply_delta(&text_delta("Answer"));
    accum.apply_delta(&ChatDelta::finish("stop"));

    let outcome = accum.into_outcome();
    assert_eq!(outcome.full_content, "Answer");
    assert_eq!(outcome.full_reasoning, "Let me think...");
    assert!(outcome.has_effective_output);
}

#[test]
fn accumulator_usage_overwrite_keeps_latest() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&usage_delta(100, 20, 120));
    accum.apply_delta(&usage_delta(100, 50, 150));

    let outcome = accum.into_outcome();
    assert_eq!(outcome.total_tokens, Some(150));
    assert_eq!(outcome.prompt_tokens, Some(100));
}

#[test]
fn accumulator_prefers_provider_total_when_consistent() {
    let mut accum = StreamAccumulator::new();
    let delta = ChatDelta {
        usage: Some(TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 20,
            total_tokens: 130,
            ..Default::default()
        }),
        ..Default::default()
    };

    accum.apply_delta(&delta);
    let outcome = accum.into_outcome();

    assert_eq!(outcome.total_tokens, Some(130), "provider total wins");
    assert!(!outcome.estimated);
}

#[test]
fn accumulator_falls_back_when_total_below_sum() {
    let mut accum = StreamAccumulator::new();
    let delta = ChatDelta {
        usage: Some(TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 20,
            total_tokens: 90,
            ..Default::default()
        }),
        ..Default::default()
    };

    accum.apply_delta(&delta);
    let outcome = accum.into_outcome();

    assert_eq!(outcome.total_tokens, Some(120), "P + C wins over bad total");
    assert!(outcome.estimated);
}

#[test]
fn accumulator_retry_splits_attempt_usages() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&usage_delta(100, 10, 110));
    accum.apply_delta(&ChatDelta::retry_notification(1, 3, 0, "retry"));
    accum.apply_delta(&usage_delta(200, 20, 220));

    let outcome = accum.into_outcome();

    assert_eq!(outcome.attempt_usages.len(), 2, "retry keeps both attempts");
    assert_eq!(outcome.attempt_usages[0].prompt_tokens, Some(100));
    assert_eq!(outcome.attempt_usages[1].prompt_tokens, Some(200));
    assert_eq!(
        outcome.prompt_tokens,
        Some(200),
        "latest attempt drives the outcome"
    );
}

#[test]
fn accumulator_empty_delta_has_no_effective_output() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&ChatDelta::default());
    assert!(!accum.has_effective_output());

    accum.apply_delta(&usage_delta(100, 50, 150));
    assert!(!accum.has_effective_output());
}

#[test]
fn output_limit_finish_reasons_are_recognized() {
    for reason in ["length", "max_tokens", "MAX_TOKENS", "max_output_tokens"] {
        assert!(
            is_output_limit_reached(Some(reason)),
            "应识别为输出上限截断: {reason}"
        );
    }
    for reason in ["stop", "tool_calls", "end_turn", "STOP", ""] {
        assert!(
            !is_output_limit_reached(Some(reason)),
            "不得误判为输出上限截断: {reason}"
        );
    }
    assert!(!is_output_limit_reached(None));
}

// ─── 压缩链路（迁自旧 chat 链路测试 g4 / g6 / g7） ───

#[tokio::test]
async fn compaction_stream_forwards_text_and_finish_fragments() {
    let deltas = vec![
        Ok(reasoning_delta("thinking")),
        Ok(ChatDelta {
            delta_tool_call_id: Some("tc_1".to_string()),
            ..Default::default()
        }),
        Ok(ChatDelta {
            delta_tool_call_name: Some("search".to_string()),
            ..Default::default()
        }),
        Ok(ChatDelta {
            delta_tool_call_arguments: Some("{}".to_string()),
            ..Default::default()
        }),
        Ok(text_delta("Summary text")),
        Ok(ChatDelta::finish("stop")),
    ];

    let emitted_text: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let emitted_finish: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let et = emitted_text.clone();
    let ef = emitted_finish.clone();

    let outcome = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        Some(Duration::from_secs(30)),
        move |_seq, delta| {
            if let Some(text) = delta.delta_text.as_ref().filter(|t| !t.is_empty()) {
                et.lock().unwrap().push(text.clone());
            }
            if let Some(reason) = &delta.finish_reason {
                ef.lock().unwrap().push(reason.clone());
            }
        },
    )
    .await
    .expect("stream completes");

    assert_eq!(outcome.full_content, "Summary text");
    assert_eq!(outcome.full_reasoning, "thinking");
    assert_eq!(
        *emitted_text.lock().unwrap(),
        vec!["Summary text".to_string()]
    );
    assert_eq!(*emitted_finish.lock().unwrap(), vec!["stop".to_string()]);
}

#[tokio::test]
async fn compaction_summary_is_outcome_full_content() {
    let deltas = vec![
        Ok(text_delta("The user asked about ")),
        Ok(text_delta("Rust async patterns. ")),
        Ok(text_delta("Key points: tokio::select!")),
        Ok(ChatDelta::finish("stop")),
    ];

    let outcome = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        Some(Duration::from_secs(30)),
        |_seq, _delta| {},
    )
    .await
    .expect("stream completes");

    assert_eq!(
        outcome.full_content,
        "The user asked about Rust async patterns. Key points: tokio::select!"
    );
    assert!(outcome.has_effective_output);
    assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
}

#[tokio::test]
async fn compaction_cancel_returns_partial_summary() {
    let deltas = vec![
        Ok(text_delta("Partial summary")),
        Ok(text_delta(" content")),
    ];
    let token = CancellationToken::new();
    let handle = token.clone();

    let result = consume(
        stream::iter(deltas),
        token,
        Some(Duration::from_secs(30)),
        move |seq, _delta| {
            if seq == 1 {
                handle.cancel();
            }
        },
    )
    .await;

    match result {
        Err(StreamError::Cancelled(partial)) => {
            assert!(partial.has_effective_output);
            assert_eq!(partial.full_content, "Partial summary");
        }
        other => panic!(
            "expected Cancelled with partial, got {:?}",
            other.map(|o| o.full_content)
        ),
    }
}

#[tokio::test(start_paused = true)]
async fn compaction_timeout_returns_partial_summary() {
    let result = consume(
        StallingStream::new(vec![text_delta("Partial")]),
        CancellationToken::new(),
        Some(Duration::from_secs(30)),
        |_seq, _delta| {},
    )
    .await;

    match result {
        Err(StreamError::Timeout(partial)) => {
            assert!(partial.has_effective_output);
            assert_eq!(partial.full_content, "Partial");
        }
        other => panic!(
            "expected Timeout with partial, got {:?}",
            other.map(|o| o.full_content)
        ),
    }
}

#[tokio::test]
async fn compaction_provider_error_returns_partial_summary() {
    let deltas = vec![
        Ok(text_delta("Partial summary before error")),
        Err(ProviderError::StreamError("provider failed".to_string())),
    ];

    let result = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        Some(Duration::from_secs(30)),
        |_seq, _delta| {},
    )
    .await;

    match result {
        Err(StreamError::Provider(_, partial)) => {
            assert!(partial.has_effective_output);
            assert_eq!(partial.full_content, "Partial summary before error");
        }
        other => panic!(
            "expected Provider error with partial, got {:?}",
            other.map(|o| o.full_content)
        ),
    }
}
