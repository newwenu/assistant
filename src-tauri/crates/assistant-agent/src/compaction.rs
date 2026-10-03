// 参考: OpenBitFun/crates/execution/agent-runtime/src/compression_prefetch.rs — Context Compaction

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompactionState {
    pub total_tokens: u32,
    pub max_tokens: u32,
    pub compaction_threshold_percent: u8,
    pub messages_before_compaction: usize,
    pub messages_after_compaction: usize,
}

impl CompactionState {
    pub fn new(max_tokens: u32) -> Self {
        Self {
            total_tokens: 0,
            max_tokens,
            compaction_threshold_percent: 80,
            messages_before_compaction: 0,
            messages_after_compaction: 0,
        }
    }

    pub fn needs_compaction(&self) -> bool {
        let threshold = self.max_tokens as f32 * (self.compaction_threshold_percent as f32 / 100.0);
        self.total_tokens as f32 >= threshold
    }

    pub fn update_token_count(&mut self, tokens: u32) {
        self.total_tokens = tokens;
    }
}