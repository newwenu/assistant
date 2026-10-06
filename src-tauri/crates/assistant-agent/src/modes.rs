// 参考: OpenBitFun/crates/execution/agent-runtime/src/agents.rs — Agent 模式定义

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AgentMode {
    Quick,
    Standard,
    Deep,
}

impl AgentMode {
    pub fn presentation_rank(self) -> u8 {
        match self {
            AgentMode::Quick => 0,
            AgentMode::Standard => 1,
            AgentMode::Deep => 2,
        }
    }

    pub fn system_prompt(self) -> &'static str {
        match self {
            AgentMode::Quick => "You are a helpful assistant. Be concise and direct.",
            AgentMode::Standard => "You are a helpful assistant. Provide thorough and well-structured answers.",
            AgentMode::Deep => "You are a helpful assistant. Think step by step, explore multiple perspectives, and provide comprehensive analysis.",
        }
    }

    pub fn default_temperature(self) -> f32 {
        match self {
            AgentMode::Quick => 0.3,
            AgentMode::Standard => 0.7,
            AgentMode::Deep => 0.9,
        }
    }

    pub fn default_max_tokens(self) -> u32 {
        match self {
            AgentMode::Quick => 1024,
            AgentMode::Standard => 4096,
            AgentMode::Deep => 8192,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModeProfile {
    pub mode: AgentMode,
    pub system_prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl ModeProfile {
    pub fn from_mode(mode: AgentMode) -> Self {
        Self {
            system_prompt: mode.system_prompt().to_string(),
            temperature: mode.default_temperature(),
            max_tokens: mode.default_max_tokens(),
            mode,
        }
    }
}
