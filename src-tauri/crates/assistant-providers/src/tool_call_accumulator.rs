use log::{error, warn};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::types::unified::UnifiedResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallBoundary {
    NewTool,
    FinishReason,
    StreamEnd,
}

#[derive(Debug, Clone, Default)]
pub struct FinalizedToolCall {
    pub tool_id: String,
    pub tool_name: String,
    pub raw_arguments: String,
    pub arguments: Value,
    pub is_error: bool,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolCallStreamKey {
    Indexed(usize),
    Unindexed,
}

impl From<Option<usize>> for ToolCallStreamKey {
    fn from(value: Option<usize>) -> Self {
        match value {
            Some(index) => Self::Indexed(index),
            None => Self::Unindexed,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct PendingToolCall {
    tool_id: String,
    tool_name: String,
    raw_arguments: String,
}

impl PendingToolCall {
    fn has_pending(&self) -> bool {
        !self.tool_id.is_empty()
    }

    fn has_meaningful_payload(&self) -> bool {
        !self.tool_name.is_empty() || !self.raw_arguments.is_empty()
    }

    fn start_new(&mut self, tool_id: String, tool_name: Option<String>) {
        self.tool_id = tool_id;
        self.tool_name = tool_name.unwrap_or_default();
        self.raw_arguments.clear();
    }

    fn update_tool_name_if_missing(&mut self, tool_name: Option<String>) {
        if self.tool_name.is_empty() {
            self.tool_name = tool_name.unwrap_or_default();
        }
    }

    fn append_arguments(&mut self, arguments_chunk: &str) {
        self.raw_arguments.push_str(arguments_chunk);
    }

    fn replace_arguments(&mut self, arguments_snapshot: &str) {
        self.raw_arguments.clear();
        self.raw_arguments.push_str(arguments_snapshot);
    }

    fn finalize(&mut self, boundary: ToolCallBoundary) -> Option<FinalizedToolCall> {
        if !self.has_pending() {
            return None;
        }

        if !self.has_meaningful_payload() {
            self.tool_id.clear();
            self.tool_name.clear();
            self.raw_arguments.clear();
            return None;
        }

        let tool_id = std::mem::take(&mut self.tool_id);
        let tool_name = std::mem::take(&mut self.tool_name);
        let raw_arguments = std::mem::take(&mut self.raw_arguments);

        let (arguments, is_error, parse_error) = match serde_json::from_str::<Value>(&raw_arguments)
        {
            Ok(value) => (value, false, None),
            Err(e) => {
                let error_msg = e.to_string();
                let repaired = repair_truncated_json(&raw_arguments)
                    .and_then(|candidate| serde_json::from_str::<Value>(&candidate).ok());
                match repaired {
                    Some(value) => {
                        warn!(
                            "Tool call arguments repaired at boundary={:?}: tool_id={}, tool_name={}, raw_len={}",
                            boundary, tool_id, tool_name, raw_arguments.len()
                        );
                        (value, false, Some(error_msg))
                    }
                    None => {
                        error!(
                            "Tool call arguments parsing failed at boundary={:?}: tool_id={}, tool_name={}, raw_len={}, error={}",
                            boundary, tool_id, tool_name, raw_arguments.len(), error_msg
                        );
                        (Value::Object(serde_json::Map::new()), true, Some(error_msg))
                    }
                }
            }
        };

        Some(FinalizedToolCall {
            tool_id,
            tool_name,
            raw_arguments,
            arguments,
            is_error,
            parse_error,
        })
    }
}

fn repair_truncated_json(raw: &str) -> Option<String> {
    let mut in_string = false;
    let mut escape = false;
    let mut stack: Vec<u8> = Vec::new();
    let mut last_significant: Option<u8> = None;

    for &b in raw.as_bytes() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            match b {
                b'\\' => escape = true,
                b'"' => {
                    in_string = false;
                    last_significant = Some(b'"');
                }
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => {
                in_string = true;
                last_significant = Some(b'"');
            }
            b'{' => {
                stack.push(b'{');
                last_significant = Some(b'{');
            }
            b'[' => {
                stack.push(b'[');
                last_significant = Some(b'[');
            }
            b'}' => {
                if stack.pop() != Some(b'{') {
                    return None;
                }
                last_significant = Some(b'}');
            }
            b']' => {
                if stack.pop() != Some(b'[') {
                    return None;
                }
                last_significant = Some(b']');
            }
            b' ' | b'\t' | b'\n' | b'\r' => {}
            other => last_significant = Some(other),
        }
    }

    if !in_string && stack.is_empty() {
        return None;
    }

    if !in_string {
        if let Some(b',') | Some(b':') = last_significant {
            return None;
        }
    }

    let mut out = String::with_capacity(raw.len() + stack.len() + 1);
    out.push_str(raw);
    if in_string {
        out.push('"');
    }
    while let Some(c) = stack.pop() {
        out.push(match c {
            b'{' => '}',
            b'[' => ']',
            _ => unreachable!(),
        });
    }
    Some(out)
}

#[derive(Debug, Clone, Default)]
pub struct ToolCallDeltaOutcome {
    pub finalized_previous: Option<FinalizedToolCall>,
    pub params_partial: Option<ToolCallParamsChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallParamsChunk {
    pub tool_id: String,
    pub tool_name: String,
    pub params_chunk: String,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCallAccumulator {
    pending: BTreeMap<ToolCallStreamKey, PendingToolCall>,
}

impl ToolCallAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_unified_response(&mut self, response: &UnifiedResponse) -> ToolCallDeltaOutcome {
        let Some(tc) = &response.tool_call else {
            return ToolCallDeltaOutcome::default();
        };

        let key = ToolCallStreamKey::from(tc.tool_call_index);
        let tool_id = tc.id.clone();
        let tool_name = tc.name.clone();
        let arguments = tc.arguments.clone();

        self.apply_delta(key, tool_id, tool_name, arguments, false)
    }

    pub fn apply_delta(
        &mut self,
        key: ToolCallStreamKey,
        tool_id: Option<String>,
        tool_name: Option<String>,
        arguments: Option<String>,
        arguments_is_snapshot: bool,
    ) -> ToolCallDeltaOutcome {
        let mut outcome = ToolCallDeltaOutcome::default();

        let has_tool_id = tool_id.as_ref().is_some_and(|id| !id.is_empty());
        if !self.pending.contains_key(&key) {
            if has_tool_id {
                self.pending.insert(key.clone(), PendingToolCall::default());
            } else {
                return outcome;
            }
        }

        let Some(pending) = self.pending.get_mut(&key) else {
            return outcome;
        };

        if let Some(tool_id) = tool_id.filter(|id| !id.is_empty()) {
            let is_new_tool = pending.tool_id != tool_id;
            if is_new_tool {
                outcome.finalized_previous = pending.finalize(ToolCallBoundary::NewTool);
                pending.start_new(tool_id, tool_name.clone());
            } else {
                pending.update_tool_name_if_missing(tool_name.clone());
            }
        } else if tool_name.as_ref().is_some_and(|name| !name.is_empty()) {
            pending.update_tool_name_if_missing(tool_name.clone());
        }

        if let Some(arguments) = arguments.filter(|a| !a.is_empty()) {
            if pending.has_pending() {
                if arguments_is_snapshot {
                    pending.replace_arguments(&arguments);
                } else {
                    pending.append_arguments(&arguments);
                }
                let tool_name = pending.tool_name.clone();
                if !arguments.is_empty() {
                    outcome.params_partial = Some(ToolCallParamsChunk {
                        tool_id: pending.tool_id.clone(),
                        tool_name,
                        params_chunk: arguments,
                    });
                }
            }
        }

        outcome
    }

    pub fn finalize_key(
        &mut self,
        key: &ToolCallStreamKey,
        boundary: ToolCallBoundary,
    ) -> Option<FinalizedToolCall> {
        let mut pending = self.pending.remove(key)?;
        pending.finalize(boundary)
    }

    pub fn finalize_all(&mut self, boundary: ToolCallBoundary) -> Vec<FinalizedToolCall> {
        let keys: Vec<_> = self.pending.keys().cloned().collect();
        keys.into_iter()
            .filter_map(|key| self.finalize_key(&key, boundary))
            .collect()
    }

    pub fn has_pending(&self) -> bool {
        self.pending.values().any(|p| p.has_pending())
    }
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;
    use crate::types::unified::UnifiedToolCall;

    #[test]
    fn accumulates_tool_call_across_chunks() {
        let mut acc = ToolCallAccumulator::new();

        acc.apply_delta(
            ToolCallStreamKey::Indexed(0),
            Some("call_1".to_string()),
            Some("search".to_string()),
            Some("{\"q\":".to_string()),
            false,
        );
        acc.apply_delta(
            ToolCallStreamKey::Indexed(0),
            None,
            None,
            Some("\"test\"}".to_string()),
            false,
        );

        let finalized = acc.finalize_all(ToolCallBoundary::FinishReason);
        assert_eq!(finalized.len(), 1);
        assert_eq!(finalized[0].tool_id, "call_1");
        assert_eq!(finalized[0].tool_name, "search");
        assert_eq!(finalized[0].arguments, serde_json::json!({"q": "test"}));
        assert!(!finalized[0].is_error);
    }

    #[test]
    fn invalid_json_becomes_error() {
        let mut acc = ToolCallAccumulator::new();

        acc.apply_delta(
            ToolCallStreamKey::Indexed(0),
            Some("call_1".to_string()),
            Some("tool_a".to_string()),
            Some("{\"a\":".to_string()),
            false,
        );

        let finalized = acc.finalize_all(ToolCallBoundary::StreamEnd);
        assert_eq!(finalized.len(), 1);
        assert!(finalized[0].is_error);
        assert_eq!(finalized[0].raw_arguments, "{\"a\":");
    }

    #[test]
    fn new_tool_finalizes_previous() {
        let mut acc = ToolCallAccumulator::new();

        acc.apply_delta(
            ToolCallStreamKey::Indexed(0),
            Some("call_1".to_string()),
            Some("tool_a".to_string()),
            Some("{\"x\":1}".to_string()),
            false,
        );

        let outcome = acc.apply_delta(
            ToolCallStreamKey::Indexed(0),
            Some("call_2".to_string()),
            Some("tool_b".to_string()),
            None,
            false,
        );

        let finalized = outcome.finalized_previous.unwrap();
        assert_eq!(finalized.tool_id, "call_1");
        assert_eq!(finalized.tool_name, "tool_a");
    }

    #[test]
    fn apply_unified_response() {
        let mut acc = ToolCallAccumulator::new();

        let response = UnifiedResponse {
            tool_call: Some(UnifiedToolCall {
                tool_call_index: Some(0),
                id: Some("tc_1".to_string()),
                name: Some("search".to_string()),
                arguments: Some("{\"q\":\"hello\"}".to_string()),
                arguments_is_snapshot: false,
            }),
            ..Default::default()
        };

        acc.apply_unified_response(&response);

        let finalized = acc.finalize_all(ToolCallBoundary::FinishReason);
        assert_eq!(finalized.len(), 1);
        assert_eq!(finalized[0].tool_id, "tc_1");
    }

    #[test]
    fn repair_truncated_json_cases() {
        assert_eq!(
            repair_truncated_json("{\"a\":1"),
            Some("{\"a\":1}".to_string())
        );
        assert_eq!(
            repair_truncated_json("{\"a\":[1,2"),
            Some("{\"a\":[1,2]}".to_string())
        );
        assert_eq!(
            repair_truncated_json("{\"a\":\"hello"),
            Some("{\"a\":\"hello\"}".to_string())
        );
        assert_eq!(repair_truncated_json("{\"a\":"), None);
    }

    #[test]
    fn snapshot_replaces_arguments() {
        let mut acc = ToolCallAccumulator::new();

        acc.apply_delta(
            ToolCallStreamKey::Indexed(0),
            Some("call_1".to_string()),
            Some("tool_a".to_string()),
            Some("{\"partial\"".to_string()),
            false,
        );
        acc.apply_delta(
            ToolCallStreamKey::Indexed(0),
            None,
            None,
            Some("{\"full\":true}".to_string()),
            true,
        );

        let finalized = acc.finalize_all(ToolCallBoundary::FinishReason);
        assert_eq!(finalized[0].raw_arguments, "{\"full\":true}");
    }
}
