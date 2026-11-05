use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::ToolUseContext;
use crate::result::ToolResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolTier {
    Sandbox,
    Os,
    External,
}

/// `validate_input` 的判定结果。
///
/// `blocks_input_rewrite = true` 表示该失败**不可通过改写输入绕过**：
/// 模型必须换一种做法，而不是换个路径/参数重试同一意图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub ok: bool,
    pub message: Option<String>,
    pub error_code: Option<String>,
    pub blocks_input_rewrite: bool,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            ok: true,
            message: None,
            error_code: None,
            blocks_input_rewrite: false,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: Some(message.into()),
            error_code: None,
            blocks_input_rewrite: false,
        }
    }

    pub fn blocked(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: Some(message.into()),
            error_code: None,
            blocks_input_rewrite: true,
        }
    }

    pub fn with_error_code(mut self, code: impl Into<String>) -> Self {
        self.error_code = Some(code.into());
        self
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BehaviorHints {
    pub read_only: bool,
    pub destructive: bool,
    pub open_world: bool,
}

/// 工具的暴露受众。两个维度独立：`agent` 决定是否进入 LLM 的 `tools` 列表，
/// `toolbox` 决定是否允许 ToolBox 桥接入口（`os_tool_exec`）按名调用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolAudience {
    pub agent: bool,
    pub toolbox: bool,
}

impl ToolAudience {
    pub const BOTH: Self = Self {
        agent: true,
        toolbox: true,
    };
    pub const TOOLBOX_ONLY: Self = Self {
        agent: false,
        toolbox: true,
    };
    pub const AGENT_ONLY: Self = Self {
        agent: true,
        toolbox: false,
    };
    pub const NONE: Self = Self {
        agent: false,
        toolbox: false,
    };
}

impl Default for ToolAudience {
    /// 默认不可见（fail-closed）：新增工具必须显式声明受众，
    /// 忘记声明时不会被任一入口暴露。
    fn default() -> Self {
        Self::NONE
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionIntent {
    pub action: String,
    pub resources: Vec<String>,
}

impl PermissionIntent {
    pub fn new(action: impl Into<String>, resource: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            resources: vec![resource.into()],
        }
    }
}

/// 工具执行结果的错误通道分两层：
/// - `ToolResult { ok: false }`：工具正常执行，业务失败（路径不存在、权限不足、参数缺失、未知 action），
///   结果回填给模型，模型可自行纠正。
/// - `Err(String)`：工具无法完成本次调用（内部缺陷、无法构造结果），
///   由 `exec::invoke` 归一化为 `ToolResult::err`，对上游不可见。
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;

    async fn description(&self) -> Result<String, String>;

    fn input_schema(&self) -> Value;

    fn tier(&self) -> ToolTier;

    /// 默认不可见（fail-closed）。新增工具必须显式声明受众：
    /// 面向模型用 `BOTH` / `AGENT_ONLY`，仅 ToolBox 用 `TOOLBOX_ONLY`。
    fn audience(&self) -> ToolAudience {
        ToolAudience::NONE
    }

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints::default()
    }

    fn is_readonly(&self) -> bool {
        self.behavior_hints().read_only
    }

    /// 纯检查、无 IO。Pipeline 的 validate 阶段调用，先于审批，
    /// 避免为非法输入弹审批框。路径检查必须复用 `os::path`。
    fn validate_input(&self, _input: &Value, _context: &ToolUseContext) -> ValidationResult {
        ValidationResult::ok()
    }

    /// 默认等于只读：只读工具可并发，写类工具默认串行。工具可按 input 覆写。
    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        self.is_readonly()
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        Ok(vec![])
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String>;
}
