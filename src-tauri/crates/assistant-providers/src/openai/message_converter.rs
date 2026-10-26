use crate::base::{ChatMessage, MessageRole};
use crate::types::{parse_content, ContentBlock};
use serde_json::{json, Value};

pub(crate) fn role_str(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

fn convert_content_block(block: &ContentBlock) -> Value {
    match block {
        ContentBlock::Text { text } => json!({
            "type": "text",
            "text": text
        }),
        ContentBlock::Image {
            attachment,
            cached_data,
            ..
        } => {
            let mt = attachment.media_type.as_deref().unwrap_or("image/png");
            match cached_data.as_deref().filter(|d| !d.is_empty()) {
                Some(data) => json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}", mt, data)
                    }
                }),
                // 缺少 bytes 时降级为文本，避免发出 `data:...;base64,` 空 URI 触发 API 报错
                None => json!({
                    "type": "text",
                    "text": format!(
                        "[image unavailable: {}]",
                        attachment.name.as_deref().unwrap_or("unknown")
                    )
                }),
            }
        }
        ContentBlock::File { attachment, .. } => json!({
            "type": "text",
            "text": attachment.handle_text()
        }),
        _ => json!({}),
    }
}

fn build_content(m: &ChatMessage) -> Value {
    if let Some(blocks) = &m.content_blocks {
        let parts: Vec<Value> = blocks.iter().map(convert_content_block).collect();
        if !parts.is_empty() {
            return json!(parts);
        }
    }
    if let Some(blocks) = parse_content(&m.content) {
        let has_media = blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Image { .. } | ContentBlock::File { .. }));
        if has_media {
            let parts: Vec<Value> = blocks.iter().map(convert_content_block).collect();
            return json!(parts);
        }
    }
    json!(m.content)
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
                    "content": build_content(m)
                })
            }
        })
        .collect()
}
