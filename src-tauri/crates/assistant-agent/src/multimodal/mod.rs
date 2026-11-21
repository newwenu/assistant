mod admit;
mod blocks;
mod normalization;
mod pdf_text;
mod project;
mod rehydrate;
mod text;
mod validate;

pub use admit::admit_attachments;
pub use project::{
    offloaded_image_text, project_files, project_images_for_text_model, text_only_image_text,
};
pub use rehydrate::rehydrate_images;
pub use text::file_handle_text;
pub use validate::MultimodalConfig;

/// 附件限额的**唯一数值来源**：后端准入、前端展示、DTO 均从这里取。
/// 字段必须 `pub`——`src-tauri` 是另一个 crate，私有字段读不到。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentLimits {
    pub max_images: usize,
    pub max_files: usize,
    pub max_image_bytes: u64,
    pub max_file_bytes: u64,
}

pub const ATTACHMENT_LIMITS: AttachmentLimits = AttachmentLimits {
    max_images: 4,
    max_files: 4,
    max_image_bytes: 8 * 1024 * 1024,
    max_file_bytes: 16 * 1024 * 1024,
};

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
