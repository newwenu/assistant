use std::mem;

use crate::types::unified::{ReasoningContentKind, UnifiedResponse, UnifiedTokenUsage};

const INLINE_THINK_OPEN_TAG: &str = "<think>";
const INLINE_THINK_CLOSE_TAG: &str = "</think>";

#[derive(Debug, Default)]
struct DeferredResponseMeta {
    usage: Option<UnifiedTokenUsage>,
    finish_reason: Option<String>,
    provider_metadata: Option<serde_json::Value>,
}

impl DeferredResponseMeta {
    fn from_response(response: &mut UnifiedResponse) -> Self {
        Self {
            usage: response.usage.take(),
            finish_reason: response.finish_reason.take(),
            provider_metadata: response.provider_metadata.take(),
        }
    }

    fn merge(&mut self, other: Self) {
        if other.usage.is_some() {
            self.usage = other.usage;
        }
        if other.finish_reason.is_some() {
            self.finish_reason = other.finish_reason;
        }
        if other.provider_metadata.is_some() {
            self.provider_metadata = other.provider_metadata;
        }
    }

    fn apply_to(self, response: &mut UnifiedResponse) {
        if response.usage.is_none() {
            response.usage = self.usage;
        }
        if response.finish_reason.is_none() {
            response.finish_reason = self.finish_reason;
        }
        if response.provider_metadata.is_none() {
            response.provider_metadata = self.provider_metadata;
        }
    }

    fn is_empty(&self) -> bool {
        self.usage.is_none() && self.finish_reason.is_none() && self.provider_metadata.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineThinkActivation {
    Unknown,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineThinkMode {
    Text,
    Thinking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InlineThinkSegment {
    Text(String),
    Thinking(String),
}

#[derive(Debug)]
pub struct InlineThinkParser {
    enabled: bool,
    activation: InlineThinkActivation,
    mode: InlineThinkMode,
    pending_tail: String,
    initial_probe: String,
    deferred_meta: DeferredResponseMeta,
}

impl InlineThinkParser {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            activation: InlineThinkActivation::Unknown,
            mode: InlineThinkMode::Text,
            pending_tail: String::new(),
            initial_probe: String::new(),
            deferred_meta: DeferredResponseMeta::default(),
        }
    }

    pub fn normalize_response(&mut self, mut response: UnifiedResponse) -> Vec<UnifiedResponse> {
        if !self.enabled {
            return vec![response];
        }

        let Some(text) = response.text.take() else {
            return vec![response];
        };

        if response.reasoning_content.is_some()
            || response.tool_call.is_some()
            || response.thinking_signature.is_some()
        {
            response.text = Some(text);
            return vec![response];
        }

        let current_meta = DeferredResponseMeta::from_response(&mut response);
        let segments = match self.activation {
            InlineThinkActivation::Unknown => self.consume_unknown_text(text),
            InlineThinkActivation::Enabled => self.parse_enabled_text(text),
            InlineThinkActivation::Disabled => vec![InlineThinkSegment::Text(text)],
        };

        self.attach_meta_to_segments(segments, current_meta)
    }

    pub fn flush(&mut self) -> Vec<UnifiedResponse> {
        if !self.enabled {
            return Vec::new();
        }

        let segments = match self.activation {
            InlineThinkActivation::Unknown => {
                let pending = mem::take(&mut self.initial_probe);
                if pending.is_empty() {
                    Vec::new()
                } else {
                    vec![InlineThinkSegment::Text(pending)]
                }
            }
            InlineThinkActivation::Enabled => {
                let pending = mem::take(&mut self.pending_tail);
                if pending.is_empty() {
                    Vec::new()
                } else if self.mode == InlineThinkMode::Thinking {
                    vec![InlineThinkSegment::Thinking(pending)]
                } else {
                    vec![InlineThinkSegment::Text(pending)]
                }
            }
            InlineThinkActivation::Disabled => Vec::new(),
        };

        self.attach_meta_to_segments(segments, DeferredResponseMeta::default())
    }

    fn consume_unknown_text(&mut self, text: String) -> Vec<InlineThinkSegment> {
        self.initial_probe.push_str(&text);

        let trimmed = self.initial_probe.trim_start_matches(char::is_whitespace);
        if trimmed.is_empty() {
            return Vec::new();
        }

        if trimmed.starts_with(INLINE_THINK_OPEN_TAG) {
            self.activation = InlineThinkActivation::Enabled;
            let buffered = mem::take(&mut self.initial_probe);
            return self.parse_enabled_text(buffered);
        }

        if INLINE_THINK_OPEN_TAG.starts_with(trimmed) {
            return Vec::new();
        }

        self.activation = InlineThinkActivation::Disabled;
        vec![InlineThinkSegment::Text(mem::take(&mut self.initial_probe))]
    }

    fn parse_enabled_text(&mut self, text: String) -> Vec<InlineThinkSegment> {
        let mut data = mem::take(&mut self.pending_tail);
        data.push_str(&text);

        let mut segments = Vec::new();

        loop {
            let marker = match self.mode {
                InlineThinkMode::Text => INLINE_THINK_OPEN_TAG,
                InlineThinkMode::Thinking => INLINE_THINK_CLOSE_TAG,
            };

            if let Some(marker_idx) = data.find(marker) {
                let before_marker = data[..marker_idx].to_string();
                self.push_segment(&mut segments, before_marker);

                data = data[marker_idx + marker.len()..].to_string();
                self.mode = match self.mode {
                    InlineThinkMode::Text => InlineThinkMode::Thinking,
                    InlineThinkMode::Thinking => InlineThinkMode::Text,
                };
                continue;
            }

            let tail_len = longest_suffix_prefix_len(&data, marker);
            let flush_len = data.len() - tail_len;
            let ready = data[..flush_len].to_string();
            self.push_segment(&mut segments, ready);
            self.pending_tail = data[flush_len..].to_string();
            break;
        }

        segments
    }

    fn push_segment(&self, segments: &mut Vec<InlineThinkSegment>, content: String) {
        if content.is_empty() {
            return;
        }

        match self.mode {
            InlineThinkMode::Text => segments.push(InlineThinkSegment::Text(content)),
            InlineThinkMode::Thinking => segments.push(InlineThinkSegment::Thinking(content)),
        }
    }

    fn attach_meta_to_segments(
        &mut self,
        segments: Vec<InlineThinkSegment>,
        current_meta: DeferredResponseMeta,
    ) -> Vec<UnifiedResponse> {
        let mut merged_meta = mem::take(&mut self.deferred_meta);
        merged_meta.merge(current_meta);

        let mut responses: Vec<UnifiedResponse> = segments
            .into_iter()
            .map(|segment| match segment {
                InlineThinkSegment::Text(text) => UnifiedResponse {
                    text: Some(text),
                    ..Default::default()
                },
                InlineThinkSegment::Thinking(reasoning_content) => UnifiedResponse {
                    reasoning_content: Some(reasoning_content),
                    reasoning_content_kind: Some(ReasoningContentKind::Reasoning),
                    ..Default::default()
                },
            })
            .collect();

        if let Some(last_response) = responses.last_mut() {
            merged_meta.apply_to(last_response);
        } else if !merged_meta.is_empty() {
            self.deferred_meta = merged_meta;
        }

        responses
    }
}

fn longest_suffix_prefix_len(value: &str, marker: &str) -> usize {
    let max_len = value.len().min(marker.len().saturating_sub(1));
    (1..=max_len)
        .rev()
        .find(|&len| {
            if value.len() < len || !value.is_char_boundary(value.len() - len) {
                return false;
            }
            let suffix = &value[value.len() - len..];
            marker.starts_with(suffix)
        })
        .unwrap_or(0)
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn longest_suffix_prefix_len_detects_partial_tag_boundary() {
        assert_eq!(longest_suffix_prefix_len("<thi", INLINE_THINK_OPEN_TAG), 4);
        assert_eq!(
            longest_suffix_prefix_len("answer", INLINE_THINK_OPEN_TAG),
            0
        );
    }

    #[test]
    fn inline_think_parser_streams_thinking_and_text_per_chunk() {
        let mut parser = InlineThinkParser::new(true);

        let chunk1 = parser.normalize_response(UnifiedResponse {
            text: Some(format!("{}abc", INLINE_THINK_OPEN_TAG)),
            ..Default::default()
        });
        let chunk2 = parser.normalize_response(UnifiedResponse {
            text: Some(format!("def{}ghi", INLINE_THINK_CLOSE_TAG)),
            ..Default::default()
        });

        assert_eq!(chunk1.len(), 1);
        assert_eq!(chunk1[0].reasoning_content.as_deref(), Some("abc"));

        assert_eq!(chunk2.len(), 2);
        assert_eq!(chunk2[0].reasoning_content.as_deref(), Some("def"));
        assert_eq!(chunk2[1].text.as_deref(), Some("ghi"));
    }

    #[test]
    fn disabled_passthrough() {
        let mut parser = InlineThinkParser::new(false);
        let result = parser.normalize_response(UnifiedResponse {
            text: Some(format!(
                "{}thinking{}text",
                INLINE_THINK_OPEN_TAG, INLINE_THINK_CLOSE_TAG
            )),
            ..Default::default()
        });
        assert_eq!(result.len(), 1);
        let expected = format!(
            "{}thinking{}text",
            INLINE_THINK_OPEN_TAG, INLINE_THINK_CLOSE_TAG
        );
        assert_eq!(result[0].text.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn no_think_tag_passthrough() {
        let mut parser = InlineThinkParser::new(true);
        let result = parser.normalize_response(UnifiedResponse {
            text: Some("just regular text".to_string()),
            ..Default::default()
        });
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text.as_deref(), Some("just regular text"));
    }

    #[test]
    fn flush_pending_thinking() {
        let mut parser = InlineThinkParser::new(true);
        let result = parser.normalize_response(UnifiedResponse {
            text: Some(format!("{}unfinished", INLINE_THINK_OPEN_TAG)),
            ..Default::default()
        });
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].reasoning_content.as_deref(), Some("unfinished"));
    }

    #[test]
    fn flush_pending_thinking_across_chunks() {
        let mut parser = InlineThinkParser::new(true);
        let r1 = parser.normalize_response(UnifiedResponse {
            text: Some(format!("{}abc", INLINE_THINK_OPEN_TAG)),
            ..Default::default()
        });
        let r2 = parser.normalize_response(UnifiedResponse {
            text: Some("def".to_string()),
            ..Default::default()
        });
        let flushed = parser.flush();
        let all_reasoning: String = r1
            .iter()
            .chain(r2.iter())
            .chain(flushed.iter())
            .filter_map(|r| r.reasoning_content.as_ref())
            .cloned()
            .collect();
        assert_eq!(all_reasoning, "abcdef");
    }

    #[test]
    fn respects_existing_reasoning_content() {
        let mut parser = InlineThinkParser::new(true);
        let result = parser.normalize_response(UnifiedResponse {
            text: Some(format!(
                "{}thinking{}text",
                INLINE_THINK_OPEN_TAG, INLINE_THINK_CLOSE_TAG
            )),
            reasoning_content: Some("native reasoning".to_string()),
            ..Default::default()
        });
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].reasoning_content.as_deref(),
            Some("native reasoning")
        );
        assert!(result[0].text.is_some());
    }
}
