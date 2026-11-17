use assistant_providers::types::{ChatMessage, ContentBlock, MessageRole};
use assistant_storage::attachment_store::AttachmentStore;
use base64::engine::general_purpose;
use base64::Engine;

use super::blocks::{apply_blocks, blocks_of};

pub async fn rehydrate_images(messages: &mut [ChatMessage], store: &AttachmentStore) {
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(mut blocks) = blocks_of(msg) else {
            continue;
        };

        let mut changed = false;
        for block in blocks.iter_mut() {
            let ContentBlock::Image {
                ref attachment,
                ref mut cached_data,
                ..
            } = block
            else {
                continue;
            };

            let id = attachment.attachment_id.clone();
            match store.read(&id).await {
                Ok(data) => {
                    *cached_data = Some(general_purpose::STANDARD.encode(&data));
                    changed = true;
                }
                Err(e) => {
                    log::warn!("Failed to rehydrate image {}: {}", id, e);
                    *block = ContentBlock::Text {
                        text: "[Image not found]".to_string(),
                    };
                    changed = true;
                }
            }
        }

        if changed {
            apply_blocks(msg, blocks);
        }
    }
}
