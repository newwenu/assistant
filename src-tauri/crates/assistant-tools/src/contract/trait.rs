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

impl ToolTier {
    /// 与 serde 同形的小写标识；策略规则用 `tier:<tier>` 表达层级上限。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sandbox => "sandbox",
            Self::Os => "os",
            Self::External => "external",
        }
    }
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

/// 审批卡片上的一句话量级摘要。
///
/// `preview_for_approval` 返回 `None` 表示「未知」，与「零」可区分：
/// 卡片不得把 `None` 渲染成 `0`。量纲由各工具自行表达（条目数 / 字节数 / 行数 / 替换次数），
/// 因此这里不设固定的 `size` 字段。
///
/// 内部类型，不参与 wire，因此不 derive `Serialize`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalPreview {
    /// 后端已格式化的一句话，如「删除 128 个条目（3.4 MB）」。
    pub summary: String,
    /// 已达扫描上限，只能给出下限。
    pub truncated: bool,
    /// 按参数估算，执行前会变（TOCTOU）。
    pub estimated: bool,
}

/// 工具执行结果的错误通道分两层：
/// - `ToolResult { ok: false }`：工具正常执行，业务失败（路径不存在、权限不足、参数缺失、未知 action），
///   结果回填给模型，模型可自行纠正。
/// - `Err(String)`：工具无法完成本次调用（内部缺陷、无法构造结果），
///   由 `exec::invoke` 归一化为 `ToolResult::err`，对上游不可见。
/// 工具的行为契约与身份声明。身份由实现自身持有，注册处只登记不覆盖：
/// `ToolRegistry::register` 以 `id()` 为键，并把 `name()` 登记为模型可见别名。
#[async_trait]
pub trait Tool: Send + Sync {
    /// 内部唯一标识：注册表键、白名单匹配、审批与审计、报错前缀。
    fn id(&self) -> &str;

    /// 模型可见的调用名（`ToolSpec.name`）。默认与 [`Tool::id`] 同名；
    /// 需要内部名与对外调用名分离时才覆写。
    fn name(&self) -> &str {
        self.id()
    }

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

    /// 是否自管执行超时（I38c §11 OQ5）。
    ///
    /// 返回 `true` 时 `exec::invoke` **不套**统一 deadline，由工具自身的等待收口
    /// （`ask_user` 要等用户作答，30s 统一超时会把它当成失败）。
    /// 绕过统一超时**不等于**不可中断：turn 子令牌的取消在 `exec::invoke` 外层生效。
    fn manages_own_execution_timeout(&self) -> bool {
        false
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        Ok(vec![])
    }

    /// 审批预演：审批卡片展示「这次操作有多大」。
    ///
    /// 调用时机：**仅**网关判定 `Ask` 之后、人工审批之前；`Allow` / `Deny` 路径不调用，
    /// 避免给全量工具调用增加 IO。默认 `None`（不提供预演），存量工具零改动。
    ///
    /// 约束：可做只读 IO；2s 上限由调用方施加，失败 / 超时即 `None`；
    /// 不得写 grant、不得写 `ReadReceiptStore`、不得创建或删除任何文件、不得改变决策。
    /// 预演与 `execute` 必须复用同一套路径解析与统计函数，禁止「展示 A、执行 B」。
    async fn preview_for_approval(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Option<ApprovalPreview> {
        None
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String>;
}
