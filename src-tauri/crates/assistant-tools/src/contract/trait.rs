use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::ToolUseContext;
use crate::result::ToolResult;
use crate::sandbox::SandboxEnforcement;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolTier {
    Sandbox,
    Os,
    External,
}

impl ToolTier {
    /// 策略规则用 `tier:<tier>` 表达层级上限。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sandbox => "sandbox",
            Self::Os => "os",
            Self::External => "external",
        }
    }
}

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

/// `agent` 决定是否进入 LLM 的 `tools` 列表，`toolbox` 决定是否允许 ToolBox 桥接入口（`os_tool_exec`）按名调用。
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

/// `None` 表示「未知」，卡片不得渲染成 `0`。
///
/// 量纲由各工具自行表达，因此不设固定 `size` 字段。内部类型，不参与 wire。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalPreview {
    pub summary: String,
    pub truncated: bool,
    pub estimated: bool,
}

/// 工具执行结果的错误通道分两层：
/// - `ToolResult { ok: false }`：业务失败，结果回填给模型，模型可自行纠正；
/// - `Err(String)`：工具无法完成本次调用，由 `exec::invoke` 归一化为 `ToolResult::err`，对上游不可见。
///
/// 身份由实现自身持有，注册处只登记不覆盖：
/// `ToolRegistry::register` 以 `id()` 为键，并把 `name()` 登记为模型可见别名。
#[async_trait]
pub trait Tool: Send + Sync {
    fn id(&self) -> &str;

    /// 模型可见的调用名（`ToolSpec.name`）。
    fn name(&self) -> &str {
        self.id()
    }

    /// `full` 档描述：只在工具**已进入本轮 tools 数组**时下发（I58 §3.6）。
    ///
    /// 写「怎么用不出错」；选择指引属于 [`Tool::brief`]，写在这里会在调用时变成反向指令。
    async fn description(&self) -> Result<String, String>;

    /// `brief` 档描述：申请前的「一句用途 + 何时该申请」（I58 §3.6）。
    ///
    /// 默认 `None` 时回落到 [`Tool::description`] 的首句；覆写只写用途与时机，不重复 `full` 档内容。
    fn brief(&self) -> Option<&'static str> {
        None
    }

    /// 面向模型的手册正文（`request_tool` 的 `manual` 详情用它，I58 §3.1）。
    ///
    /// 默认 `None`：既有工具零改动，`request_tool` 回退到 [`Tool::description`]。
    /// 覆写时只写「怎么调、代价多大、边界在哪」，不重复 schema 已经表达的事实。
    fn manual(&self) -> Option<&'static str> {
        None
    }

    fn input_schema(&self) -> Value;

    fn tier(&self) -> ToolTier;

    /// 伴随工具：返回基工具的内部 id，跟随 `base` 获得**准入**（I58 §3.6）。
    ///
    /// 非 `None` 的工具在配置路径上一律被排除（`exclude_companions`），能否被调用只由运行时申请决定。
    fn companion_of(&self) -> Option<&str> {
        None
    }

    fn audience(&self) -> ToolAudience {
        ToolAudience::NONE
    }

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints::default()
    }

    fn is_readonly(&self) -> bool {
        self.behavior_hints().read_only
    }

    /// 本工具命令类操作的实际隔离强度；进审批键，避免「强边界下批准」的授权被弱边界执行复用。
    ///
    /// 默认 `None`：非沙箱工具的命令键形状不变（旧 `grant_key` 解出 `None` 即不复用）。
    /// `shell` 返回 `Some(..)`（只在探测 `>= partial` 时注册），故新旧键必然不相等。
    fn sandbox_enforcement(&self) -> Option<SandboxEnforcement> {
        None
    }

    /// 纯检查、无 IO。Pipeline 的 validate 阶段调用，先于审批，
    /// 避免为非法输入弹审批框。路径检查必须复用 `os::path`。
    fn validate_input(&self, _input: &Value, _context: &ToolUseContext) -> ValidationResult {
        ValidationResult::ok()
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        self.is_readonly()
    }

    /// 本工具单条结果的「内联上限」（字符数）；`None` = 用全局默认（I71 §4.3 W3.1）。
    ///
    /// 只抬**单结果**阈值，不改单轮预算：超阈值仍走落盘 + 预览，不新增截断语义。
    /// 覆写的理由是「我的结果天然该比 50k 大 / 小」，不是「我想让模型看到更多」。
    fn inline_output_limit(&self) -> Option<usize> {
        None
    }

    /// 是否自管执行超时（I38c §11 OQ5）。
    ///
    /// 返回 `true` 时 `exec::invoke` **不套**统一 deadline（`ask_user` 要等用户作答）；
    /// 绕过超时**不等于**不可中断，turn 子令牌的取消仍在外层生效。
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

    /// 仅在网关判定 `Ask` 之后、人工审批之前调用（`Allow` / `Deny` 不调用，避免多余 IO）。
    /// 可做只读 IO，2s 上限由调用方施加；不得写 grant、不得写 `ReadReceiptStore`、不得增删文件、
    /// 不得改变决策。预演与 `execute` 必须复用同一套路径解析与统计函数，禁止「展示 A、执行 B」。
    async fn preview_for_approval(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Option<ApprovalPreview> {
        None
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String>;
}
