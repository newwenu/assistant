use std::collections::HashMap;

use assistant_providers::base::{self, ChatMessage, MessageRole, ToolCall};
use assistant_storage::conversations::{ConversationDetail, MessageRow};

/// 跨进程中断留下的未配对调用，投影期合成的结果文本（D27）。
///
/// 单一定义在 storage，此处只转发，避免多处文案漂移。两条文案都不允许断言「未执行」的
/// 事实错误：`TOOL_NOT_STARTED_RESULT` 只用于占位行 `status = pending` 且正文为空（已声明、
/// 未进执行队列）的调用，其余一律走「结果未知」——中断时无法判断工具是否已产生副作用。
pub use assistant_storage::conversations::{INTERRUPTED_TOOL_RESULT, TOOL_NOT_STARTED_RESULT};
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
    ///
    /// 展示层剥离 Windows verbatim 前缀：工作区根目录按 `canonicalize` 原样存储，
    /// 直接下发 `\\?\G:\...` 时模型回填参数容易少抄一个反斜杠，`\?\G:\...` 会被
    /// 当成根相对路径与进程 cwd 拼接，拼出工作区外的错误路径（I44 §10.3）。
    pub fn render(&self) -> Option<String> {
        self.working_directory
            .as_deref()
            .filter(|path| !path.is_empty())
            .map(|path| format!("Working directory: {}", display_path(path)))
    }
}

/// 剥离 verbatim 前缀用于展示；非 verbatim 文本原样返回。
///
/// `\\?\UNC\server\share` 还原为 `\\server\share`，`\\?\C:\dir` 还原为 `C:\dir`。
fn display_path(raw: &str) -> String {
    const VERBATIM_UNC: &str = r"\\?\UNC\";
    const VERBATIM: &str = r"\\?\";
    if let Some(rest) = raw.strip_prefix(VERBATIM_UNC) {
        format!(r"\\{}", rest)
    } else if let Some(rest) = raw.strip_prefix(VERBATIM) {
        rest.to_string()
    } else {
        raw.to_string()
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
    // 配对判定只看「有没有结果行」：占位行本身就在链上，因此有占位行即视为已配对，
    // 不参与文案之外的逻辑。文案在 `rebuild_rows` 里按占位行状态决定。
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
            // 占位行的文案按阶段决定（D27）：`pending` = 已声明、未进执行队列，可重试；
            // `running` = 已进队列、结果未落，只能报结果未知。行在链上，配对即成立，
            // 因此这里补的是「这条结果说了什么」，而不是替调用方决定要不要重试。
            // `pending` 但正文非空 = 状态推进写失败时预写的保守文案，同样只能报结果未知。
            let in_flight_text = if role == MessageRole::Tool {
                match row.status.as_str() {
                    assistant_storage::conversations::TOOL_ROW_PENDING
                        if row.content.is_empty() =>
                    {
                        Some(TOOL_NOT_STARTED_RESULT)
                    }
                    assistant_storage::conversations::TOOL_ROW_PENDING
                    | assistant_storage::conversations::TOOL_ROW_RUNNING => {
                        Some(INTERRUPTED_TOOL_RESULT)
                    }
                    _ => None,
                }
            } else {
                None
            };
            let content = if let Some(text) = in_flight_text {
                text.to_string()
            } else if role == MessageRole::Tool && row.content.is_empty() {
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
                is_error: row.is_error || in_flight_text.is_some(),
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
        // D27 兜底路径：链上根本没有这条调用的结果行——存量会话（占位行机制上线前的
        // 数据），或声明时插行失败。此时无从判断是否已执行，只能保守报「结果未知」。
        // 走占位行的路径不会走到这里：占位行在链上，`rebuild_rows` 已按其状态补好文案。
        log::debug!(
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
