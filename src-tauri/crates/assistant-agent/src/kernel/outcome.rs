use serde::{Deserialize, Serialize};

use super::stream::UsageSnapshot;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnError {
    /// 中间件返回 `StopTurn` 时结束本轮（Step 4 预留）。
    MiddlewareStop {
        name: String,
        reason: String,
    },
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
            error: None,
        }
    }
}

impl Default for TurnOutcome {
    fn default() -> Self {
        Self::new()
    }
}
