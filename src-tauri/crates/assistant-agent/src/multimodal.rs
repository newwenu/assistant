use assistant_providers::types::{
    parse_content, ChatMessage, ContentBlock, FileAttachmentRef, ImageAttachmentRef, MessageRole,
};
use assistant_storage::attachment_store::AttachmentStore;
use base64::engine::general_purpose;
use base64::Engine;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;

use crate::error::AgentError;
use crate::token_estimate::{
    estimate_text_tokens, is_pdf_name, truncate_to_tokens, PDF_NATIVE_TOKENS,
};

const MAX_IMAGES: usize = 4;
const MAX_FILES: usize = 4;
const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;

/// 单文件最低配额：低于该值时不再截断正文，直接降级为占位文本。
const MIN_FILE_QUOTA_TOKENS: usize = 2_000;
/// 窗口未知时所有文件合计的投影上限（不是每文件上限）。
pub const MAX_UNKNOWN_WINDOW_FILE_TOKENS: usize = 32_000;
/// 头尾截断时头部占比。
const TRUNCATE_HEAD_PERCENT: u8 = 60;

/// 投影用途：发送路径允许记录「窗口未知」等告警，展示兜底路径不应刷日志。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionPurpose {
    Send,
    Estimate,
}

impl ProjectionPurpose {
    pub(crate) fn warns(self) -> bool {
        matches!(self, Self::Send)
    }
}

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
                "rs".into(),
                "toml".into(),
                "java".into(),
                "go".into(),
                "c".into(),
                "h".into(),
                "cpp".into(),
                "sh".into(),
                "ps1".into(),
                "sql".into(),
                "log".into(),
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
        "txt" | "log" => Some("text/plain"),
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
        "rs" => Some("text/x-rust"),
        "toml" => Some("text/x-toml"),
        "java" => Some("text/x-java"),
        "go" => Some("text/x-go"),
        "c" | "h" => Some("text/x-c"),
        "cpp" => Some("text/x-cpp"),
        "sh" => Some("text/x-shellscript"),
        "ps1" => Some("text/x-powershell"),
        "sql" => Some("application/sql"),
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

    if let Some(fp) = file_path.as_deref() {
        validate_path(fp, config).await?;
    }

    let (data, mt): (Vec<u8>, String) = if let Some(fp) = file_path.take() {
        let path = Path::new(&fp);
        if !path.exists() {
            return Err(AgentError::other(format!("file not found: {}", fp)));
        }
        let mt = media_type_from_ext(&ext_of(&fp))
            .map(|s| s.to_string())
            .ok_or_else(|| {
                AgentError::other(format!(
                    "unsupported image extension: .{} (allowed: png, jpg, jpeg, jfif, gif, webp, bmp, tif, tiff)",
                    ext_of(&fp)
                ))
            })?;
        let data = tokio::fs::read(&fp)
            .await
            .map_err(|e| AgentError::other(format!("read failed: {}", e)))?;
        (data, mt)
    } else if let Some(data_b64) = cached_data.take() {
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
            "ImageBlock needs either file_path or cached_data",
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

    if let Some(fp) = file_path.as_deref() {
        validate_path(fp, config).await?;
        let ext = ext_of(fp);
        if !config.allowed_file_extensions.iter().any(|e| e == &ext) {
            return Err(AgentError::other(format!(
                "unsupported file extension: .{}",
                ext
            )));
        }
    }

    let (data, mt, name): (Vec<u8>, String, String) = if let Some(fp) = file_path.take() {
        let path = Path::new(&fp);
        if !path.exists() {
            return Err(AgentError::other(format!("file not found: {}", fp)));
        }
        let mt = file_media_type_from_ext(&ext_of(&fp))
            .map(|s| s.to_string())
            .ok_or_else(|| {
                AgentError::other(format!("unsupported file extension: .{}", ext_of(&fp)))
            })?;
        let data = tokio::fs::read(&fp)
            .await
            .map_err(|e| AgentError::other(format!("read failed: {}", e)))?;
        let name = if attachment.name.is_empty() {
            file_name_from_path(&fp)
        } else {
            attachment.name.clone()
        };
        (data, mt, name)
    } else if let Some(data_b64) = cached_data.take() {
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
            "FileBlock needs either file_path or cached_data",
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

/// 文件投影：在预算内把文件正文转成文本 / 保留原生 PDF / 降级占位。
///
/// `file_budget_tokens = None` 表示窗口未知（不可判定），按所有文件合计上限投影；
/// `Some(0)` 表示没有文件预算，全部降级。预算由调用方按「输入预算 - 非文件估算」算出。
pub async fn project_files(
    messages: &mut [ChatMessage],
    store: &AttachmentStore,
    support_pdf: bool,
    file_budget_tokens: Option<usize>,
    purpose: ProjectionPurpose,
) {
    let mut occurrences: Vec<(usize, usize, FileAttachmentRef)> = Vec::new();
    for (mi, msg) in messages.iter().enumerate() {
        if msg.role != MessageRole::User {
            continue;
        }
        let Some(blocks) = blocks_of(msg) else {
            continue;
        };
        for (bi, block) in blocks.iter().enumerate() {
            if let ContentBlock::File { attachment, .. } = block {
                occurrences.push((mi, bi, attachment.clone()));
            }
        }
    }
    if occurrences.is_empty() {
        return;
    }

    let mut remaining = match file_budget_tokens {
        Some(v) => v,
        None => {
            if purpose.warns() {
                log::warn!(
                    "[attachment-budget] 窗口未知，按文件总额上限 {} tokens 投影",
                    MAX_UNKNOWN_WINDOW_FILE_TOKENS
                );
            }
            MAX_UNKNOWN_WINDOW_FILE_TOKENS
        }
    };
    // 单文件上限：只有一个文件时可用全部预算；两个及以上时每个最多一半，
    // 避免最新一个文件吞掉全部配额后其余文件直接降级。
    let per_file_cap = remaining / occurrences.len().clamp(1, 2);

    // 新消息优先：消息索引从大到小；同一消息内保持原顺序。
    let mut ordered = occurrences.clone();
    ordered.sort_by_key(|(mi, bi, _)| (Reverse(*mi), *bi));

    let mut projected: HashMap<(usize, usize), ContentBlock> = HashMap::new();
    for (mi, bi, att) in ordered {
        let quota = remaining.min(per_file_cap);
        let (block, used) = project_one_file(&att, store, support_pdf, quota, purpose).await;
        remaining = remaining.saturating_sub(used);
        projected.insert((mi, bi), block);
    }

    for (mi, bi, _) in occurrences {
        let Some(mut blocks) = blocks_of(&messages[mi]) else {
            continue;
        };
        if let Some(block) = projected.remove(&(mi, bi)) {
            if bi < blocks.len() {
                blocks[bi] = block;
            }
        }
        apply_blocks(&mut messages[mi], blocks);
    }
}

async fn project_one_file(
    att: &FileAttachmentRef,
    store: &AttachmentStore,
    support_pdf: bool,
    quota: usize,
    purpose: ProjectionPurpose,
) -> (ContentBlock, usize) {
    if quota == 0 {
        return (placeholder_block(att), placeholder_tokens(att));
    }

    if is_pdf_name(&att.name) {
        // 原生优先：预算足够时保留 PDF 原样发送；不足时回退侧车文本。
        if support_pdf && quota >= PDF_NATIVE_TOKENS {
            match store.read(&att.attachment_id).await {
                Ok(data) => {
                    return (
                        ContentBlock::File {
                            attachment: att.clone(),
                            cached_data: Some(general_purpose::STANDARD.encode(&data)),
                            file_path: None,
                        },
                        PDF_NATIVE_TOKENS,
                    );
                }
                Err(e) => {
                    log::warn!("Failed to rehydrate file {}: {}", att.attachment_id, e);
                    return (file_not_found_block(), 0);
                }
            }
        }

        if let Some(text) = crate::pdf_text::extract_or_load(store, att).await {
            return project_text_body(att, &text, quota);
        }

        if purpose.warns() {
            log::warn!(
                "[attachment-budget] PDF 无可用文本层，降级为占位文本: {}",
                att.name
            );
        }
        return (placeholder_block(att), placeholder_tokens(att));
    }

    if is_text_file(&att.name) {
        return match store.read(&att.attachment_id).await {
            Ok(data) => match project_text_file(att, &data, quota) {
                Ok((block, used)) => (block, used),
                Err(reason) => (fallback_block(att, &reason), placeholder_tokens(att)),
            },
            Err(e) => {
                log::warn!("Failed to read file {}: {}", att.attachment_id, e);
                (file_not_found_block(), 0)
            }
        };
    }

    (placeholder_block(att), placeholder_tokens(att))
}

fn project_text_file(
    att: &FileAttachmentRef,
    data: &[u8],
    quota: usize,
) -> Result<(ContentBlock, usize), String> {
    let text = decode_text(data).ok_or_else(|| "binary".to_string())?;
    let lang = lang_for(&att.name);
    let total = estimate_text_tokens(&text);

    if total <= quota {
        let block = ContentBlock::Text {
            text: render_text_file(&att.name, lang, &text, None),
        };
        let used = estimate_text_tokens(&block_text(&block));
        return Ok((block, used));
    }

    if quota < MIN_FILE_QUOTA_TOKENS {
        return Ok((placeholder_block(att), placeholder_tokens(att)));
    }

    let truncated = truncate_to_tokens(&text, quota, TRUNCATE_HEAD_PERCENT);
    let body = format!("{}\n...\n{}", truncated.head, truncated.tail);
    let header = format!(
        "[file: {} — truncated, kept head {} + tail {} tokens of {}]",
        att.name, truncated.head_tokens, truncated.tail_tokens, truncated.original_tokens
    );
    let block = ContentBlock::Text {
        text: render_text_file(&att.name, lang, &body, Some(header)),
    };
    let used = estimate_text_tokens(&block_text(&block));
    Ok((block, used))
}

fn project_text_body(att: &FileAttachmentRef, text: &str, quota: usize) -> (ContentBlock, usize) {
    let lang = lang_for(&att.name);
    let total = estimate_text_tokens(text);
    if total <= quota {
        let block = ContentBlock::Text {
            text: render_text_file(&att.name, lang, text, None),
        };
        let used = estimate_text_tokens(&block_text(&block));
        return (block, used);
    }
    if quota < MIN_FILE_QUOTA_TOKENS {
        return (placeholder_block(att), placeholder_tokens(att));
    }
    let truncated = truncate_to_tokens(text, quota, TRUNCATE_HEAD_PERCENT);
    let body = format!("{}\n...\n{}", truncated.head, truncated.tail);
    let header = format!(
        "[file: {} — truncated, kept head {} + tail {} tokens of {}]",
        att.name, truncated.head_tokens, truncated.tail_tokens, truncated.original_tokens
    );
    let block = ContentBlock::Text {
        text: render_text_file(&att.name, lang, &body, Some(header)),
    };
    let used = estimate_text_tokens(&block_text(&block));
    (block, used)
}

fn block_text(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text { text } => text.clone(),
        _ => String::new(),
    }
}

fn decode_text(data: &[u8]) -> Option<String> {
    let text = if let Ok(text) = std::str::from_utf8(data) {
        text.to_string()
    } else {
        let mut detector = chardetng::EncodingDetector::new();
        detector.feed(data, true);
        let encoding = detector.guess(None, true);
        let (text, _, _) = encoding.decode(data);
        text.into_owned()
    };

    // 扩展名白名单是唯一的「看起来像文本」信号；再用控制字符比例兜底，
    // 避免二进制文件改名 `.txt` 后被整段当成正文注入。
    let total = text.chars().count().max(1);
    let control = text
        .chars()
        .filter(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        .count();
    if control as f64 / total as f64 > 0.05 {
        return None;
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn has_fence(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim_start().starts_with("```"))
}

/// 文件正文渲染：原文已含围栏时不再外包一层，避免嵌套围栏；
/// 否则用代码围栏包住并标注语言。
fn render_text_file(name: &str, lang: &str, body: &str, header: Option<String>) -> String {
    let header = header.unwrap_or_else(|| format!("[file: {}]", name));
    if has_fence(body) {
        format!("{}\n{}", header, body)
    } else {
        format!("{}\n```{}\n{}\n```", header, lang, body)
    }
}

fn is_text_file(name: &str) -> bool {
    let ext = ext_of(name);
    matches!(
        ext.as_str(),
        "txt"
            | "md"
            | "csv"
            | "html"
            | "json"
            | "py"
            | "js"
            | "ts"
            | "tsx"
            | "css"
            | "yaml"
            | "yml"
            | "xml"
            | "rs"
            | "toml"
            | "java"
            | "go"
            | "c"
            | "h"
            | "cpp"
            | "sh"
            | "ps1"
            | "sql"
            | "log"
    )
}

fn lang_for(name: &str) -> &'static str {
    match ext_of(name).as_str() {
        "md" => "markdown",
        "csv" => "csv",
        "html" => "html",
        "json" => "json",
        "py" => "python",
        "js" => "javascript",
        "ts" | "tsx" => "typescript",
        "css" => "css",
        "yaml" | "yml" => "yaml",
        "xml" => "xml",
        "rs" => "rust",
        "toml" => "toml",
        "java" => "java",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" => "cpp",
        "sh" => "bash",
        "ps1" => "powershell",
        "sql" => "sql",
        _ => "",
    }
}

fn fallback_block(att: &FileAttachmentRef, reason: &str) -> ContentBlock {
    ContentBlock::Text {
        text: format!("[file: {} — {}]", att.handle_text(), reason),
    }
}

fn file_not_found_block() -> ContentBlock {
    ContentBlock::Text {
        text: "[File not found]".to_string(),
    }
}

fn placeholder_block(att: &FileAttachmentRef) -> ContentBlock {
    ContentBlock::Text {
        text: att.handle_text(),
    }
}

fn placeholder_tokens(att: &FileAttachmentRef) -> usize {
    estimate_text_tokens(&att.handle_text())
}

pub fn text_only_image_text() -> String {
    "[Image omitted — model does not support images]".to_string()
}

pub fn offloaded_image_text(attachment_id: &str) -> String {
    format!("[Image offloaded (ID: {})]", attachment_id)
}

pub fn file_handle_text(att: &FileAttachmentRef) -> String {
    att.handle_text()
}
