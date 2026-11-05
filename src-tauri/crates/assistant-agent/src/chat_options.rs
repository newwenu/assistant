use assistant_providers::base::ChatOptions;
use assistant_storage::agent_modes::AgentModeRow;

use crate::prompt_builder::AgentKind;

/// 内置档位值：不指定强度，交给 Provider 默认。前端与后端共认这一个字符串
pub const REASONING_LEVEL_DEFAULT: &str = "default";
/// 内置档位值：显式关闭
pub const REASONING_LEVEL_OFF: &str = "off";
/// 内置档位值：显式开启但不指定强度
pub const REASONING_LEVEL_ON: &str = "on";

/// 未配置 `max_tokens` 时的默认输出预留。
pub const DEFAULT_MAX_TOKENS: u32 = 4_096;

#[derive(Debug, Clone, Default)]
pub struct ReasoningOverride {
    /// 前端下发的档位值。后端不认 `inherit`；`None` 等价于 `default`
    pub level: Option<String>,
}

fn resolve_reasoning_level(level: Option<&str>) -> (Option<bool>, Option<String>) {
    let Some(level) = level else {
        return (None, None);
    };
    let level = level.trim();
    if level == REASONING_LEVEL_DEFAULT {
        return (None, None);
    }
    if level == REASONING_LEVEL_OFF {
        return (Some(false), None);
    }
    if level == REASONING_LEVEL_ON {
        return (Some(true), None);
    }
    (Some(true), Some(level.to_string()))
}

pub fn build_chat_options(
    kind: &AgentKind,
    mode: &AgentModeRow,
    context_window_tokens: Option<u32>,
    reasoning: &ReasoningOverride,
    tools: Option<Vec<assistant_providers::types::ToolSpec>>,
) -> ChatOptions {
    let temperature = match kind {
        AgentKind::Deep => mode.temperature.unwrap_or(0.3),
        _ => mode.temperature.unwrap_or(0.7),
    };

    let max_tokens = mode.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);

    let (reasoning_enabled, reasoning_effort) = resolve_reasoning_level(reasoning.level.as_deref());

    ChatOptions {
        temperature,
        max_tokens,
        stream: true,
        context_window_tokens,
        reasoning_enabled,
        reasoning_effort,
        tools,
    }
}
