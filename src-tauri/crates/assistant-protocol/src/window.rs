use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 主窗口形态（封闭集合，运行时状态，不落盘）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum WindowModeDto {
    Normal,
    Mini,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowModeChangedPayloadDto {
    pub mode: WindowModeDto,
}
