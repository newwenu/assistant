use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 能力状态。`Pending` 只出现在按需泳道未触发时。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CapStateDto {
    Pending,
    Ready,
    Failed,
}

/// 能力状态快照：`startup:capability-changed` 广播载荷，同时是状态查询命令的返回项。
///
/// `message` 只在 `Failed` 时有值（可读原因），其余状态恒为 `null`：键恒在，
/// 前端无需区分「没有该字段」与「字段为空」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityStateDto {
    pub id: String,
    pub state: CapStateDto,
    pub message: Option<String>,
}
