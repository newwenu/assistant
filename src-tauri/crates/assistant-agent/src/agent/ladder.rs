//! 上下文阶梯 ③④⑤（I36 spec §4.3）。
//!
//! ② 摘要压缩随续跑式压缩同批落地（I38b D13），本模块不实现、不预留分支。
//! 阶梯顺序是语义：③ 在 ④ 之前（否则还原的是空内容），④ 之后不得触发 ⑤
//! （二者共用同一份 allowance）。

use assistant_providers::types::{ChatMessage, MessageRole};

use crate::compaction::COMPACTION_SAFETY_TOKENS;
use crate::request_projection::estimate_total_tokens;
use crate::token_estimate::estimate_text_tokens;

/// 微压缩占位符前缀；保留工具名与规模，让模型知道「这里发生过什么」。
pub const CLEARED_PLACEHOLDER_PREFIX: &str = "[tool result cleared: ";

/// 单次 ③ 清空的工具结果体，供 ④ 按额度还原。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearedBody {
    pub index: usize,
    pub tool_name: String,
    pub placeholder: String,
    pub original_content: String,
    pub original_tokens: usize,
    pub placeholder_tokens: usize,
}

/// 预算 = 窗口 - 输出预留 - 安全裕度；窗口未知或为 0 时返回 `None`（不可判定）。
pub fn budget_of(context_window_tokens: Option<u32>, max_tokens: u32) -> Option<usize> {
    context_window_tokens.filter(|&w| w > 0).map(|w| {
        (w as usize).saturating_sub(max_tokens as usize + COMPACTION_SAFETY_TOKENS as usize)
    })
}

/// ③ 微压缩：估算超预算时，把「较旧」的工具结果体清成占位符，保留最近 `keep_recent` 条。
///
/// 未超预算时不得执行；`keep_recent == 0` 表示关闭；窗口未知时不可判定，不执行。
/// 返回被清空的记录，供 ④ 在同一份 allowance 内还原。
pub fn microcompact(
    messages: &mut [ChatMessage],
    budget: Option<usize>,
    keep_recent: usize,
) -> Vec<ClearedBody> {
    let Some(budget) = budget else {
        return Vec::new();
    };
    if keep_recent == 0 || estimate_total_tokens(messages) <= budget {
        return Vec::new();
    }

    let tool_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == MessageRole::Tool && !m.content.trim().is_empty())
        .map(|(idx, _)| idx)
        .collect();
    if tool_indices.len() <= keep_recent {
        return Vec::new();
    }

    let older = &tool_indices[..tool_indices.len() - keep_recent];
    let mut cleared = Vec::with_capacity(older.len());
    for &idx in older {
        let original_content = messages[idx].content.clone();
        let original_tokens = estimate_text_tokens(&original_content);
        let tool_name = messages[idx]
            .name
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let placeholder = format!(
            "{}{}, {} tokens]",
            CLEARED_PLACEHOLDER_PREFIX, tool_name, original_tokens
        );
        let placeholder_tokens = estimate_text_tokens(&placeholder);
        messages[idx].content = placeholder.clone();
        messages[idx].content_blocks = None;
        cleared.push(ClearedBody {
            index: idx,
            tool_name,
            placeholder,
            original_content,
            original_tokens,
            placeholder_tokens,
        });
    }
    cleared
}

/// ④ 收尾还原：只作用于本次结论调用，最近清空的优先，超出 allowance 的仍保持占位符。
///
/// 返回实际还原的条数。调用方负责只在 `is_wrap_up_call` 时调用。
pub fn restore_tool_bodies(
    messages: &mut [ChatMessage],
    cleared: &[ClearedBody],
    allowance: usize,
) -> usize {
    let mut remaining = allowance;
    let mut restored = 0;
    for entry in cleared.iter().rev() {
        let cost = entry
            .original_tokens
            .saturating_sub(entry.placeholder_tokens);
        if cost > remaining {
            continue;
        }
        let Some(msg) = messages.get_mut(entry.index) else {
            continue;
        };
        // 只还原「仍是本次 ③ 写下的占位符」的消息，避免把旧记录写回已被其他阶梯改写的行。
        if msg.content != entry.placeholder {
            continue;
        }
        msg.content = entry.original_content.clone();
        msg.content_blocks = None;
        remaining -= cost;
        restored += 1;
    }
    restored
}

/// ④ 的额度从 ① 拆：内容清单 3/4，收尾还原 1/4，二者共用同一份 allowance。
///
/// 因此 ④ 之后不会突破 `budget`，也不会触发 ⑤。
pub fn split_input_allowance(budget: usize, estimated: usize) -> (usize, usize) {
    let allowance = budget.saturating_sub(estimated);
    let toc_allowance = allowance * 3 / 4;
    (toc_allowance, allowance - toc_allowance)
}

/// ⑤ 硬裁剪：整条淘汰最旧消息，`system` 消息与最后一条用户消息永不淘汰。
///
/// 淘汰 assistant 工具调用行时连同其配对 `tool` 行一起删除，避免留下孤立 tool 结果。
/// 返回被淘汰的消息条数。
pub fn hard_trim(messages: &mut Vec<ChatMessage>, budget: usize) -> usize {
    let mut evicted = 0;
    while estimate_total_tokens(messages) > budget {
        let Some(idx) = oldest_evictable(messages) else {
            break;
        };
        let end = span_end(messages, idx);
        messages.drain(idx..end);
        evicted += end - idx;
    }
    evicted
}

fn oldest_evictable(messages: &[ChatMessage]) -> Option<usize> {
    let last_user = messages.iter().rposition(|m| m.role == MessageRole::User);
    messages
        .iter()
        .enumerate()
        .position(|(idx, m)| m.role != MessageRole::System && Some(idx) != last_user)
}

/// 淘汰跨度：assistant 行的工具调用与其后连续的配对 `tool` 行必须一起删除。
fn span_end(messages: &[ChatMessage], idx: usize) -> usize {
    let call_ids: Vec<&str> = messages[idx]
        .tool_calls
        .as_ref()
        .map(|calls| calls.iter().map(|c| c.id.as_str()).collect())
        .unwrap_or_default();
    let mut end = idx + 1;
    while end < messages.len() {
        let msg = &messages[end];
        if msg.role != MessageRole::Tool {
            break;
        }
        match msg.tool_call_id.as_deref() {
            Some(id) if call_ids.contains(&id) => end += 1,
            _ => break,
        }
    }
    end
}
