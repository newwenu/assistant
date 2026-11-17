use std::path::Path;

use crate::error::AgentError;

use super::ATTACHMENT_LIMITS;

/// 附件准入策略：可信通道（用户主动输入）宽松，不可信通道（API / MCP 返回）严格。
#[derive(Debug, Clone)]
pub struct MultimodalConfig {
    pub max_images: usize,
    pub max_files: usize,
    pub max_image_bytes: usize,
    pub max_file_bytes: usize,
    pub allowed_image_media_types: Vec<String>,
    pub allowed_file_extensions: Vec<String>,
}

impl MultimodalConfig {
    pub fn trusted_channel_input() -> Self {
        Self {
            max_images: ATTACHMENT_LIMITS.max_images,
            max_files: ATTACHMENT_LIMITS.max_files,
            max_image_bytes: ATTACHMENT_LIMITS.max_image_bytes as usize,
            max_file_bytes: ATTACHMENT_LIMITS.max_file_bytes as usize,
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
        }
    }
}

pub(super) fn reject_file_path(file_path: &Option<String>) -> Result<(), AgentError> {
    if file_path.is_some() {
        return Err(AgentError::other(
            "file_path is not accepted: attachments must be archived first (attachmentId required)",
        ));
    }
    Ok(())
}

pub(super) fn file_media_type_from_ext(ext: &str) -> Option<&'static str> {
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

pub(super) fn ext_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}
