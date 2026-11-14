//! L1 共用的流替身：按节奏产出的 chunk。
//!
//! 与真实等待无关——推进只改注入时钟的读数，`poll_next` 立即 `Ready`，
//! 因此超时 / TTFT 用例不依赖机器快慢（I36b DT3 / DT8）。
//! 时钟本身复用 `ports::test_doubles::{FixedClock, ManualClock}`，避免第二份实现。

#![allow(dead_code)]

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use assistant_agent::kernel::ports::test_doubles::ManualClock;
use assistant_providers::base::{ChatDelta, ProviderError};
use futures::stream::Stream;

/// 每产出一条 delta 前推进 `gap`：模拟「chunk 连续到达，但累计耗时增长」。
pub struct PacedStream {
    clock: Arc<ManualClock>,
    gap: Duration,
    chunks: VecDeque<ChatDelta>,
}

impl PacedStream {
    pub fn new(clock: Arc<ManualClock>, gap: Duration, chunks: Vec<ChatDelta>) -> Self {
        Self {
            clock,
            gap,
            chunks: chunks.into(),
        }
    }
}

impl Stream for PacedStream {
    type Item = Result<ChatDelta, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.clock.advance(self.gap);
        Poll::Ready(self.chunks.pop_front().map(Ok))
    }
}
