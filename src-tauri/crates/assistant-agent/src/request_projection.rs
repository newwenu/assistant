use assistant_providers::types::{parse_content, ChatMessage, ContentBlock, MessageRole};

use crate::multimodal;

/// 单张图片的 token 估算（768×768 / 0.75 ≈ 765）。
const IMAGE_TOKEN_ESTIMATE: usize = 765;

/// 请求总量超预算时，把图片降级为占位文本，避免超出上下文窗口。
///
/// 仅作用于出站请求，不修改持久化消息。
pub fn offload_if_over_budget(messages: &mut [ChatMessage], context_limit: u32) {
    let budget = context_limit as usize;
    if budget == 0 {
        return;
    }
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
                    delta += (text.len() / 4) as isize;
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

fn estimate_total_tokens(messages: &[ChatMessage]) -> usize {
    let mut total = 0;
    for msg in messages {
        if let Some(blocks) = &msg.content_blocks {
            total += estimate_blocks_tokens(blocks);
        } else if let Some(blocks) = parse_content(&msg.content) {
            total += estimate_blocks_tokens(&blocks);
        } else {
            total += msg.content.len() / 4;
        }
    }
    total
}

fn estimate_blocks_tokens(blocks: &[ContentBlock]) -> usize {
    blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.len() / 4,
            ContentBlock::Image { .. } => IMAGE_TOKEN_ESTIMATE,
            ContentBlock::File { cached_data, .. } => match cached_data {
                Some(data) => data.len() / 4,
                None => 20,
            },
            ContentBlock::Thinking { thinking, .. } => thinking.len() / 4,
            ContentBlock::ToolUse { input, .. } => input.len() / 4,
            ContentBlock::ToolResult { output, .. } => output.len() / 4,
        })
        .sum()
}
