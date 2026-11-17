use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 覆盖层的引导数据；目标显示器、原图物理尺寸与内存原图都留在 Rust 侧，不下发前端。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenshotSessionDto {
    /// 一次性令牌：裁剪/取消只认它
    pub token: String,
    /// 底图预览：`data:image/jpeg;base64,...`，内存生成，按逻辑分辨率压缩
    pub preview: String,
    /// 预览的像素尺寸（仅用于尺寸标注与诊断，不作为换算依据）
    pub preview_width: u32,
    pub preview_height: u32,
}

/// 裁剪入参：归一化选区，相对底图显示区域（0..=1），前端无需知道 `scale_factor`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenshotCropArgsDto {
    pub token: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 裁剪终态：命令返回时字节已进 `AttachmentStore`，`attachment_id` 可直接进消息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenshotCropResultDto {
    pub attachment_id: String,
    pub file_name: String,
    pub media_type: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
}
