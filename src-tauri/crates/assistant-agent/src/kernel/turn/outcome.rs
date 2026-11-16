use serde::{Deserialize, Serialize};

use crate::kernel::stream::UsageSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnStatus {
    Completed,
    Cancelled,
    Failed,
    Interrupted,
}

impl TurnStatus {
    /// 日志与 wire 共用的稳定标签，避免 `Debug` 输出参与外部契约。
    pub fn as_str(&self) -> &'static str {
        match self {
            TurnStatus::Completed => "completed",
            TurnStatus::Cancelled => "cancelled",
            TurnStatus::Failed => "failed",
            TurnStatus::Interrupted => "interrupted",
        }
    }
}

/// 终态错误的具名出口集合；**不参与 wire**，只落在 `TurnOutcome.error` /
/// `RoundOutcome.error` 供 `TurnRunner` 判定与日志（前端只看 `status` 与 `text`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnError {
    /// 中间件返回 `StopTurn` 时结束本轮（`before_model` 的 `stopped` 出口）。
    MiddlewareStop {
        name: String,
        reason: String,
    },
    /// 单次模型调用超时或 turn 墙钟耗尽。
    Timeout,
    /// 空响应重试次数用尽。
    EmptyResponse,
    /// 无进展阶梯（D19 通用 2/3 阶梯 / D25 分类预算）耗尽。
    NoProgress {
        reason: String,
    },
    /// 终态文本落库失败。
    Persist(String),
    /// provider 错误；`String` 而非 provider 类型，守住依赖方向。
    Provider(String),
    Other(String),
}

/// `hit_cap` 与 `wrap_up_injected` 分开表达，不合并（design §2.3 / §3.6）：
/// 前者表示「还没总结」，后者表示「已经总结完」。
#[derive(Debug, Clone)]
pub struct TurnOutcome {
    pub status: TurnStatus,
    pub text: String,
    /// 推理内容，与正文一并落库（`messages.reasoning`）。
    pub reasoning: String,
    pub iterations: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    /// provider 上报的 total（`messages.tokens` 口径）；未上报时保持 `None`。
    pub total_tokens: Option<u32>,
    /// 每个带 usage 的 attempt 一条快照，终态落库时据此逐条写 `token_usage`。
    pub attempt_usages: Vec<UsageSnapshot>,
    /// 首个 delta 距请求发起的时间（毫秒）；全程无 delta 时为 `None`。
    pub ttft_ms: Option<u64>,
    /// 因迭代上限结束（最后一次响应仍在请求工具）。
    pub hit_cap: bool,
    /// 最后一次调用已注入收尾指令，`text` 即结论。
    pub wrap_up_injected: bool,
    /// 本 turn 最后一条被写入并 `finalize` 的 assistant 行 id（多迭代下每迭代一条）。
    /// 只有「从未写入任何 assistant 行」时为 `None`。
    pub final_message_id: Option<String>,
    pub error: Option<TurnError>,
}

impl TurnOutcome {
    pub fn new() -> Self {
        Self {
            status: TurnStatus::Completed,
            text: String::new(),
            reasoning: String::new(),
            iterations: 0,
            model_calls: 0,
            tool_calls: 0,
            prompt_tokens: None,
            completion_tokens: None,
            cache_read_tokens: None,
            total_tokens: None,
            attempt_usages: Vec::new(),
            ttft_ms: None,
            hit_cap: false,
            wrap_up_injected: false,
            final_message_id: None,
            error: None,
        }
    }
}

impl Default for TurnOutcome {
    fn default() -> Self {
        Self::new()
    }
}
