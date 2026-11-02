use assistant_providers::types::{parse_content, ChatMessage, ContentBlock, MessageRole};

use crate::multimodal;
use crate::token_estimate::estimate_text_tokens;

/// 单张图片的 token 估算（768×768 / 0.75 ≈ 765）。
const IMAGE_TOKEN_ESTIMATE: usize = 765;

/// 请求总量超预算时，把图片降级为占位文本，避免超出上下文窗口。
///
/// 仅作用于出站请求，不修改持久化消息。
pub fn offload_if_over_budget(messages: &mut [ChatMessage], context_limit: Option<u32>) {
    let Some(budget) = context_limit.map(|v| v as usize).filter(|&v| v > 0) else {
        return;
    };
    let mut total = estimate_total_tokens(messages);
    if total <= budget {
        return;
    }

    // 从最后一条用户消息开始逆序卸载，优先保留早期的完整图文上下文。
    for idx in (0..messages.len()).rev() {
        if messages[idx].role != MessageRole::User {
            continue;
        }
        let Some(blocks) = messages[idx]
            .content_blocks
            .take()
            .or_else(|| parse_content(&messages[idx].content))
        else {
            continue;
        };
        if !blocks
            .iter()
            .any(|b| matches!(b, ContentBlock::Image { .. }))
        {
            messages[idx].content_blocks = Some(blocks);
            continue;
        }

        // 用增量差值更新总量，避免每轮全量重新估算
        let mut delta: isize = 0;
        let projected: Vec<ContentBlock> = blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::Image { attachment, .. } => {
                    delta -= IMAGE_TOKEN_ESTIMATE as isize;
                    let text = multimodal::offloaded_image_text(&attachment.attachment_id);
                    delta += estimate_text_tokens(&text) as isize;
                    ContentBlock::Text { text }
                }
                other => other,
            })
            .collect();

        if let Ok(json) = serde_json::to_string(&projected) {
            messages[idx].content = json;
        }
        messages[idx].content_blocks = Some(projected);

        total = (total as isize + delta).max(0) as usize;
        if total <= budget {
            return;
        }
    }
}

/// 出站消息的 token 估算总和，与 offload 预算共用同一口径。
///
/// 供展示兜底（分支零记录时的占用投影）复用，调用方不得另写一套求和逻辑。
pub fn estimate_total_tokens(messages: &[ChatMessage]) -> usize {
    let mut total = 0;
    for msg in messages {
        if let Some(blocks) = &msg.content_blocks {
            total += estimate_blocks_tokens(blocks);
        } else if let Some(blocks) = parse_content(&msg.content) {
            total += estimate_blocks_tokens(&blocks);
        } else {
            total += estimate_text_tokens(&msg.content);
        }
    }
    total
}

fn estimate_blocks_tokens(blocks: &[ContentBlock]) -> usize {
    blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => estimate_text_tokens(text),
            ContentBlock::Image { .. } => IMAGE_TOKEN_ESTIMATE,
            ContentBlock::File { cached_data, .. } => match cached_data {
                Some(data) => estimate_text_tokens(data),
                None => 20,
            },
            ContentBlock::Thinking { thinking, .. } => estimate_text_tokens(thinking),
            ContentBlock::ToolUse { input, .. } => estimate_text_tokens(input),
            ContentBlock::ToolResult { output, .. } => estimate_text_tokens(output),
        })
        .sum()
}
