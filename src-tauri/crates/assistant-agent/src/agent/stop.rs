use std::sync::Arc;

use crate::kernel::{StopDecision, StopHook, TurnState};

/// 迭代上限的冗余保险：主循环已在进入迭代前判过，钩子只防漏判。
pub struct IterationLimitHook;

impl StopHook for IterationLimitHook {
    fn name(&self) -> &'static str {
        "iteration-limit"
    }

    fn check(&self, state: &TurnState<'_>) -> StopDecision {
        if state.iteration > state.max_iterations {
            StopDecision::Stop {
                reason: "iteration-limit".to_string(),
            }
        } else {
            StopDecision::Continue
        }
    }
}

/// 工具调用数上限。按发起数计：被拒绝、失败、取消的调用同样占名额。
pub struct ToolCallLimitHook;

impl StopHook for ToolCallLimitHook {
    fn name(&self) -> &'static str {
        "tool-call-limit"
    }

    fn check(&self, state: &TurnState<'_>) -> StopDecision {
        if state.max_tool_calls > 0 && state.tool_calls >= state.max_tool_calls {
            StopDecision::Stop {
                reason: "tool-call-limit".to_string(),
            }
        } else {
            StopDecision::Continue
        }
    }
}

/// turn 墙钟上限。`turn_deadline` 为 0 表示不设限。
pub struct TurnWallClockHook;

impl StopHook for TurnWallClockHook {
    fn name(&self) -> &'static str {
        "turn-wall-clock"
    }

    fn check(&self, state: &TurnState<'_>) -> StopDecision {
        match state.turn_deadline {
            Some(limit) if !limit.is_zero() && state.elapsed >= limit => StopDecision::Stop {
                reason: "turn-wall-clock".to_string(),
            },
            _ => StopDecision::Continue,
        }
    }
}

/// 轮级 token 预算（附加维度）。未配置时不安装，不设默认值以免误杀。
pub struct TurnTokenBudgetHook {
    budget: u32,
}

impl TurnTokenBudgetHook {
    pub fn new(budget: u32) -> Self {
        Self { budget }
    }
}

impl StopHook for TurnTokenBudgetHook {
    fn name(&self) -> &'static str {
        "token-budget"
    }

    fn check(&self, state: &TurnState<'_>) -> StopDecision {
        if self.budget > 0
            && state.prompt_tokens.saturating_add(state.completion_tokens) >= self.budget
        {
            StopDecision::Stop {
                reason: "token-budget".to_string(),
            }
        } else {
            StopDecision::Continue
        }
    }
}

/// 按注册顺序调用全部钩子；返回 `None` 表示全部 `Continue`。
/// 首个 `Stop` 即短路，携带钩子名与原因供 Halt 收尾使用。
pub fn stop_hooks_all_continue(
    hooks: &[Arc<dyn StopHook>],
    state: &TurnState<'_>,
) -> Option<(&'static str, String)> {
    for hook in hooks {
        if let StopDecision::Stop { reason } = hook.check(state) {
            return Some((hook.name(), reason));
        }
    }
    None
}
