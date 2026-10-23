use crate::base::{ChatMessage, MessageRole};
use crate::types::{parse_content, ContentBlock};
use serde_json::{json, Value};

pub(crate) fn convert_messages(messages: &[ChatMessage]) -> (Option<Value>, Vec<Value>) {
    let mut system_parts: Vec<Value> = Vec::new();
    let mut anthropic_messages = Vec::new();

    for msg in messages {
        match msg.role {
            MessageRole::System => {
                if !msg.content.is_empty() {
                    system_parts.push(json!({ "type": "text", "text": msg.content }));
                }
            }
            MessageRole::User => {
                anthropic_messages.push(convert_user_message(msg));
            }
            MessageRole::Assistant => {
                if let Some(converted) = convert_assistant_message(msg) {
                    anthropic_messages.push(converted);
                }
            }
            MessageRole::Tool => {
                anthropic_messages.push(convert_tool_result_message(msg));
            }
        }
    }

    let mut merged_messages = merge_consecutive_messages(anthropic_messages);
    trim_final_assistant_trailing_whitespace(&mut merged_messages);

    let system = if system_parts.is_empty() {
        None
    } else if system_parts.len() == 1 {
        system_parts.into_iter().next()
    } else {
        Some(json!(system_parts))
    };

    (system, merged_messages)
}

fn convert_user_message(msg: &ChatMessage) -> Value {
    let blocks: Option<Vec<ContentBlock>> = msg
        .content_blocks
        .as_ref()
        .map(|b| Some(b.clone()))
        .unwrap_or_else(|| parse_content(&msg.content));

    if let Some(blocks) = blocks {
        let has_image = blocks.iter().any(|b| matches!(b, ContentBlock::Image { .. }));
        if has_image {
            let content_parts: Vec<Value> = blocks
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => json!({
                        "type": "text",
                        "text": text
                    }),
                    ContentBlock::Image {
                        media_type,
                        cached_data,
                        ..
                    } => {
                        let mt = media_type.as_deref().unwrap_or("image/png");
                        let data = cached_data.as_deref().unwrap_or("");
                        json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": mt,
                                "data": data
                            }
                        })
                    }
                    _ => json!({}),
                })
                .collect();
            return json!({
                "role": "user",
                "content": content_parts
            });
        }
    }

    json!({
        "role": "user",
        "content": msg.content
    })
}

fn convert_assistant_message(msg: &ChatMessage) -> Option<Value> {
    let mut content_blocks: Vec<Value> = Vec::new();

    if let Some(ref sig) = msg.thinking_signature {
        if !sig.is_empty() {
            content_blocks.push(json!({
                "type": "thinking",
                "thinking": "",
                "signature": sig
            }));
        }
    }

    if !msg.content.is_empty() {
        content_blocks.push(json!({
            "type": "text",
            "text": msg.content
        }));
    }

    if let Some(ref tool_calls) = msg.tool_calls {
        for tc in tool_calls {
            content_blocks.push(json!({
                "type": "tool_use",
                "id": tc.id,
                "name": tc.name,
                "input": tc.arguments
            }));
        }
    }

    if content_blocks.is_empty() {
        return None;
    }

    Some(json!({
        "role": "assistant",
        "content": content_blocks
    }))
}

fn convert_tool_result_message(msg: &ChatMessage) -> Value {
    let tool_use_id = msg.tool_call_id.as_deref().unwrap_or("");
    let content = if msg.is_error {
        json!({
            "type": "text",
            "text": if msg.content.is_empty() { "Tool execution failed".to_string() } else { msg.content.clone() }
        })
    } else {
        json!({
            "type": "text",
            "text": msg.content
        })
    };

    json!({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content
        }]
    })
}

fn merge_consecutive_messages(messages: Vec<Value>) -> Vec<Value> {
    let mut merged: Vec<Value> = Vec::new();

    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

        if let Some(last) = merged.last_mut() {
            let last_role = last.get("role").and_then(|r| r.as_str()).unwrap_or("");

            if last_role == role && matches!(role, "user" | "assistant") {
                if let (Some(last_blocks), Some(current_blocks)) = (
                    last.get_mut("content").and_then(Value::as_array_mut),
                    msg.get("content").and_then(Value::as_array),
                ) {
                    last_blocks.extend(current_blocks.iter().cloned());
                    continue;
                }
            }
        }

        merged.push(msg);
    }

    merged
}

fn trim_final_assistant_trailing_whitespace(messages: &mut [Value]) {
    let Some(last) = messages.last_mut() else {
        return;
    };
    if last.get("role").and_then(|role| role.as_str()) != Some("assistant") {
        return;
    }

    match last.get_mut("content") {
        Some(Value::String(text)) => {
            let trimmed_len = text.trim_end().len();
            text.truncate(trimmed_len);
        }
        Some(Value::Array(blocks)) => {
            let Some(Value::Object(block)) = blocks.last_mut() else {
                return;
            };
            if block.get("type").and_then(|value| value.as_str()) != Some("text") {
                return;
            }
            if let Some(Value::String(text)) = block.get_mut("text") {
                let trimmed_len = text.trim_end().len();
                text.truncate(trimmed_len);
            }
        }
        _ => {}
    }
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;
    use crate::base::{MessageRole, ToolCall};

    fn make_message(role: MessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        }
    }

    #[test]
    fn system_message_separated() {
        let messages = vec![
            make_message(MessageRole::System, "You are helpful"),
            make_message(MessageRole::User, "Hello"),
        ];
        let (system, msgs) = convert_messages(&messages);
        assert_eq!(system.unwrap()["type"], json!("text"));
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], json!("user"));
    }

    #[test]
    fn consecutive_user_messages_merged() {
        let messages = vec![
            make_message(MessageRole::User, "first"),
            make_message(MessageRole::User, "second"),
        ];
        let (_, msgs) = convert_messages(&messages);
        assert_eq!(msgs.len(), 1);
        let content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
    }

    #[test]
    fn trailing_whitespace_trimmed() {
        let messages = vec![
            make_message(MessageRole::User, "Continue"),
            make_message(MessageRole::Assistant, "prefill  \n"),
        ];
        let (_, msgs) = convert_messages(&messages);
        let content = msgs.last().unwrap()["content"].as_array().unwrap();
        let text = content[0]["text"].as_str().unwrap();
        assert_eq!(text, "prefill");
        assert!(!text.ends_with(char::is_whitespace));
    }

    #[test]
    fn empty_assistant_skipped() {
        let messages = vec![
            make_message(MessageRole::User, "Hello"),
            make_message(MessageRole::Assistant, ""),
            make_message(MessageRole::User, "World"),
        ];
        let (_, msgs) = convert_messages(&messages);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], json!("user"));
        let content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
    }

    #[test]
    fn content_blocks_format() {
        let messages = vec![make_message(MessageRole::User, "Hello")];
        let (_, msgs) = convert_messages(&messages);
        let content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], json!("text"));
        assert_eq!(content[0]["text"], json!("Hello"));
    }

    #[test]
    fn tool_result_converted() {
        let messages = vec![ChatMessage {
            role: MessageRole::Tool,
            content: "{\"temp\": 22}".to_string(),
            tool_calls: None,
            tool_call_id: Some("tool_123".to_string()),
            name: Some("get_weather".to_string()),
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (_, msgs) = convert_messages(&messages);
        assert_eq!(msgs[0]["role"], json!("user"));
        let content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], json!("tool_result"));
        assert_eq!(content[0]["tool_use_id"], json!("tool_123"));
    }

    #[test]
    fn tool_result_error() {
        let messages = vec![ChatMessage {
            role: MessageRole::Tool,
            content: "API error".to_string(),
            tool_calls: None,
            tool_call_id: Some("tool_456".to_string()),
            name: None,
            is_error: true,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (_, msgs) = convert_messages(&messages);
        let content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], json!("tool_result"));
        let inner = &content[0]["content"];
        assert_eq!(inner["text"], json!("API error"));
    }

    #[test]
    fn assistant_with_tool_calls() {
        let messages = vec![ChatMessage {
            role: MessageRole::Assistant,
            content: "".to_string(),
            tool_calls: Some(vec![ToolCall {
                id: "tool_1".to_string(),
                name: "search".to_string(),
                arguments: "{\"q\": \"rust\"}".to_string(),
            }]),
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (_, msgs) = convert_messages(&messages);
        let content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], json!("tool_use"));
        assert_eq!(content[0]["name"], json!("search"));
        assert_eq!(content[0]["input"], json!("{\"q\": \"rust\"}"));
    }

    #[test]
    fn thinking_signature_replayed() {
        let messages = vec![ChatMessage {
            role: MessageRole::Assistant,
            content: "result".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: Some("sig_abc".to_string()),
            content_blocks: None,
        }];
        let (_, msgs) = convert_messages(&messages);
        let content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], json!("thinking"));
        assert_eq!(content[0]["signature"], json!("sig_abc"));
        assert_eq!(content[1]["type"], json!("text"));
        assert_eq!(content[1]["text"], json!("result"));
    }

    #[test]
    fn multiple_system_messages_merged() {
        let messages = vec![
            make_message(MessageRole::System, "Rule 1"),
            make_message(MessageRole::System, "Rule 2"),
            make_message(MessageRole::User, "Hello"),
        ];
        let (system, msgs) = convert_messages(&messages);
        let sys = system.unwrap();
        assert!(sys.is_array());
        assert_eq!(sys.as_array().unwrap().len(), 2);
        assert_eq!(msgs.len(), 1);
    }
}