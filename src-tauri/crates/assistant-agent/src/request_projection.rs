use assistant_providers::types::{parse_content, ChatMessage, ContentBlock, MessageRole};

use crate::multimodal::{self, ProjectionPurpose};
use crate::token_estimate::{estimate_text_tokens, truncate_to_tokens};

/// 历史用户文本块被截断时至少保留的 token 数，避免把历史指令切没。
const MIN_HISTORY_TEXT_TOKENS: usize = 200;

fn blocks_of(msg: &ChatMessage) -> Option<Vec<ContentBlock>> {
    msg.content_blocks
        .clone()
        .or_else(|| parse_content(&msg.content))
}

fn apply_blocks(msg: &mut ChatMessage, blocks: Vec<ContentBlock>) {
    if let Ok(json) = serde_json::to_string(&blocks) {
        msg.content = json;
    }
    msg.content_blocks = Some(blocks);
}

/// 请求总量超预算时按固定优先级卸载：
/// 图片（最旧优先）→ Tool 内容 → 历史用户文本（最后手段）。
///
/// 文件正文由 [`crate::multimodal::project_files`] 按文件预算处理，本函数不再回读文件。
/// `budget = None` 表示窗口未知（不可判定），记录告警后原样返回；`Some(0)` 表示
/// 没有输入预算，必须尽量降级，二者不得混为一谈。
pub fn offload_if_over_budget(
    messages: &mut [ChatMessage],
    budget: Option<usize>,
    purpose: ProjectionPurpose,
) {
    let Some(budget) = budget else {
        if purpose.warns() {
            log::warn!("[offload] 上下文窗口未知，无法判定是否超预算，按不可判定处理");
        }
        return;
    };

    let mut total = estimate_total_tokens(messages);
    if total <= budget {
        return;
    }

    // 1) 图片：从最旧的用户消息开始卸载，保留最新一轮附件。
    for idx in 0..messages.len() {
        if total <= budget {
            return;
        }
        if messages[idx].role != MessageRole::User {
            continue;
        }
        let Some(mut blocks) = blocks_of(&messages[idx]) else {
            continue;
        };
        let mut changed = false;
        for block in blocks.iter_mut() {
            if let ContentBlock::Image { attachment, .. } = block {
                *block = ContentBlock::Text {
                    text: multimodal::offloaded_image_text(&attachment.attachment_id),
                };
                changed = true;
            }
        }
        if changed {
            apply_blocks(&mut messages[idx], blocks);
            total = estimate_total_tokens(messages);
        }
    }

    // 2) Tool 消息内容：截断或整体降级，保留 tool_call_id。
    for idx in 0..messages.len() {
        if total <= budget {
            return;
        }
        if messages[idx].role != MessageRole::Tool {
            continue;
        }
        let tokens = estimate_text_tokens(&messages[idx].content);
        if tokens == 0 {
            continue;
        }
        let excess = total - budget;
        if excess >= tokens {
            messages[idx].content = "[tool output offloaded]".to_string();
            messages[idx].content_blocks = None;
        } else {
            let keep = (tokens - excess).max(MIN_HISTORY_TEXT_TOKENS.min(tokens));
            let truncated = truncate_to_tokens(&messages[idx].content, keep, 60);
            messages[idx].content = format!("{}\n...\n{}", truncated.head, truncated.tail);
            messages[idx].content_blocks = None;
        }
        total = estimate_total_tokens(messages);
    }

    // 3) 历史用户文本：最后手段，且最后一条用户消息的指令永不整体降级。
    let last_user = messages.iter().rposition(|m| m.role == MessageRole::User);
    for idx in 0..messages.len() {
        if total <= budget {
            return;
        }
        if Some(idx) == last_user || messages[idx].role != MessageRole::User {
            continue;
        }
        let Some(mut blocks) = blocks_of(&messages[idx]) else {
            continue;
        };
        let mut remaining_excess = total - budget;
        let mut changed = false;
        for block in blocks.iter_mut() {
            if remaining_excess == 0 {
                break;
            }
            let ContentBlock::Text { text } = block else {
                continue;
            };
            // 文件正文已由 project_files 在文件预算内处理过；这里不再二次截断，
            // 否则会把「最近一轮附件」再削一遍。投影后的文件正文统一以 `[file: ` 开头。
            if text.starts_with("[file: ") {
                continue;
            }
            let tokens = estimate_text_tokens(text);
            if tokens <= MIN_HISTORY_TEXT_TOKENS {
                continue;
            }
            let cut = remaining_excess.min(tokens - MIN_HISTORY_TEXT_TOKENS);
            let keep = tokens - cut;
            let truncated = truncate_to_tokens(text, keep, 60);
            *text = format!("{}\n...\n{}", truncated.head, truncated.tail);
            remaining_excess = remaining_excess.saturating_sub(cut);
            changed = true;
        }
        if changed {
            apply_blocks(&mut messages[idx], blocks);
            total = estimate_total_tokens(messages);
        }
    }

    if total > budget && purpose.warns() {
        let over_ratio = total as f64 / budget.max(1) as f64;
        log::warn!(
            "[offload] 卸载后仍超预算: estimated={} budget={} over_ratio={:.2}",
            total,
            budget,
            over_ratio
        );
    }
}

/// 出站消息的 token 估算总和，与 offload 预算共用同一口径。
///
/// 供展示兜底（分支零记录时的占用投影）复用，调用方不得另写一套求和逻辑。
pub fn estimate_total_tokens(messages: &[ChatMessage]) -> usize {
    crate::token_estimate::estimate_messages_tokens(messages)
}
