use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 已解析的主题（封闭集合）：`system` 已在后端解析为具体值，前端不再兜底。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ResolvedThemeDto {
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeChangedPayloadDto {
    pub resolved: ResolvedThemeDto,
}
