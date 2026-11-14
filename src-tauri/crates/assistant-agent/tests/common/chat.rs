//! provider wire 类型构造器：`ChatMessage` / `ChatDelta` / `ToolSpec`。
//!
//! 跨 ladder、middleware、kernel 与 loop 用例共用。

#![allow(dead_code)]

use assistant_providers::base::ChatDelta;
use assistant_providers::types::{ChatMessage, MessageRole, ToolSpec};

/// 纯文本消息：其余字段一律缺省。
pub fn text_msg(role: MessageRole, content: &str) -> ChatMessage {
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

/// 工具结果消息：在文本消息之上补 `name` 与 `tool_call_id`。
pub fn tool_msg(name: &str, id: &str, body: &str) -> ChatMessage {
    let mut msg = text_msg(MessageRole::Tool, body);
    msg.name = Some(name.to_string());
    msg.tool_call_id = Some(id.to_string());
    msg
}

/// 单段正文 delta，用于脚本化流式响应。
pub fn delta_text(text: &str) -> ChatDelta {
    ChatDelta {
        delta_text: Some(text.to_string()),
        ..Default::default()
    }
}

/// 工具声明：schema 由调用点给出，避免不同用例的入参契约被悄悄统一。
pub fn tool_spec(name: &str, description: &str, schema: serde_json::Value) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: schema,
    }
}

/// 工具调用片段：三个字段都可缺省，用于验证累加器的「片段补齐」行为。
pub fn delta_tool_call(id: Option<&str>, name: Option<&str>, args: Option<&str>) -> ChatDelta {
    ChatDelta {
        delta_tool_call_id: id.map(str::to_string),
        delta_tool_call_name: name.map(str::to_string),
        delta_tool_call_arguments: args.map(str::to_string),
        ..Default::default()
    }
}

/// 带序号的完整工具调用片段：脚本化「同批次多调用」的标准写法。
pub fn delta_tool_call_at(index: usize, id: &str, name: &str, args: &str) -> ChatDelta {
    ChatDelta {
        delta_tool_call_index: Some(index),
        ..delta_tool_call(Some(id), Some(name), Some(args))
    }
}

/// 仅带 `finish_reason` 的收尾片段。
///
/// 需要同时带 usage 时用 `ChatDelta { usage, ..delta_finish("stop") }` 叠加，
/// 避免各用例各自手写一遍 `Some(reason.to_string())`。
pub fn delta_finish(reason: &str) -> ChatDelta {
    ChatDelta {
        finish_reason: Some(reason.to_string()),
        ..Default::default()
    }
}
