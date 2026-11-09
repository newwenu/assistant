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

/// 环境事实：随系统提示词下发、不落库、每轮重算（I38d §4.9.1）。
///
/// 取值由宿主侧给出（唯一来源 `tool_setup::workspace_root_from_conf()`），
/// 渲染格式归本 crate，保证发送路径与占用估算看到同一句话。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptFacts {
    /// 工作区根目录；`None` 表示未配置或配置非法。
    pub working_directory: Option<String>,
}

/// 无事实：chat 链路与未配置工作区共用，使「不注入」成为显式取值而非隐式缺省。
pub const NO_FACTS: PromptFacts = PromptFacts {
    working_directory: None,
};

impl PromptFacts {
    /// 渲染为系统提示词的一行；空值渲染为 `None`（整行不出现，不写占位符）。
    pub fn render(&self) -> Option<String> {
        self.working_directory
            .as_deref()
            .filter(|path| !path.is_empty())
            .map(|path| format!("Working directory: {path}"))
    }
}

/// 拼装最终系统提示词：`mode.prompt` 在前，事实行在后，`\n\n` 分隔。
///
/// 两者皆空时返回空串，`prepare_messages` 据此不插系统消息（与注入前逐字节相同）。
pub fn compose_system_prompt(system_prompt: &str, facts: &PromptFacts) -> String {
    let Some(fact) = facts.render() else {
        return system_prompt.to_string();
    };
    if system_prompt.is_empty() {
        fact
    } else {
        format!("{system_prompt}\n\n{fact}")
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
    let row_ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
    let mut chat_messages = repair_tool_pairing(rebuilt, &detail.conversation.id, &row_ids);

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

fn repair_tool_pairing(
    messages: Vec<ChatMessage>,
    conversation_id: &str,
    row_ids: &[&str],
) -> Vec<ChatMessage> {
    let mut out: Vec<ChatMessage> = Vec::with_capacity(messages.len());
    let mut pending: Vec<ToolCall> = Vec::new();
    let mut pending_owner: Option<String> = None;

    for (index, msg) in messages.into_iter().enumerate() {
        match msg.role {
            MessageRole::Assistant if msg.tool_calls.is_some() => {
                flush_pending(
                    &mut out,
                    &mut pending,
                    conversation_id,
                    pending_owner.as_deref(),
                );
                pending = msg.tool_calls.clone().unwrap_or_default();
                pending_owner = row_ids.get(index).map(|id| id.to_string());
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
                        "会话 {} 的工具结果缺少父调用，投影期丢弃: message_id = {:?}, tool_call_id = {:?}",
                        conversation_id,
                        row_ids.get(index),
                        call_id
                    );
                    continue;
                }
                pending.retain(|call| call_id.as_ref() != Some(&call.id));
                out.push(msg);
            }
            _ => {
                flush_pending(
                    &mut out,
                    &mut pending,
                    conversation_id,
                    pending_owner.as_deref(),
                );
                pending_owner = None;
                out.push(msg);
            }
        }
    }
    flush_pending(
        &mut out,
        &mut pending,
        conversation_id,
        pending_owner.as_deref(),
    );
    out
}

fn flush_pending(
    out: &mut Vec<ChatMessage>,
    pending: &mut Vec<ToolCall>,
    conversation_id: &str,
    owner_message_id: Option<&str>,
) {
    for call in pending.drain(..) {
        // D27 兜底路径：跨进程中断必然留下未配对的调用，投影期只读补一条中断结果让 wire 配平
        // （不写库，I38b §8）。行链修复后正常 turn 不会再漏写结果行，凡走到这里都意味着
        // 「中断」或「落库失败」，故用 warn 并带定位三要素，便于与真实缺陷区分。
        log::warn!(
            target: "agent::prompt_builder",
            "会话 {} 的消息 {:?} 的调用 {} 缺少工具结果，投影期合成中断结果",
            conversation_id,
            owner_message_id,
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
