use std::collections::BTreeMap;
use std::time::Instant;

use log::{debug, warn};

use crate::types::unified::{UnifiedResponse, UnifiedTokenUsage};

#[derive(Debug)]
pub struct StreamStats {
    provider: String,
    started_at: Instant,
    first_event_at: Option<Instant>,
    last_event_at: Option<Instant>,
    total_sse_events: usize,
    total_unified_responses: usize,
    last_usage: Option<UnifiedTokenUsage>,
    last_replay_protocol: Option<String>,
    counters: BTreeMap<String, usize>,
}

impl StreamStats {
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            started_at: Instant::now(),
            first_event_at: None,
            last_event_at: None,
            total_sse_events: 0,
            total_unified_responses: 0,
            last_usage: None,
            last_replay_protocol: None,
            counters: BTreeMap::new(),
        }
    }

    pub fn record_sse_event(&mut self, event_kind: impl AsRef<str>) {
        let now = Instant::now();
        if self.first_event_at.is_none() {
            self.first_event_at = Some(now);
        }
        self.last_event_at = Some(now);
        self.total_sse_events += 1;
        self.increment(format!("sse:{}", event_kind.as_ref()));
    }

    pub fn increment(&mut self, label: impl Into<String>) {
        *self.counters.entry(label.into()).or_insert(0) += 1;
    }

    pub fn counter(&self, label: &str) -> usize {
        self.counters.get(label).copied().unwrap_or(0)
    }

    pub fn record_unified_response(&mut self, response: &UnifiedResponse) {
        self.total_unified_responses += 1;

        let mut classified = false;

        if response.text.is_some() {
            self.increment("out:text");
            classified = true;
        }
        if response.reasoning_content.is_some() {
            self.increment("out:reasoning");
            classified = true;
        }
        if response.tool_call.is_some() {
            self.increment("out:tool_call");
            classified = true;
        }
        if let Some(usage) = response.usage.as_ref() {
            self.last_usage = Some(usage.clone());
            self.increment("out:usage");
            classified = true;
        }
        if response.finish_reason.is_some() {
            self.increment("out:finish_reason");
            classified = true;
        }
        if response.thinking_signature.is_some() {
            self.increment("out:thinking_signature");
            classified = true;
        }
        if response.provider_metadata.is_some() {
            self.increment("out:provider_metadata");
            classified = true;
        }
        if let Some(capture) = response.model_response_replay.as_ref() {
            self.last_replay_protocol = Some(capture.protocol.clone());
            self.increment("out:replay_capture");
            classified = true;
        }

        if !classified {
            self.increment("out:other");
        }
    }

    pub fn log_summary(&self, reason: &str) {
        let wall_elapsed_ms = self.started_at.elapsed().as_millis();
        let first_event_latency_ms = self
            .first_event_at
            .map(|instant| instant.duration_since(self.started_at).as_millis())
            .unwrap_or(0);
        let receive_elapsed_secs = match (self.first_event_at, self.last_event_at) {
            (Some(first), Some(last)) => last.duration_since(first).as_secs_f64(),
            _ => 0.0,
        };
        let receive_elapsed_ms = (receive_elapsed_secs * 1000.0).round() as u128;
        let unified_response_rate_per_sec = if receive_elapsed_secs > 0.0 {
            self.total_unified_responses as f64 / receive_elapsed_secs
        } else {
            0.0
        };

        let counter_lines = if self.counters.is_empty() {
            "counter.none=0".to_string()
        } else {
            self.counters
                .iter()
                .map(|(label, count)| format!("counter.{}={}", label, count))
                .collect::<Vec<_>>()
                .join(" ")
        };

        if let Some(usage) = self.last_usage.as_ref() {
            debug!(
                target: "ai::stream_usage",
                "{} final token usage: input={}, output={}, total={}, reasoning={:?}, cache_read={:?}, cache_write={:?}",
                self.provider,
                usage.prompt_token_count,
                usage.candidates_token_count,
                usage.total_token_count,
                usage.reasoning_token_count,
                usage.cached_content_token_count,
                usage.cache_creation_token_count
            );
        }

        debug!(
            target: "ai::stream_stats",
            "{} stream stats | reason={} | sse_events={} | unified_responses={} | first_event_latency_ms={} | receive_elapsed_ms={} | wall_elapsed_ms={} | rate_per_sec={:.2} | {}",
            self.provider,
            reason,
            self.total_sse_events,
            self.total_unified_responses,
            first_event_latency_ms,
            receive_elapsed_ms,
            wall_elapsed_ms,
            unified_response_rate_per_sec,
            counter_lines
        );

        if self.counter("out:replay_capture") > 0 {
            warn!(
                target: "ai::stream_stats",
                "{} opaque reasoning captured (replay-only): count={} protocol={}",
                self.provider,
                self.counter("out:replay_capture"),
                self.last_replay_protocol.as_deref().unwrap_or("unknown")
            );
        }
    }

    pub fn total_sse_events(&self) -> usize {
        self.total_sse_events
    }
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;
    use crate::types::unified::UnifiedResponse;

    #[test]
    fn records_sse_events() {
        let mut stats = StreamStats::new("openai");
        stats.record_sse_event("data");
        stats.record_sse_event("data");
        stats.record_sse_event("done");
        assert_eq!(stats.total_sse_events(), 3);
        assert_eq!(stats.counters.get("sse:data"), Some(&2));
        assert_eq!(stats.counters.get("sse:done"), Some(&1));
    }

    #[test]
    fn records_unified_response_fields() {
        let mut stats = StreamStats::new("openai");
        stats.record_unified_response(&UnifiedResponse {
            text: Some("hello".to_string()),
            ..Default::default()
        });
        stats.record_unified_response(&UnifiedResponse {
            reasoning_content: Some("thinking".to_string()),
            ..Default::default()
        });
        stats.record_unified_response(&UnifiedResponse {
            finish_reason: Some("stop".to_string()),
            ..Default::default()
        });
        assert_eq!(stats.total_unified_responses, 3);
        assert_eq!(stats.counters.get("out:text"), Some(&1));
        assert_eq!(stats.counters.get("out:reasoning"), Some(&1));
        assert_eq!(stats.counters.get("out:finish_reason"), Some(&1));
    }

    #[test]
    fn custom_counter() {
        let mut stats = StreamStats::new("openai");
        stats.increment("skip:unknown_event");
        stats.increment("skip:unknown_event");
        assert_eq!(stats.counters.get("skip:unknown_event"), Some(&2));
    }

    #[test]
    fn last_usage_tracked() {
        let mut stats = StreamStats::new("openai");
        stats.record_unified_response(&UnifiedResponse {
            usage: Some(UnifiedTokenUsage {
                prompt_token_count: 10,
                candidates_token_count: 20,
                total_token_count: 30,
                reasoning_token_count: None,
                cached_content_token_count: None,
                cache_creation_token_count: None,
            }),
            ..Default::default()
        });
        let usage = stats.last_usage.as_ref().unwrap();
        assert_eq!(usage.prompt_token_count, 10);
        assert_eq!(usage.candidates_token_count, 20);
    }

    #[test]
    fn records_replay_capture_separately() {
        use crate::types::unified::{ModelResponseReplayCapture, ModelResponseReplayItem};
        let mut stats = StreamStats::new("codex");
        stats.record_unified_response(&UnifiedResponse {
            model_response_replay: Some(ModelResponseReplayCapture {
                protocol: "codex".to_string(),
                items: vec![ModelResponseReplayItem::OpaqueReasoning {
                    item_id: None,
                    summary: Vec::new(),
                    opaque_state: "enc".to_string(),
                }],
            }),
            ..Default::default()
        });
        assert_eq!(stats.counter("out:replay_capture"), 1);
        assert_eq!(stats.counter("out:other"), 0);
        assert_eq!(stats.last_replay_protocol.as_deref(), Some("codex"));
    }

    #[test]
    fn no_replay_capture_for_plain_text() {
        let mut stats = StreamStats::new("openai");
        stats.record_unified_response(&UnifiedResponse {
            text: Some("hi".to_string()),
            ..Default::default()
        });
        assert_eq!(stats.counter("out:replay_capture"), 0);
        assert!(stats.last_replay_protocol.is_none());
    }
}
