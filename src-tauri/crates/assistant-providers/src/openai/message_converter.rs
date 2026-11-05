use crate::base::{ChatMessage, MessageRole};
use crate::types::{parse_content, ContentBlock, ToolSpec};
use serde_json::{json, Value};

/// 目标端点：两条端点共用 `convert_messages`，但原生附件 part 形状不同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenAiTarget {
    ChatCompletions,
    Responses,
}

pub(crate) fn role_str(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

fn text_part(target: OpenAiTarget, text: String) -> Value {
    match target {
        OpenAiTarget::ChatCompletions => json!({ "type": "text", "text": text }),
        OpenAiTarget::Responses => json!({ "type": "input_text", "text": text }),
    }
}

fn convert_content_block(block: &ContentBlock, target: OpenAiTarget) -> Option<Value> {
    match block {
        ContentBlock::Text { text } => Some(text_part(target, text.clone())),
        ContentBlock::Image {
            attachment,
            cached_data,
            ..
        } => {
            let mt = attachment.media_type.as_deref().unwrap_or("image/png");
            let Some(data) = cached_data.as_deref().filter(|d| !d.is_empty()) else {
                // 缺少 bytes 时降级为文本，避免发出 `data:...;base64,` 空 URI 触发 API 报错
                return Some(text_part(
                    target,
                    format!(
                        "[image unavailable: {}]",
                        attachment.name.as_deref().unwrap_or("unknown")
                    ),
                ));
            };
            let url = format!("data:{};base64,{}", mt, data);
            Some(match target {
                OpenAiTarget::ChatCompletions => json!({
                    "type": "image_url",
                    "image_url": { "url": url }
                }),
                OpenAiTarget::Responses => json!({
                    "type": "input_image",
                    "image_url": url
                }),
            })
        }
        ContentBlock::File {
            attachment,
            cached_data,
            ..
        } => {
            let is_pdf = attachment.name.to_ascii_lowercase().ends_with(".pdf");
            let data = cached_data.as_deref().filter(|d| !d.is_empty());
            match (target, is_pdf, data) {
                (OpenAiTarget::Responses, true, Some(data)) => Some(json!({
                    "type": "input_file",
                    "file_data": format!("data:application/pdf;base64,{}", data),
                    "filename": attachment.name,
                })),
                (_, true, Some(_)) => {
                    // Chat Completions 没有原生 PDF 字段；正常情况下 project_files
                    // 已按端点能力跳过 rehydrate，这里只兜底并明确告警。
                    log::warn!(
                        "[openai] PDF 端点不支持原生附件，已降级为占位文本: {}",
                        attachment.name
                    );
                    Some(text_part(target, attachment.handle_text()))
                }
                _ => Some(text_part(target, attachment.handle_text())),
            }
        }
        // Thinking / ToolUse / ToolResult 由 ChatMessage 的专用字段承载，
        // 不进入 content parts；返回 None 避免产出空对象。
        _ => None,
    }
}

fn build_content(m: &ChatMessage, target: OpenAiTarget) -> Value {
    if let Some(blocks) = &m.content_blocks {
        let parts: Vec<Value> = blocks
            .iter()
            .filter_map(|block| convert_content_block(block, target))
            .collect();
        if !parts.is_empty() {
            return json!(parts);
        }
    }
    if let Some(blocks) = parse_content(&m.content) {
        let has_media = blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Image { .. } | ContentBlock::File { .. }));
        if has_media {
            let parts: Vec<Value> = blocks
                .iter()
                .filter_map(|block| convert_content_block(block, target))
                .collect();
            if !parts.is_empty() {
                return json!(parts);
            }
        }
    }
    json!(m.content)
}

pub(crate) fn convert_tools(tools: &[ToolSpec]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema
                }
            })
        })
        .collect()
}

pub(crate) fn convert_messages(messages: &[ChatMessage], target: OpenAiTarget) -> Vec<Value> {
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
                    "content": build_content(m, target)
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::MessageRole;
    use crate::types::{FileAttachmentRef, ImageAttachmentRef};

    fn message(role: MessageRole, blocks: Vec<ContentBlock>) -> ChatMessage {
        ChatMessage {
            role,
            content: "{}".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: Some(blocks),
        }
    }

    fn text(text: &str) -> ContentBlock {
        ContentBlock::Text {
            text: text.to_string(),
        }
    }

    fn image() -> ContentBlock {
        ContentBlock::Image {
            attachment: ImageAttachmentRef {
                attachment_id: "a.png".to_string(),
                media_type: Some("image/png".to_string()),
                bytes: Some(1),
                width: Some(1),
                height: Some(1),
                name: Some("a.png".to_string()),
            },
            cached_data: Some("AAAA".to_string()),
            file_path: None,
        }
    }

    fn file(name: &str, cached: Option<&str>) -> ContentBlock {
        ContentBlock::File {
            attachment: FileAttachmentRef {
                attachment_id: "a.pdf".to_string(),
                name: name.to_string(),
                bytes: 1024,
            },
            cached_data: cached.map(str::to_string),
            file_path: None,
        }
    }

    #[test]
    fn chat_completions_text_and_image_shapes() {
        let msg = message(MessageRole::User, vec![text("hi"), image()]);
        let out = convert_messages(&[msg], OpenAiTarget::ChatCompletions);
        let parts = out[0]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], json!("text"));
        assert_eq!(parts[1]["type"], json!("image_url"));
        assert!(parts[1]["image_url"]["url"].is_string());
    }

    #[test]
    fn responses_text_and_image_shapes() {
        let msg = message(MessageRole::User, vec![text("hi"), image()]);
        let out = convert_messages(&[msg], OpenAiTarget::Responses);
        let parts = out[0]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], json!("input_text"));
        assert_eq!(parts[1]["type"], json!("input_image"));
        assert!(parts[1]["image_url"].is_string());
    }

    #[test]
    fn responses_pdf_becomes_input_file() {
        let msg = message(MessageRole::User, vec![file("a.pdf", Some("AAAA"))]);
        let out = convert_messages(&[msg], OpenAiTarget::Responses);
        let parts = out[0]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], json!("input_file"));
        assert!(parts[0]["file_data"]
            .as_str()
            .unwrap()
            .starts_with("data:application/pdf;base64,"));
        assert_eq!(parts[0]["filename"], json!("a.pdf"));
    }

    #[test]
    fn chat_completions_pdf_degrades_to_text() {
        let msg = message(MessageRole::User, vec![file("a.pdf", Some("AAAA"))]);
        let out = convert_messages(&[msg], OpenAiTarget::ChatCompletions);
        let parts = out[0]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], json!("text"));
        assert_eq!(parts[0]["text"], json!("[file: a.pdf (1.0 KB)]"));
    }

    #[test]
    fn pdf_without_cache_degrades_to_text() {
        let msg = message(MessageRole::User, vec![file("a.pdf", None)]);
        let out = convert_messages(&[msg], OpenAiTarget::Responses);
        let parts = out[0]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], json!("input_text"));
    }

    #[test]
    fn non_pdf_file_always_text() {
        let msg = message(MessageRole::User, vec![file("a.txt", Some("AAAA"))]);
        let out = convert_messages(&[msg], OpenAiTarget::Responses);
        let parts = out[0]["content"].as_array().unwrap();
        assert_eq!(parts[0]["type"], json!("input_text"));
    }

    #[test]
    fn unsupported_blocks_are_skipped_not_empty_objects() {
        let msg = message(
            MessageRole::User,
            vec![
                text("hi"),
                ContentBlock::Thinking {
                    thinking: "t".to_string(),
                    signature: None,
                },
            ],
        );
        let out = convert_messages(&[msg], OpenAiTarget::ChatCompletions);
        let parts = out[0]["content"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["type"], json!("text"));
    }

    #[test]
    fn second_round_tool_history_is_well_formed() {
        let assistant = ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            tool_calls: Some(vec![crate::base::ToolCall {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: r#"{"path":"a.txt"}"#.to_string(),
            }]),
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        };
        let tool_result = ChatMessage {
            role: MessageRole::Tool,
            content: "file body".to_string(),
            tool_calls: None,
            tool_call_id: Some("call_1".to_string()),
            name: Some("read_file".to_string()),
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        };
        let out = convert_messages(&[assistant, tool_result], OpenAiTarget::ChatCompletions);
        assert_eq!(out[0]["role"], json!("assistant"));
        assert_eq!(out[0]["tool_calls"][0]["id"], json!("call_1"));
        assert_eq!(out[0]["tool_calls"][0]["type"], json!("function"));
        assert_eq!(
            out[0]["tool_calls"][0]["function"]["arguments"],
            json!(r#"{"path":"a.txt"}"#)
        );
        assert_eq!(out[1]["role"], json!("tool"));
        assert_eq!(out[1]["tool_call_id"], json!("call_1"));
        assert_eq!(out[1]["content"], json!("file body"));
    }

    #[test]
    fn convert_tools_wraps_spec_into_function_shape() {
        let tools = vec![ToolSpec {
            name: "search".to_string(),
            description: "search the web".to_string(),
            input_schema: json!({ "type": "object", "properties": { "q": { "type": "string" } } }),
        }];
        let converted = convert_tools(&tools);
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0]["type"], json!("function"));
        assert_eq!(converted[0]["function"]["name"], json!("search"));
        assert_eq!(
            converted[0]["function"]["description"],
            json!("search the web")
        );
        assert_eq!(
            converted[0]["function"]["parameters"]["properties"]["q"]["type"],
            json!("string")
        );
        assert!(converted[0].get("name").is_none());
    }
}
