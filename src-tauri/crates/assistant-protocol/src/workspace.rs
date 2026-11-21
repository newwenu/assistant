use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 目录簿条目（展示层缓存，不参与工作区解析，I38h §2.2）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDto {
    pub path: String,
    pub name: String,
    /// 目录当前是否已不存在：只读探测，不回写库（I38h §8 表）。
    pub missing: bool,
}
