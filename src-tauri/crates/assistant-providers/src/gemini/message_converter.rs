use crate::base::{ChatMessage, MessageRole};
use crate::types::{parse_content, ContentBlock, ToolSpec};
use serde_json::{json, Value};

pub(crate) fn convert_messages(messages: &[ChatMessage]) -> (Option<Value>, Vec<Value>) {
    let mut system_texts = Vec::new();
    let mut contents = Vec::new();

    for msg in messages {
        match msg.role {
            MessageRole::System => {
                if !msg.content.is_empty() {
                    system_texts.push(msg.content.clone());
                }
            }
            MessageRole::User => {
                let parts = convert_user_parts(msg);
                push_content(&mut contents, "user", parts);
            }
            MessageRole::Assistant => {
                let parts = convert_assistant_parts(msg);
                push_content(&mut contents, "model", parts);
            }
            MessageRole::Tool => {
                let parts = convert_tool_result_parts(msg);
                push_content(&mut contents, "user", parts);
            }
        }
    }

    let system_instruction = if system_texts.is_empty() {
        None
    } else {
        Some(json!({
            "parts": [{
                "text": system_texts.join("\n\n")
            }]
        }))
    };

    (system_instruction, contents)
}

fn convert_user_parts(msg: &ChatMessage) -> Vec<Value> {
    let blocks: Option<Vec<ContentBlock>> = msg
        .content_blocks
        .as_ref()
        .map(|b| Some(b.clone()))
        .unwrap_or_else(|| parse_content(&msg.content));

    if let Some(blocks) = blocks {
        let has_media = blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Image { .. } | ContentBlock::File { .. }));
        if has_media {
            return blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } if !text.is_empty() => {
                        Some(json!({ "text": text }))
                    }
                    ContentBlock::Image {
                        attachment,
                        cached_data,
                        ..
                    } => {
                        let mt = attachment.media_type.as_deref().unwrap_or("image/png");
                        let data = cached_data.as_deref().unwrap_or("");
                        if data.is_empty() {
                            // 缺少 bytes 时降级为文本，避免静默丢图
                            Some(json!({
                                "text": format!(
                                    "[image unavailable: {}]",
                                    attachment.name.as_deref().unwrap_or("unknown")
                                )
                            }))
                        } else {
                            Some(json!({
                                "inlineData": {
                                    "mimeType": mt,
                                    "data": data,
                                }
                            }))
                        }
                    }
                    ContentBlock::File {
                        attachment,
                        cached_data,
                        ..
                    } => {
                        let data = cached_data.as_deref().unwrap_or("");
                        if data.is_empty() || !attachment.name.to_lowercase().ends_with(".pdf") {
                            Some(json!({ "text": attachment.handle_text() }))
                        } else {
                            Some(json!({
                                "inlineData": {
                                    "mimeType": "application/pdf",
                                    "data": data,
                                }
                            }))
                        }
                    }
                    _ => None,
                })
                .collect();
        }
    }

    let mut parts = Vec::new();
    if !msg.content.is_empty() {
        parts.push(json!({ "text": msg.content }));
    }
    parts
}

fn convert_assistant_parts(msg: &ChatMessage) -> Vec<Value> {
    let mut parts = Vec::new();

    let has_tool_calls = msg
        .tool_calls
        .as_ref()
        .map(|tc| !tc.is_empty())
        .unwrap_or(false);

    let mut pending_signature = msg
        .thinking_signature
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .cloned();

    if !msg.content.is_empty() && !has_tool_calls {
        if let Some(sig) = pending_signature.take() {
            parts.push(json!({ "thoughtSignature": sig }));
        }
        parts.push(json!({ "text": msg.content }));
    } else if !msg.content.is_empty() {
        parts.push(json!({ "text": msg.content }));
    }

    if let Some(ref tool_calls) = msg.tool_calls {
        for (i, tc) in tool_calls.iter().enumerate() {
            let mut part = serde_json::Map::new();
            part.insert(
                "functionCall".to_string(),
                json!({
                    "name": tc.name,
                    "args": parse_function_args(&tc.arguments),
                }),
            );

            if i == 0 {
                if let Some(sig) = pending_signature.take() {
                    part.insert("thoughtSignature".to_string(), Value::String(sig));
                }
            }

            parts.push(Value::Object(part));
        }
    }

    if let Some(sig) = pending_signature {
        parts.push(json!({ "thoughtSignature": sig }));
    }

    parts
}

fn parse_function_args(arguments: &str) -> Value {
    if arguments.trim().is_empty() {
        return json!({});
    }
    match serde_json::from_str::<Value>(arguments) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => json!({}),
    }
}

fn convert_tool_result_parts(msg: &ChatMessage) -> Vec<Value> {
    let tool_name = msg.name.as_deref().unwrap_or("");
    if tool_name.is_empty() {
        return Vec::new();
    }

    let response = if msg.is_error {
        let error_text = if msg.content.is_empty() {
            "Tool execution failed"
        } else {
            &msg.content
        };
        json!({ "error": error_text })
    } else {
        parse_tool_response(&msg.content)
    };

    let parts = vec![json!({
        "functionResponse": {
            "name": tool_name,
            "response": response
        }
    })];

    parts
}

fn parse_tool_response(content: &str) -> Value {
    if content.trim().is_empty() {
        return json!({ "content": "Tool execution completed" });
    }

    match serde_json::from_str::<Value>(content) {
        Ok(Value::Object(map)) => Value::Object(map),
        Ok(value) => json!({ "content": value }),
        Err(_) => json!({ "content": content }),
    }
}

fn push_content(contents: &mut Vec<Value>, role: &str, parts: Vec<Value>) {
    if parts.is_empty() {
        return;
    }

    if let Some(last) = contents.last_mut() {
        let last_role = last.get("role").and_then(Value::as_str).unwrap_or_default();
        if last_role == role {
            if let Some(existing_parts) = last.get_mut("parts").and_then(Value::as_array_mut) {
                existing_parts.extend(parts);
                return;
            }
        }
    }

    contents.push(json!({
        "role": role,
        "parts": parts,
    }));
}

pub(crate) fn convert_tools(tool_defs: &[ToolSpec]) -> Option<Vec<Value>> {
    if tool_defs.is_empty() {
        return None;
    }

    let declarations: Vec<Value> = tool_defs
        .iter()
        .map(|tool| {
            let schema = sanitize_schema(&tool.input_schema);
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": schema,
            })
        })
        .collect();

    if declarations.is_empty() {
        None
    } else {
        Some(vec![json!({
            "functionDeclarations": declarations,
        })])
    }
}

fn sanitize_schema(parameters: &Value) -> Value {
    strip_unsupported_schema_fields(parameters.clone())
}

fn strip_unsupported_schema_fields(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            map.remove("allOf");
            map.remove("anyOf");
            map.remove("oneOf");

            let mut sanitized = serde_json::Map::new();
            for (key, val) in map {
                if key == "properties" {
                    if let Value::Object(properties) = val {
                        sanitized.insert(
                            key,
                            Value::Object(
                                properties
                                    .into_iter()
                                    .map(|(name, schema)| {
                                        (name, strip_unsupported_schema_fields(schema))
                                    })
                                    .collect(),
                            ),
                        );
                    }
                    continue;
                }
                if key == "items" {
                    sanitized.insert(key, strip_unsupported_schema_fields(val));
                    continue;
                }
                sanitized.insert(key, val);
            }
            Value::Object(sanitized)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(strip_unsupported_schema_fields)
                .collect(),
        ),
        other => other,
    }
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;
    use crate::base::ToolCall;

    #[test]
    fn convert_system_message() {
        let messages = vec![ChatMessage {
            role: MessageRole::System,
            content: "You are helpful".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (system, contents) = convert_messages(&messages);
        assert!(system.is_some());
        assert!(contents.is_empty());
        let sys = system.unwrap();
        assert_eq!(sys["parts"][0]["text"], "You are helpful");
    }

    #[test]
    fn convert_user_message() {
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (system, contents) = convert_messages(&messages);
        assert!(system.is_none());
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "Hello");
    }

    #[test]
    fn convert_assistant_role_maps_to_model() {
        let messages = vec![ChatMessage {
            role: MessageRole::Assistant,
            content: "Hi there".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (_, contents) = convert_messages(&messages);
        assert_eq!(contents[0]["role"], "model");
    }

    #[test]
    fn convert_tool_result_message() {
        let messages = vec![ChatMessage {
            role: MessageRole::Tool,
            content: r#"{"temp": 22}"#.to_string(),
            tool_calls: None,
            tool_call_id: Some("tc_1".to_string()),
            name: Some("get_weather".to_string()),
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (_, contents) = convert_messages(&messages);
        assert_eq!(contents[0]["role"], "user");
        let fr = &contents[0]["parts"][0]["functionResponse"];
        assert_eq!(fr["name"], "get_weather");
        assert_eq!(fr["response"]["temp"], 22);
    }

    #[test]
    fn convert_assistant_with_tool_calls() {
        let messages = vec![ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            tool_calls: Some(vec![ToolCall {
                id: "tc_1".to_string(),
                name: "search".to_string(),
                arguments: r#"{"query":"rust"}"#.to_string(),
            }]),
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (_, contents) = convert_messages(&messages);
        let fc = &contents[0]["parts"][0]["functionCall"];
        assert_eq!(fc["name"], "search");
        assert_eq!(fc["args"], json!({ "query": "rust" }));
    }

    #[test]
    fn convert_assistant_tool_call_with_invalid_args_falls_back_to_empty_object() {
        let messages = vec![ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            tool_calls: Some(vec![ToolCall {
                id: "tc_1".to_string(),
                name: "search".to_string(),
                arguments: "{not json".to_string(),
            }]),
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (_, contents) = convert_messages(&messages);
        assert_eq!(contents[0]["parts"][0]["functionCall"]["args"], json!({}));
    }

    #[test]
    fn second_round_tool_history_is_well_formed() {
        let messages = vec![
            ChatMessage {
                role: MessageRole::Assistant,
                content: String::new(),
                tool_calls: Some(vec![ToolCall {
                    id: "tc_1".to_string(),
                    name: "read_file".to_string(),
                    arguments: r#"{"path":"a.txt"}"#.to_string(),
                }]),
                tool_call_id: None,
                name: None,
                is_error: false,
                thinking_signature: None,
                content_blocks: None,
            },
            ChatMessage {
                role: MessageRole::Tool,
                content: r#"{"content":"file body"}"#.to_string(),
                tool_calls: None,
                tool_call_id: Some("tc_1".to_string()),
                name: Some("read_file".to_string()),
                is_error: false,
                thinking_signature: None,
                content_blocks: None,
            },
        ];
        let (_, contents) = convert_messages(&messages);
        assert_eq!(contents[0]["role"], "model");
        assert_eq!(contents[0]["parts"][0]["functionCall"]["name"], "read_file");
        assert_eq!(
            contents[0]["parts"][0]["functionCall"]["args"],
            json!({ "path": "a.txt" })
        );
        assert_eq!(contents[1]["role"], "user");
        assert_eq!(
            contents[1]["parts"][0]["functionResponse"]["name"],
            "read_file"
        );
        assert_eq!(
            contents[1]["parts"][0]["functionResponse"]["response"]["content"],
            "file body"
        );
    }

    #[test]
    fn convert_tools_uses_function_declarations_with_sanitized_schema() {
        let tools = vec![ToolSpec {
            name: "search".to_string(),
            description: "search the web".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "q": { "type": "string" } },
                "allOf": [{ "type": "object" }]
            }),
        }];
        let converted = convert_tools(&tools).expect("non-empty tools");
        assert_eq!(converted.len(), 1);
        let declarations = &converted[0]["functionDeclarations"];
        assert_eq!(declarations[0]["name"], "search");
        assert_eq!(declarations[0]["description"], "search the web");
        assert_eq!(
            declarations[0]["parameters"]["properties"]["q"]["type"],
            "string"
        );
        assert!(declarations[0]["parameters"].get("allOf").is_none());
    }

    #[test]
    fn convert_tools_returns_none_for_empty_input() {
        assert!(convert_tools(&[]).is_none());
    }

    #[test]
    fn convert_thinking_signature() {
        let messages = vec![ChatMessage {
            role: MessageRole::Assistant,
            content: "answer".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            is_error: false,
            thinking_signature: Some("sig_abc".to_string()),
            content_blocks: None,
        }];
        let (_, contents) = convert_messages(&messages);
        let parts = contents[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["thoughtSignature"], "sig_abc");
        assert_eq!(parts[1]["text"], "answer");
    }

    #[test]
    fn merge_consecutive_same_role() {
        let messages = vec![
            ChatMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                is_error: false,
                thinking_signature: None,
                content_blocks: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: "World".to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                is_error: false,
                thinking_signature: None,
                content_blocks: None,
            },
        ];
        let (_, contents) = convert_messages(&messages);
        assert_eq!(contents.len(), 1);
        let parts = contents[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn tool_error_response() {
        let messages = vec![ChatMessage {
            role: MessageRole::Tool,
            content: "timeout".to_string(),
            tool_calls: None,
            tool_call_id: Some("tc_1".to_string()),
            name: Some("search".to_string()),
            is_error: true,
            thinking_signature: None,
            content_blocks: None,
        }];
        let (_, contents) = convert_messages(&messages);
        let fr = &contents[0]["parts"][0]["functionResponse"];
        assert_eq!(fr["response"]["error"], "timeout");
    }
}
