#![cfg(feature = "test-doubles")]

mod common;

use std::sync::Arc;
use std::time::Duration;

use assistant_agent::kernel::ports::test_doubles::ManualClock;
use assistant_agent::kernel::stream::consume;
use assistant_agent::kernel::Clock;
use assistant_providers::base::{ChatDelta, ProviderError};
use common::chat::delta_text as text_delta;
use common::stream::PacedStream;
use futures::stream;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn ttft_is_none_when_stream_emits_no_delta() {
    let clock = ManualClock::starting_now();
    let request_started = clock.now();

    let outcome = consume(
        stream::iter(Vec::<Result<ChatDelta, ProviderError>>::new()),
        CancellationToken::new(),
        None,
        &clock,
        request_started,
        |_seq, _delta| {},
    )
    .await
    .expect("empty stream completes");

    assert_eq!(outcome.ttft_ms, None, "无 delta 时 TTFT 必须为 None");
}

#[tokio::test]
async fn ttft_records_first_delta_only() {
    let clock = Arc::new(ManualClock::starting_now());
    let request_started = clock.now();
    let chunks = vec![text_delta("a"), text_delta("b"), ChatDelta::finish("stop")];

    let outcome = consume(
        PacedStream::new(clock.clone(), Duration::from_millis(25), chunks),
        CancellationToken::new(),
        None,
        clock.as_ref(),
        request_started,
        |_seq, _delta| {},
    )
    .await
    .expect("stream completes");

    assert_eq!(
        outcome.ttft_ms,
        Some(25),
        "TTFT 只取首个 delta，后续 delta 不覆盖"
    );
}
