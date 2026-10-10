use crate::base::{ChatMessage, MessageRole};
use serde_json::{json, Value};

pub(crate) fn role_str(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

pub(crate) fn convert_messages(messages: &[ChatMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| match m.role {
            MessageRole::Tool => {
                let tool_call_id = m.tool_call_id.as_deref().unwrap_or("");
                json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": m.content
                })
            }
            MessageRole::Assistant if m.tool_calls.is_some() => {
                let tool_calls: Vec<Value> = m
                    .tool_calls
                    .as_ref()
                    .map(|calls| {
                        calls
                            .iter()
                            .map(|tc| {
                                json!({
                                    "id": tc.id,
                                    "type": "function",
                                    "function": {
                                        "name": tc.name,
                                        "arguments": tc.arguments
                                    }
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                json!({
                    "role": "assistant",
                    "content": if m.content.is_empty() { Value::Null } else { json!(m.content) },
                    "tool_calls": tool_calls
                })
            }
            _ => {
                json!({
                    "role": role_str(&m.role),
                    "content": m.content
                })
            }
        })
        .collect()
}
