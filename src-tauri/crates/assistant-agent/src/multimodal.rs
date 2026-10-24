use assistant_providers::types::{
    parse_content, ChatMessage, ContentBlock, ImageAttachmentRef, MessageRole,
};
use assistant_storage::attachment_store::AttachmentStore;
use base64::engine::general_purpose;
use base64::Engine;
use std::collections::HashMap;
use std::path::Path;

fn media_type_from_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

pub async fn admit_attachments(content: &str, store: &AttachmentStore) -> Result<String, String> {
    let mut blocks: Vec<ContentBlock> =
        serde_json::from_str(content).map_err(|e| format!("Invalid ContentBlock JSON: {}", e))?;

    let image_count = blocks
        .iter()
        .filter(|b| {
            matches!(
                b,
                ContentBlock::Image { attachment, .. } if attachment.attachment_id.is_empty()
            )
        })
        .count();
    if image_count > 4 {
        let mut args = HashMap::new();
        args.insert("max", "4");
        return Err(assistant_i18n::t_with_args("chat.too_many_images", &args));
    }

    for block in blocks.iter_mut() {
        let ContentBlock::Image {
            ref attachment,
            ref mut cached_data,
            ref mut file_path,
        } = block
        else {
            continue;
        };

        if !attachment.attachment_id.is_empty() {
            continue;
        }

        let (data, mt): (Vec<u8>, String) = if let Some(fp) = file_path.take() {
            let path = Path::new(&fp);
            if !path.exists() {
                return Err(format!("file not found: {}", fp));
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();
            let mt = media_type_from_ext(&ext)
                .map(|s| s.to_string())
                .ok_or_else(|| {
                    format!(
                        "unsupported image extension: .{} (allowed: png, jpg, jpeg, gif, webp)",
                        ext
                    )
                })?;
            let data = tokio::fs::read(&fp)
                .await
                .map_err(|e| format!("read failed: {}", e))?;
            (data, mt)
        } else if let Some(data_b64) = cached_data.take() {
            let data = general_purpose::STANDARD
                .decode(&data_b64)
                .map_err(|e| format!("Invalid base64 in ImageBlock: {}", e))?;
            let mt = attachment
                .media_type
                .clone()
                .unwrap_or_else(|| "image/png".to_string());
            (data, mt)
        } else {
            return Err("ImageBlock needs either file_path or cached_data".to_string());
        };

        if data.len() > 8 * 1024 * 1024 {
            let mut args = HashMap::new();
            args.insert("max_mb", "8");
            return Err(assistant_i18n::t_with_args("chat.image_too_large", &args));
        }

        let ref_info = store
            .save_image(&data, &mt)
            .await
            .map_err(|e| e.to_string())?;

        *block = ContentBlock::Image {
            attachment: ImageAttachmentRef {
                attachment_id: ref_info.id,
                media_type: Some(ref_info.media_type),
                bytes: Some(ref_info.bytes),
                width: attachment.width,
                height: attachment.height,
                name: attachment.name.clone(),
            },
            cached_data: None,
            file_path: None,
        };
    }

    serde_json::to_string(&blocks).map_err(|e| format!("Failed to serialize ContentBlocks: {}", e))
}

pub async fn rehydrate_images(messages: &mut [ChatMessage], store: &AttachmentStore) {
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(mut blocks) = parse_content(&msg.content) else {
            continue;
        };

        let mut has_image = false;
        let mut content_changed = false;
        for i in 0..blocks.len() {
            let ContentBlock::Image {
                ref attachment,
                ref mut cached_data,
                ..
            } = blocks[i]
            else {
                continue;
            };

            let id = attachment.attachment_id.clone();
            match store.read(&id).await {
                Ok(data) => {
                    *cached_data = Some(general_purpose::STANDARD.encode(&data));
                    has_image = true;
                }
                Err(e) => {
                    log::warn!("Failed to rehydrate image {}: {}", id, e);
                    blocks[i] = ContentBlock::Text {
                        text: assistant_i18n::t("chat.image_not_found_placeholder"),
                    };
                    content_changed = true;
                }
            }
        }

        if has_image || content_changed {
            msg.content_blocks = Some(blocks);
        }
    }
}

pub fn project_images_for_text_model(messages: &mut [ChatMessage]) {
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(blocks) = parse_content(&msg.content) else {
            continue;
        };

        let projected: Vec<ContentBlock> = blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::Image { .. } => ContentBlock::Text {
                    text: assistant_i18n::t("chat.image_projected_placeholder"),
                },
                other => other,
            })
            .collect();

        if let Ok(json) = serde_json::to_string(&projected) {
            msg.content = json;
        }
    }
}
