use assistant_providers::types::{ContentBlock, FileAttachmentRef, ImageAttachmentRef};
use assistant_storage::attachment_store::AttachmentStore;
use base64::engine::general_purpose;
use base64::Engine;

use crate::error::AgentError;

use super::validate::{ext_of, file_media_type_from_ext, reject_file_path};
use super::MultimodalConfig;

pub async fn admit_attachments(
    content: &str,
    store: &AttachmentStore,
    config: &MultimodalConfig,
) -> Result<String, AgentError> {
    let mut blocks: Vec<ContentBlock> = serde_json::from_str(content)
        .map_err(|e| AgentError::other(format!("Invalid ContentBlock JSON: {}", e)))?;

    // 上限按整条消息的附件总数计算，已有 attachment_id 的附件同样占用配额，
    // 避免“历史附件 + 新附件”绕过限制。
    let image_count = blocks
        .iter()
        .filter(|b| matches!(b, ContentBlock::Image { .. }))
        .count();
    if image_count > 0 && config.max_images == 0 {
        return Err(AgentError::AttachmentsNotAllowed);
    }
    if image_count > config.max_images {
        return Err(AgentError::TooManyImages {
            max: config.max_images,
        });
    }

    let file_count = blocks
        .iter()
        .filter(|b| matches!(b, ContentBlock::File { .. }))
        .count();
    if file_count > 0 && config.max_files == 0 {
        return Err(AgentError::AttachmentsNotAllowed);
    }
    if file_count > config.max_files {
        return Err(AgentError::TooManyFiles {
            max: config.max_files,
        });
    }

    for block in blocks.iter_mut() {
        match block {
            ContentBlock::Image { .. } => admit_image(block, store, config).await?,
            ContentBlock::File { .. } => admit_file(block, store, config).await?,
            _ => {}
        }
    }

    serde_json::to_string(&blocks)
        .map_err(|e| AgentError::other(format!("Failed to serialize ContentBlocks: {}", e)))
}

async fn admit_image(
    block: &mut ContentBlock,
    store: &AttachmentStore,
    config: &MultimodalConfig,
) -> Result<(), AgentError> {
    let ContentBlock::Image {
        ref attachment,
        ref mut cached_data,
        ref mut file_path,
    } = block
    else {
        return Ok(());
    };

    if !attachment.attachment_id.is_empty() {
        return Ok(());
    }

    reject_file_path(file_path)?;

    let (data, mt): (Vec<u8>, String) = if let Some(data_b64) = cached_data.take() {
        let data = general_purpose::STANDARD
            .decode(&data_b64)
            .map_err(|e| AgentError::other(format!("Invalid base64 in ImageBlock: {}", e)))?;
        let mt = attachment
            .media_type
            .clone()
            .unwrap_or_else(|| "image/png".to_string());
        (data, mt)
    } else {
        return Err(AgentError::other(
            "ImageBlock needs cached_data (attachmentId empty)",
        ));
    };

    if !config.allowed_image_media_types.iter().any(|m| m == &mt) {
        return Err(AgentError::other(format!(
            "image media type not allowed: {}",
            mt
        )));
    }

    if data.len() > config.max_image_bytes {
        let max_mb = config.max_image_bytes / (1024 * 1024);
        return Err(AgentError::ImageTooLarge { max_mb });
    }

    let normalized = crate::normalization::normalize_image(&data, &mt)?;
    let width = if normalized.width > 0 {
        Some(normalized.width)
    } else {
        attachment.width
    };
    let height = if normalized.height > 0 {
        Some(normalized.height)
    } else {
        attachment.height
    };

    let ref_info = store
        .save_attachment(&normalized.data, &normalized.media_type)
        .await
        .map_err(|e| AgentError::other(e.to_string()))?;

    *block = ContentBlock::Image {
        attachment: ImageAttachmentRef {
            attachment_id: ref_info.id,
            media_type: Some(ref_info.media_type),
            bytes: Some(ref_info.bytes),
            width,
            height,
            name: attachment.name.clone(),
        },
        cached_data: None,
        file_path: None,
    };
    Ok(())
}

async fn admit_file(
    block: &mut ContentBlock,
    store: &AttachmentStore,
    config: &MultimodalConfig,
) -> Result<(), AgentError> {
    let ContentBlock::File {
        ref attachment,
        ref mut cached_data,
        ref mut file_path,
    } = block
    else {
        return Ok(());
    };

    if !attachment.attachment_id.is_empty() {
        return Ok(());
    }

    reject_file_path(file_path)?;

    let (data, mt, name): (Vec<u8>, String, String) = if let Some(data_b64) = cached_data.take() {
        let data = general_purpose::STANDARD
            .decode(&data_b64)
            .map_err(|e| AgentError::other(format!("Invalid base64 in FileBlock: {}", e)))?;
        // cached_data 路径同样必须过扩展名白名单，防止伪造 MIME 写入任意文件
        let ext = ext_of(&attachment.name);
        if !config.allowed_file_extensions.iter().any(|e| e == &ext) {
            return Err(AgentError::other(format!(
                "unsupported file extension: .{}",
                ext
            )));
        }
        let mt = file_media_type_from_ext(&ext)
            .unwrap_or("application/octet-stream")
            .to_string();
        (data, mt, attachment.name.clone())
    } else {
        return Err(AgentError::other(
            "FileBlock needs cached_data (attachmentId empty)",
        ));
    };

    if data.len() > config.max_file_bytes {
        let max_mb = config.max_file_bytes / (1024 * 1024);
        return Err(AgentError::FileTooLarge { max_mb });
    }

    let ref_info = store
        .save_attachment(&data, &mt)
        .await
        .map_err(|e| AgentError::other(e.to_string()))?;

    *block = ContentBlock::File {
        attachment: FileAttachmentRef {
            attachment_id: ref_info.id,
            name,
            bytes: ref_info.bytes,
        },
        cached_data: None,
        file_path: None,
    };
    Ok(())
}
