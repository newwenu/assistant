use std::collections::HashMap;

use assistant_providers::base::{self, ChatMessage, MessageRole, ToolCall};
use assistant_storage::conversations::{ConversationDetail, MessageRow};

/// 跨进程中断留下的未配对调用，投影期合成的结果文本（D27）。
pub const INTERRUPTED_TOOL_RESULT: &str = "未执行：本轮中断";
/// 空结果体的投影占位；不落库，避免 Anthropic 把它替换成通用失败文案。
pub const EMPTY_TOOL_RESULT: &str = "(空结果)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentKind {
    #[default]
    Chat,
    Agent,
}

impl AgentKind {
    /// 唯一分支规则：`kind = agent` 走 agent 链路，其余（含旧值兼容标签）一律 chat。
    pub fn from_mode_kind(s: &str) -> Self {
        if s == "agent" {
            AgentKind::Agent
        } else {
            AgentKind::Chat
        }
    }

    pub fn supports_tool_call(&self) -> bool {
        matches!(self, AgentKind::Agent)
    }
}

pub fn prepare_messages(
    detail: &ConversationDetail,
    system_prompt: &str,
    append_user_prompt: Option<&str>,
) -> Vec<ChatMessage> {
    let rows: Vec<&MessageRow> = detail
        .messages
        .iter()
        .filter(|m| m.role != "compact_summary")
        .collect();
    let tool_names = collect_tool_names(&rows);
    let rebuilt = rebuild_rows(&rows, &tool_names);
    let mut chat_messages = repair_tool_pairing(rebuilt, &detail.conversation.id);

    if !system_prompt.is_empty() {
        chat_messages.insert(
            0,
            ChatMessage {
                role: MessageRole::System,
                content: system_prompt.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                is_error: false,
                thinking_signature: None,
                content_blocks: None,
            },
        );
    }

    if let Some(extra) = append_user_prompt {
        if !extra.is_empty() {
            chat_messages.push(ChatMessage {
                role: MessageRole::User,
                content: extra.to_string(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                is_error: false,
                thinking_signature: None,
                content_blocks: None,
            });
        }
    }

    chat_messages
}

fn parse_tool_calls(json: &str) -> Option<Vec<ToolCall>> {
    let calls: Vec<ToolCall> = serde_json::from_str(json).ok()?;
    if calls.is_empty() {
        None
    } else {
        Some(calls)
    }
}

fn collect_tool_names(rows: &[&MessageRow]) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for row in rows {
        if let Some(calls) = row.tool_calls_json.as_deref().and_then(parse_tool_calls) {
            for call in calls {
                names.entry(call.id).or_insert(call.name);
            }
        }
    }
    names
}

fn rebuild_rows(rows: &[&MessageRow], tool_names: &HashMap<String, String>) -> Vec<ChatMessage> {
    rows.iter()
        .map(|row| {
            let role = base::message_role_from_str(&row.role);
            let content = if role == MessageRole::Tool && row.content.is_empty() {
                EMPTY_TOOL_RESULT.to_string()
            } else {
                row.content.clone()
            };
            let tool_calls = row.tool_calls_json.as_deref().and_then(parse_tool_calls);
            let name = if role == MessageRole::Tool {
                row.tool_call_id
                    .as_ref()
                    .and_then(|id| tool_names.get(id).cloned())
            } else {
                None
            };
            ChatMessage {
                role,
                content,
                tool_calls,
                tool_call_id: row.tool_call_id.clone(),
                name,
                is_error: row.is_error,
                thinking_signature: None,
                content_blocks: None,
            }
        })
        .collect()
}

fn repair_tool_pairing(messages: Vec<ChatMessage>, conversation_id: &str) -> Vec<ChatMessage> {
    let mut out: Vec<ChatMessage> = Vec::with_capacity(messages.len());
    let mut pending: Vec<ToolCall> = Vec::new();

    for msg in messages {
        match msg.role {
            MessageRole::Assistant if msg.tool_calls.is_some() => {
                flush_pending(&mut out, &mut pending, conversation_id);
                pending = msg.tool_calls.clone().unwrap_or_default();
                out.push(msg);
            }
            MessageRole::Tool => {
                let call_id = msg.tool_call_id.clone();
                let matched = call_id
                    .as_ref()
                    .is_some_and(|id| pending.iter().any(|call| &call.id == id));
                if !matched {
                    log::warn!(
                        target: "agent::prompt_builder",
                        "会话 {} 的工具结果缺少父调用，投影期丢弃: tool_call_id = {:?}",
                        conversation_id,
                        call_id
                    );
                    continue;
                }
                pending.retain(|call| call_id.as_ref() != Some(&call.id));
                out.push(msg);
            }
            _ => {
                flush_pending(&mut out, &mut pending, conversation_id);
                out.push(msg);
            }
        }
    }
    flush_pending(&mut out, &mut pending, conversation_id);
    out
}

fn flush_pending(out: &mut Vec<ChatMessage>, pending: &mut Vec<ToolCall>, conversation_id: &str) {
    for call in pending.drain(..) {
        log::warn!(
            target: "agent::prompt_builder",
            "会话 {} 的调用 {} 缺少工具结果，投影期合成中断结果",
            conversation_id,
            call.id
        );
        out.push(ChatMessage {
            role: MessageRole::Tool,
            content: INTERRUPTED_TOOL_RESULT.to_string(),
            tool_calls: None,
            tool_call_id: Some(call.id),
            name: Some(call.name),
            is_error: true,
            thinking_signature: None,
            content_blocks: None,
        });
    }
}
