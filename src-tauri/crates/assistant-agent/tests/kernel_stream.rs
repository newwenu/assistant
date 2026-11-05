mod common;

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use assistant_agent::kernel::stream::{consume as consume_with_clock, StreamError, StreamOutcome};
use assistant_agent::kernel::Clock;
use assistant_providers::base::{ChatDelta, ProviderError, TokenUsage};
use common::{FixedClock, ManualClock, PacedStream};
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
    consume_with_clock(stream, token, timeout, &clock, clock.base(), on_delta).await
}

fn text_delta(text: &str) -> ChatDelta {
    ChatDelta {
        delta_text: Some(text.to_string()),
        ..Default::default()
    }
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

#[tokio::test(start_paused = true)]
async fn cancelled_stream_keeps_accumulated_content() {
    let token = CancellationToken::new();
    let cancel_handle = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancel_handle.cancel();
    });

    let result = consume(
        StallingStream::new(vec![text_delta("partial")]),
        token,
        None,
        |_seq, _delta| {},
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
async fn tool_call_fragments_accumulate_into_single_call() {
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

    let outcome = consume(
        stream::iter(deltas),
        CancellationToken::new(),
        None,
        |_seq, _delta| {},
    )
    .await
    .expect("stream completes");

    assert_eq!(outcome.tool_calls.len(), 1);
    assert_eq!(outcome.tool_calls[0].id, "call_1");
    assert_eq!(outcome.tool_calls[0].name, "read");
    assert_eq!(outcome.tool_calls[0].arguments, "{\"path\":\"a.txt\"}");
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
    let clock = Arc::new(ManualClock::default());
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
