use std::collections::VecDeque;

use log::error;

use crate::base::SseLogConfig;

pub struct SseLogCollector {
    buffer: VecDeque<String>,
    retained_bytes: usize,
    dropped_events: usize,
    dropped_bytes: usize,
    config: SseLogConfig,
}

impl SseLogCollector {
    pub fn new(config: SseLogConfig) -> Self {
        Self {
            buffer: VecDeque::with_capacity(config.max_events.min(512)),
            retained_bytes: 0,
            dropped_events: 0,
            dropped_bytes: 0,
            config,
        }
    }

    pub fn push(&mut self, data: String) {
        let byte_len = data.len();
        while self.buffer.len() >= self.config.max_events || (self.retained_bytes + byte_len > self.config.max_retained_bytes && !self.buffer.is_empty()) {
            if let Some(removed) = self.buffer.pop_front() {
                self.retained_bytes -= removed.len();
                self.dropped_events += 1;
                self.dropped_bytes += removed.len();
            }
        }
        self.retained_bytes += byte_len;
        self.buffer.push_back(data);
    }

    pub fn flush_on_error(&self, context: &str) {
        if self.buffer.is_empty() {
            error!("SSE error: {} (no SSE history retained)", context);
            return;
        }
        let total = self.buffer.len() + self.dropped_events;
        let head_count = 50usize;
        let tail_count = self.buffer.len().saturating_sub(head_count);
        let mut output = String::with_capacity(self.retained_bytes + 256);
        output.push_str(&format!("SSE error: {}\nSSE history ({} events", context, total));
        if self.dropped_events > 0 {
            output.push_str(&format!(", {} dropped ({} bytes)", self.dropped_events, self.dropped_bytes));
        }
        output.push_str("):\n");
        for (i, event) in self.buffer.iter().enumerate() {
            if i < head_count || i >= self.buffer.len() - tail_count {
                output.push_str(&format!("  {}: {}\n", i, event));
            } else if i == head_count {
                let skipped = self.buffer.len() - head_count - tail_count;
                output.push_str(&format!("  ... ({} events omitted)\n", skipped));
            }
        }
        error!("{}", output);
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}