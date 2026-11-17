use assistant_providers::types::{parse_content, ChatMessage, ContentBlock};

pub(super) fn apply_blocks(msg: &mut ChatMessage, blocks: Vec<ContentBlock>) {
    if let Ok(json) = serde_json::to_string(&blocks) {
        msg.content = json;
    }
    msg.content_blocks = Some(blocks);
}

pub(super) fn blocks_of(msg: &ChatMessage) -> Option<Vec<ContentBlock>> {
    msg.content_blocks
        .clone()
        .or_else(|| parse_content(&msg.content))
}
