use assistant_providers::types::{
    parse_content, ChatMessage, ContentBlock, FileAttachmentRef, ImageAttachmentRef, MessageRole,
};
use assistant_storage::attachment_store::AttachmentStore;
use base64::engine::general_purpose;
use base64::Engine;
use std::collections::HashMap;
use std::path::Path;

const MAX_IMAGES: usize = 4;
const MAX_FILES: usize = 4;
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;

/// 附件准入策略：可信通道（用户主动输入）宽松，不可信通道（API / MCP 返回）严格。
#[derive(Debug, Clone)]
pub struct MultimodalConfig {
    pub max_images: usize,
    pub max_files: usize,
    pub max_image_bytes: usize,
    pub max_file_bytes: usize,
    pub allowed_image_media_types: Vec<String>,
    pub allowed_file_extensions: Vec<String>,
    /// 可信通道允许绝对路径（桌面选择器返回的路径）；
    /// 不可信通道必须禁止绝对路径与 `..`。
    pub allow_absolute_paths: bool,
}

impl MultimodalConfig {
    pub fn trusted_channel_input() -> Self {
        Self {
            max_images: MAX_IMAGES,
            max_files: MAX_FILES,
            max_image_bytes: MAX_IMAGE_BYTES,
            max_file_bytes: MAX_FILE_BYTES,
            allowed_image_media_types: vec![
                "image/png".into(),
                "image/jpeg".into(),
                "image/gif".into(),
                "image/webp".into(),
                "image/bmp".into(),
                "image/tiff".into(),
            ],
            allowed_file_extensions: vec![
                "pdf".into(),
                "txt".into(),
                "md".into(),
                "csv".into(),
                "html".into(),
                "json".into(),
                "py".into(),
                "js".into(),
                "ts".into(),
                "tsx".into(),
                "css".into(),
                "yaml".into(),
                "yml".into(),
                "xml".into(),
            ],
            allow_absolute_paths: true,
        }
    }

    pub fn untrusted_channel_input() -> Self {
        Self {
            max_images: 0,
            max_files: 0,
            max_image_bytes: 0,
            max_file_bytes: 0,
            allowed_image_media_types: Vec::new(),
            allowed_file_extensions: Vec::new(),
            allow_absolute_paths: false,
        }
    }
}

async fn validate_path(path: &str, config: &MultimodalConfig) -> Result<(), String> {
    if config.allow_absolute_paths {
        return Ok(());
    }
    if path.contains("..") {
        return Err("path traversal detected".to_string());
    }
    if Path::new(path).is_absolute() {
        return Err("absolute path not allowed".to_string());
    }
    let meta = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|e| format!("stat failed: {}", e))?;
    if meta.file_type().is_symlink() {
        return Err("symlink not allowed".to_string());
    }
    Ok(())
}

fn media_type_from_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" | "jfif" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}

fn file_media_type_from_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "pdf" => Some("application/pdf"),
        "txt" => Some("text/plain"),
        "md" => Some("text/markdown"),
        "csv" => Some("text/csv"),
        "html" => Some("text/html"),
        "json" => Some("application/json"),
        "py" => Some("text/x-python"),
        "js" => Some("text/javascript"),
        "ts" | "tsx" => Some("text/typescript"),
        "css" => Some("text/css"),
        "yaml" | "yml" => Some("text/yaml"),
        "xml" => Some("text/xml"),
        _ => None,
    }
}

fn ext_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

fn file_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

fn apply_blocks(msg: &mut ChatMessage, blocks: Vec<ContentBlock>) {
    if let Ok(json) = serde_json::to_string(&blocks) {
        msg.content = json;
    }
    msg.content_blocks = Some(blocks);
}

fn blocks_of(msg: &ChatMessage) -> Option<Vec<ContentBlock>> {
    msg.content_blocks
        .clone()
        .or_else(|| parse_content(&msg.content))
}

pub async fn admit_attachments(
    content: &str,
    store: &AttachmentStore,
    config: &MultimodalConfig,
) -> Result<String, String> {
    let mut blocks: Vec<ContentBlock> =
        serde_json::from_str(content).map_err(|e| format!("Invalid ContentBlock JSON: {}", e))?;

    // 上限按整条消息的附件总数计算，已有 attachment_id 的附件同样占用配额，
    // 避免“历史附件 + 新附件”绕过限制。
    let image_count = blocks
        .iter()
        .filter(|b| matches!(b, ContentBlock::Image { .. }))
        .count();
    if image_count > 0 && config.max_images == 0 {
        return Err(assistant_i18n::t("chat.attachments_not_allowed"));
    }
    if image_count > config.max_images {
        let max = config.max_images.to_string();
        let mut args = HashMap::new();
        args.insert("max", max.as_str());
        return Err(assistant_i18n::t_with_args("chat.too_many_images", &args));
    }

    let file_count = blocks
        .iter()
        .filter(|b| matches!(b, ContentBlock::File { .. }))
        .count();
    if file_count > 0 && config.max_files == 0 {
        return Err(assistant_i18n::t("chat.attachments_not_allowed"));
    }
    if file_count > config.max_files {
        let max = config.max_files.to_string();
        let mut args = HashMap::new();
        args.insert("max", max.as_str());
        return Err(assistant_i18n::t_with_args("chat.too_many_files", &args));
    }

    for block in blocks.iter_mut() {
        match block {
            ContentBlock::Image { .. } => admit_image(block, store, config).await?,
            ContentBlock::File { .. } => admit_file(block, store, config).await?,
            _ => {}
        }
    }

    serde_json::to_string(&blocks).map_err(|e| format!("Failed to serialize ContentBlocks: {}", e))
}

async fn admit_image(
    block: &mut ContentBlock,
    store: &AttachmentStore,
    config: &MultimodalConfig,
) -> Result<(), String> {
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

    if let Some(fp) = file_path.as_deref() {
        validate_path(fp, config).await?;
    }

    let (data, mt): (Vec<u8>, String) = if let Some(fp) = file_path.take() {
        let path = Path::new(&fp);
        if !path.exists() {
            return Err(format!("file not found: {}", fp));
        }
        let mt = media_type_from_ext(&ext_of(&fp))
            .map(|s| s.to_string())
            .ok_or_else(|| {
                format!(
                    "unsupported image extension: .{} (allowed: png, jpg, jpeg, jfif, gif, webp, bmp, tif, tiff)",
                    ext_of(&fp)
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

    if !config.allowed_image_media_types.iter().any(|m| m == &mt) {
        return Err(format!("image media type not allowed: {}", mt));
    }

    if data.len() > config.max_image_bytes {
        let max_mb = (config.max_image_bytes / (1024 * 1024)).to_string();
        let mut args = HashMap::new();
        args.insert("max_mb", max_mb.as_str());
        return Err(assistant_i18n::t_with_args("chat.image_too_large", &args));
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
        .map_err(|e| e.to_string())?;

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
) -> Result<(), String> {
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

    if let Some(fp) = file_path.as_deref() {
        validate_path(fp, config).await?;
        let ext = ext_of(&fp);
        if !config.allowed_file_extensions.iter().any(|e| e == &ext) {
            return Err(format!("unsupported file extension: .{}", ext));
        }
    }

    let (data, mt, name): (Vec<u8>, String, String) = if let Some(fp) = file_path.take() {
        let path = Path::new(&fp);
        if !path.exists() {
            return Err(format!("file not found: {}", fp));
        }
        let mt = file_media_type_from_ext(&ext_of(&fp))
            .map(|s| s.to_string())
            .ok_or_else(|| format!("unsupported file extension: .{}", ext_of(&fp)))?;
        let data = tokio::fs::read(&fp)
            .await
            .map_err(|e| format!("read failed: {}", e))?;
        let name = if attachment.name.is_empty() {
            file_name_from_path(&fp)
        } else {
            attachment.name.clone()
        };
        (data, mt, name)
    } else if let Some(data_b64) = cached_data.take() {
        let data = general_purpose::STANDARD
            .decode(&data_b64)
            .map_err(|e| format!("Invalid base64 in FileBlock: {}", e))?;
        // cached_data 路径同样必须过扩展名白名单，防止伪造 MIME 写入任意文件
        let ext = ext_of(&attachment.name);
        if !config.allowed_file_extensions.iter().any(|e| e == &ext) {
            return Err(format!("unsupported file extension: .{}", ext));
        }
        let mt = file_media_type_from_ext(&ext)
            .unwrap_or("application/octet-stream")
            .to_string();
        (data, mt, attachment.name.clone())
    } else {
        return Err("FileBlock needs either file_path or cached_data".to_string());
    };

    if data.len() > config.max_file_bytes {
        let max_mb = (config.max_file_bytes / (1024 * 1024)).to_string();
        let mut args = HashMap::new();
        args.insert("max_mb", max_mb.as_str());
        return Err(assistant_i18n::t_with_args("chat.file_too_large", &args));
    }

    let ref_info = store
        .save_attachment(&data, &mt)
        .await
        .map_err(|e| e.to_string())?;

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
                        text: assistant_i18n::t("chat.image_not_found_placeholder"),
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

/// 只为 PDF 填充 bytes：其余文件类型始终走 `file_handle_text` 文本投影，
/// 因为当前 Provider 均不支持把非 PDF 文件作为原生附件发送。
pub async fn rehydrate_files(
    messages: &mut [ChatMessage],
    store: &AttachmentStore,
    support_pdf: bool,
) {
    if !support_pdf {
        return;
    }
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(mut blocks) = blocks_of(msg) else {
            continue;
        };

        let mut changed = false;
        for block in blocks.iter_mut() {
            let ContentBlock::File {
                ref attachment,
                ref mut cached_data,
                ..
            } = block
            else {
                continue;
            };
            if !attachment.name.to_lowercase().ends_with(".pdf") {
                continue;
            }

            match store.read(&attachment.attachment_id).await {
                Ok(data) => {
                    *cached_data = Some(general_purpose::STANDARD.encode(&data));
                    changed = true;
                }
                Err(e) => {
                    log::warn!(
                        "Failed to rehydrate file {}: {}",
                        attachment.attachment_id,
                        e
                    );
                    *block = ContentBlock::Text {
                        text: assistant_i18n::t("chat.file_not_found_placeholder"),
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

pub fn project_images_for_text_model(messages: &mut [ChatMessage]) {
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(blocks) = blocks_of(msg) else {
            continue;
        };

        let projected: Vec<ContentBlock> = blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::Image { .. } => ContentBlock::Text {
                    text: text_only_image_text(),
                },
                other => other,
            })
            .collect();

        apply_blocks(msg, projected);
    }
}

pub fn project_files(messages: &mut [ChatMessage], support_pdf: bool) {
    for msg in messages.iter_mut() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(blocks) = blocks_of(msg) else {
            continue;
        };

        let projected: Vec<ContentBlock> = blocks
            .into_iter()
            .map(|block| match block {
                ContentBlock::File { ref attachment, .. }
                    if !should_send_file_as_is(attachment, support_pdf) =>
                {
                    ContentBlock::Text {
                        text: file_handle_text(attachment),
                    }
                }
                other => other,
            })
            .collect();

        apply_blocks(msg, projected);
    }
}

fn should_send_file_as_is(att: &FileAttachmentRef, support_pdf: bool) -> bool {
    support_pdf && att.name.to_lowercase().ends_with(".pdf")
}

pub fn text_only_image_text() -> String {
    assistant_i18n::t("chat.image_projected_placeholder")
}

pub fn offloaded_image_text(attachment_id: &str) -> String {
    let mut args = HashMap::new();
    args.insert("id", attachment_id);
    assistant_i18n::t_with_args("chat.image_offloaded_placeholder", &args)
}

pub fn file_handle_text(att: &FileAttachmentRef) -> String {
    att.handle_text()
}
