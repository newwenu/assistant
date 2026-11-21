use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Millis;

/// 计时模式（封闭集合，线格式与前端联合一致）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ClockModeDto {
    Countdown,
    Stopwatch,
}

/// 时钟状态（封闭集合，线格式与前端联合一致）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ClockStatusDto {
    Idle,
    Running,
    Paused,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClockSnapshotDto {
    pub mode: ClockModeDto,
    pub status: ClockStatusDto,
    /// 倒计时配置时长；普通计时模式下保留上一次倒计时时长
    pub duration_ms: Millis,
    /// 仅倒计时有值；普通计时为 null
    pub remaining_ms: Option<Millis>,
    /// 两种模式都有值；倒计时下等于 duration_ms - remaining_ms
    pub elapsed_ms: Millis,
    /// 当前运行段的墙钟起点；非 Running 时为 null，前端据此本地续算
    pub started_at_ms: Option<Millis>,
    pub elapsed_before_ms: Millis,
}
