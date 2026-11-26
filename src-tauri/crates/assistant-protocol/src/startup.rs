use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 泳道：资源表的一列，同时是启动报告的归类维度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LaneDto {
    Boot,
    Idle,
    OnDemand,
}

/// 阶段结局。`Ignored` 表示失败不影响启动（已记录日志并进入降档集合）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum OutcomeDto {
    Ok,
    Degraded,
    Fatal,
    Ignored,
}

/// 能力状态。`Pending` 只出现在按需泳道未触发时。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CapStateDto {
    Pending,
    Ready,
    Failed,
}

/// 单个启动阶段的实测耗时。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhaseRecordDto {
    pub id: String,
    pub lane: LaneDto,
    pub ms: u64,
    pub budget_ms: u32,
    pub outcome: OutcomeDto,
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

/// 降档集合的一项：能力未就绪及其原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapIssueDto {
    pub id: String,
    pub message: String,
}

/// 首帧回传。`visible_ms` 与阶段耗时同域（都由前端 `frame_epoch_ms - t0_epoch_ms` 得到）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrameRecordDto {
    pub startup_id: String,
    pub frame_epoch_ms: i64,
    pub visible_ms: u64,
}

/// 启动报告：三个时延指标的载体（T_visible / T_boot / T_idle）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartupReportDto {
    pub startup_id: String,
    pub process_ms: u64,
    pub phases: Vec<PhaseRecordDto>,
    pub degraded: Vec<CapIssueDto>,
    pub first_frame: Option<FrameRecordDto>,
}
