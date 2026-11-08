use std::time::Duration;

pub const DEFAULT_MAX_ITERATIONS: u32 = 12;
pub const DEFAULT_TURN_WALL_CLOCK: Duration = Duration::from_secs(30 * 60);
pub const DEFAULT_MODEL_CALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// 压缩链路：摘要生成单次调用 30s 硬上限（沿用旧口径，不引入配置面）。
pub const DEFAULT_COMPACTION_MODEL_CALL_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_EMPTY_RESPONSE_RETRIES: u32 = 1;
pub const DEFAULT_MICROCOMPACT_KEEP_RECENT: usize = 6;
pub const TOOL_CALLS_PER_ITERATION: u32 = 8;
pub const MIN_TOOL_CALL_LIMIT: u32 = 8;

/// `max_tool_calls` 由 `max_iterations` 派生，最小 8。
pub fn derived_tool_call_limit(max_iterations: u32) -> u32 {
    max_iterations
        .saturating_mul(TOOL_CALLS_PER_ITERATION)
        .max(MIN_TOOL_CALL_LIMIT)
}

/// 嵌套作用域取 `min`，只收紧不放宽。
pub fn effective_tool_call_limit(inner: u32, outer: Option<u32>) -> u32 {
    outer.map_or(inner, |o| inner.min(o))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnLimits {
    /// 迭代上限的权威值在 `TurnSpec::max_iterations`；此字段只用于派生 `max_tool_calls`。
    pub max_iterations: u32,
    pub max_tool_calls: u32,
    /// `None` 或 `Some(0)` 表示关闭。
    pub turn_wall_clock: Option<Duration>,
    /// `None` 或 `Some(0)` 表示关闭；每次 provider 重试重新计时。
    pub model_call: Option<Duration>,
    pub empty_response_retries: u32,
    /// `0` 表示关闭微压缩。
    pub microcompact_keep_recent: usize,
}

impl TurnLimits {
    /// chat 链路：无 turn 墙钟、无单调用上限（流超时由 provider 层 ttft/idle/total 负责），
    /// 空响应不重试、不做微压缩——与 chat 链路既有行为一致。
    pub fn chat() -> Self {
        Self {
            max_iterations: 1,
            max_tool_calls: 0,
            turn_wall_clock: None,
            model_call: None,
            empty_response_retries: 0,
            microcompact_keep_recent: 0,
        }
    }
}

impl Default for TurnLimits {
    fn default() -> Self {
        Self {
            max_iterations: DEFAULT_MAX_ITERATIONS,
            max_tool_calls: derived_tool_call_limit(DEFAULT_MAX_ITERATIONS),
            turn_wall_clock: Some(DEFAULT_TURN_WALL_CLOCK),
            model_call: Some(DEFAULT_MODEL_CALL_TIMEOUT),
            empty_response_retries: DEFAULT_EMPTY_RESPONSE_RETRIES,
            microcompact_keep_recent: DEFAULT_MICROCOMPACT_KEEP_RECENT,
        }
    }
}
