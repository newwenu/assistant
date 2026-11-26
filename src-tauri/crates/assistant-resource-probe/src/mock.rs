//! 假快照来源：纯逻辑层单测用（I76 §10.1）。
//!
//! 计数用原子量而非 `Cell`，因为 trait 对象要能跨线程搬进 `spawn_blocking`。

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::source::{RawSnapshot, SnapshotSource};

#[derive(Debug)]
pub struct MockSource {
    raw: Result<RawSnapshot, String>,
    calls: AtomicUsize,
}

impl MockSource {
    pub fn new(raw: RawSnapshot) -> Self {
        Self {
            raw: Ok(raw),
            calls: AtomicUsize::new(0),
        }
    }

    pub fn failing(reason: &str) -> Self {
        Self {
            raw: Err(reason.to_string()),
            calls: AtomicUsize::new(0),
        }
    }

    /// 已被调用几次：用于断言「每拍只枚举一次」（I76 §2.2 约束 2）。
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl SnapshotSource for MockSource {
    fn snapshot(&mut self) -> Result<RawSnapshot, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.raw.clone()
    }
}
