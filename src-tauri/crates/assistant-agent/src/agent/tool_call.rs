use std::collections::BTreeMap;

use assistant_providers::types::unified::UnifiedResponse;
use log::{error, warn};
use serde_json::Value;

use assistant_providers::base::ChatDelta;

/// 触发终结的边界，只用于日志与测试断言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallBoundary {
    NewTool,
    FinishReason,
    StreamEnd,
}

/// 累积完成的工具调用。`arguments` 已解析，`raw_arguments` 保留原始文本。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FinalizedToolCall {
    pub tool_id: String,
    pub tool_name: String,
    pub raw_arguments: String,
    pub arguments: Value,
    pub is_error: bool,
    pub parse_error: Option<String>,
}

/// 流内分组键：有 index 用 index，无 index 视作单槽。
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

/// 对截断 JSON 做最小补全（补齐未闭合字符串与括号）。
pub fn repair_truncated_json(raw: &str) -> Option<String> {
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

/// 跨 chunk 累积工具调用参数。provider 只负责透传片段与快照标记，分组与拼接在此完成。
#[derive(Debug, Clone, Default)]
pub struct ToolCallAccumulator {
    pending: BTreeMap<ToolCallStreamKey, PendingToolCall>,
}

impl ToolCallAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 provider 中立流片段累积。缺 `delta_tool_call_id` 的快照片段按 index 合成 id。
    pub fn apply_chat_delta(&mut self, delta: &ChatDelta) -> ToolCallDeltaOutcome {
        if delta.delta_tool_call_id.is_none()
            && delta.delta_tool_call_name.is_none()
            && delta.delta_tool_call_arguments.is_none()
        {
            return ToolCallDeltaOutcome::default();
        }

        self.apply_delta(
            ToolCallStreamKey::from(delta.delta_tool_call_index),
            delta.delta_tool_call_id.clone(),
            delta.delta_tool_call_name.clone(),
            delta.delta_tool_call_arguments.clone(),
            delta.delta_tool_call_arguments_is_snapshot,
        )
    }

    pub fn apply_unified_response(&mut self, response: &UnifiedResponse) -> ToolCallDeltaOutcome {
        let Some(tc) = &response.tool_call else {
            return ToolCallDeltaOutcome::default();
        };

        self.apply_delta(
            ToolCallStreamKey::from(tc.tool_call_index),
            tc.id.clone(),
            tc.name.clone(),
            tc.arguments.clone(),
            tc.arguments_is_snapshot,
        )
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
            let start_id = if has_tool_id {
                tool_id.clone().filter(|id| !id.is_empty())
            } else if arguments_is_snapshot
                && tool_name.as_ref().is_some_and(|name| !name.is_empty())
            {
                Some(synthetic_tool_id(&key))
            } else {
                None
            };
            let Some(start_id) = start_id else {
                return outcome;
            };
            let mut pending = PendingToolCall::default();
            pending.start_new(start_id, tool_name.clone());
            self.pending.insert(key.clone(), pending);
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
                outcome.params_partial = Some(ToolCallParamsChunk {
                    tool_id: pending.tool_id.clone(),
                    tool_name: pending.tool_name.clone(),
                    params_chunk: arguments,
                });
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

/// 无 id 的快照流（Gemini）按分组键合成稳定 id，保证同一轮内唯一。
fn synthetic_tool_id(key: &ToolCallStreamKey) -> String {
    match key {
        ToolCallStreamKey::Indexed(index) => format!("call_{index}"),
        ToolCallStreamKey::Unindexed => "call_0".to_string(),
    }
}
