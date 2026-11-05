//! L1 共用替身：时钟与按节奏产出的流。
//!
//! 目的（I36b H4 / DT3）：超时与 TTFT 用例由推进虚拟时钟触发，
//! 用例内不出现 `Instant::now()`、不出现 `sleep`，慢机器上也不抖动。

#![allow(dead_code)]

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use assistant_agent::kernel::Clock;
use assistant_providers::base::{ChatDelta, ProviderError};
use futures::stream::Stream;

/// 固定时刻时钟：所有读取返回同一时刻，用于不需要计时的用例。
pub struct FixedClock {
    base: Instant,
}

impl Default for FixedClock {
    fn default() -> Self {
        Self {
            base: Instant::now(),
        }
    }
}

impl FixedClock {
    pub fn base(&self) -> Instant {
        self.base
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Instant {
        self.base
    }

    fn now_millis(&self) -> i64 {
        0
    }
}

/// 可推进时钟：`now()` 不自动前进，只有显式 `advance` 才推进。
pub struct ManualClock {
    base: Instant,
    offset: Mutex<Duration>,
}

impl Default for ManualClock {
    fn default() -> Self {
        Self {
            base: Instant::now(),
            offset: Mutex::new(Duration::ZERO),
        }
    }
}

impl ManualClock {
    pub fn advance(&self, delta: Duration) {
        let mut offset = self.lock_offset();
        *offset += delta;
    }

    fn lock_offset(&self) -> std::sync::MutexGuard<'_, Duration> {
        match self.offset.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.base + *self.lock_offset()
    }

    fn now_millis(&self) -> i64 {
        0
    }
}

/// 每产出一条 delta 前推进 `gap`：模拟「chunk 连续到达，但累计耗时增长」。
///
/// 与真实等待无关——推进只改时钟读数，`poll_next` 立即 `Ready`。
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
