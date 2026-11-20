//! `shell` 工具：唯一命令执行入口（I56 §1）。
//!
//! 模块形态固定（I56 §12.1）：`schema` 输入契约、`classify` 命令分类、
//! `spawn` 执行计划推导、`run` 执行编排、`deny` 拒绝判定。
//!
//! 本文件只做协议适配：身份声明与 `Tool` trait 转发，不写任何 IO 细节。

pub mod classify;
pub mod deny;
pub mod run;
pub mod schema;
pub mod spawn;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::context::ToolUseContext;
use crate::r#trait::{
    ApprovalPreview, BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier,
    ValidationResult,
};
use crate::result::ToolResult;
use crate::sandbox::{SandboxEnforcement, SandboxProbe};
use crate::storage::ToolResultStorage;

use classify::CommandClass;
use schema::ShellInput;

/// 内部标识与模型可见调用名同源：白名单、审计、报错前缀都用这一个串。
const TOOL_ID: &str = "shell";

/// `shell` 工具的身份声明与协议适配。
///
/// 持有注册期的探测结论引用：运行期**不再探测**（A-S8），否则同一会话前后会出现两套边界。
pub struct ShellTool {
    probe: &'static SandboxProbe,
    storage: Arc<dyn ToolResultStorage>,
}

impl ShellTool {
    /// 构造期再断言一次门禁：`enforcement == None` 时不存在合法实例。
    ///
    /// 门禁在注册处（`register_shell_tools`），这里只是防止绕过注册处直接构造，
    /// 让「无沙箱执行」在类型层面也不可能出现。
    pub fn new(probe: &'static SandboxProbe, storage: Arc<dyn ToolResultStorage>) -> Self {
        assert_ne!(
            probe.enforcement,
            SandboxEnforcement::None,
            "ShellTool requires a sandbox boundary; register it only when enforcement != none"
        );
        Self { probe, storage }
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok(
            "Run one command in a sandboxed shell and return its exit code plus a path to the \
             captured output. A non-zero exit code is NOT a tool failure: read `stderr` (or the \
             output file) and fix the command. There is no stdin and no PTY, so interactive \
             commands (`git commit` without `-m`, `vim`, `npm login`, a first `ssh` connection) \
             will hang until the timeout. On Windows the shell is `cmd` (Unix: `sh`): \
             `Get-ChildItem`, `$env:X` and `$VAR` do not work; single quotes are not quotes, `;` \
             is not a separator (use `&`), and `\\` is not an escape character — check \
             `data.sandbox.shell` for the shell actually used. `cwd` defaults to the workspace \
             root and must stay inside it. Output is spooled to a file: when the returned text \
             is truncated, read `outputPath` with `read` instead of re-running the command. \
             Raise `timeout_ms` explicitly for long builds (default 120000 ms, max 1800000); on \
             timeout the whole process tree is killed and the output written so far is kept."
                .to_string(),
        )
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command to execute; passed to the shell as a single script string"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory; relative paths resolve against the workspace root, and the result must stay inside it. Defaults to the workspace root"
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": schema::MIN_TIMEOUT_MS,
                    "maximum": schema::MAX_TIMEOUT_MS,
                    "default": schema::DEFAULT_TIMEOUT_MS,
                    "description": "Wall-clock limit in milliseconds; the whole process tree is killed when it expires"
                }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    fn tier(&self) -> ToolTier {
        // 创建 OS 进程：**不得**降为 `Sandbox`，否则 `max_tool_tier = sandbox` 会静默放行。
        ToolTier::Os
    }

    fn audience(&self) -> ToolAudience {
        // ToolBox 入口不解析工作区，拿不到可写根 → 必然失败，故不暴露。
        ToolAudience::AGENT_ONLY
    }

    fn behavior_hints(&self) -> BehaviorHints {
        // 静态声明取最坏值（该方法无入参）；逐命令的真实类别由 `CommandClass` 表达，
        // 两者不合并：卡片既要知道「最坏可能」，也要知道「这一次是什么」。
        BehaviorHints {
            read_only: false,
            destructive: true,
            open_world: true,
        }
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        // 进程执行争用资源，输出也无法与其它调用交错。
        false
    }

    fn manages_own_execution_timeout(&self) -> bool {
        // `exec::invoke` 的统一 deadline 对子进程无效：超时必须自己等、自己杀树。
        true
    }

    fn sandbox_enforcement(&self) -> Option<SandboxEnforcement> {
        // 进审批键：强边界下批准的授权不得在弱边界上复用（§6.2）。
        Some(self.probe.enforcement)
    }

    fn validate_input(&self, input: &Value, context: &ToolUseContext) -> ValidationResult {
        // 与 `execute` 共用同一条解析路径；本步失败不弹审批框。
        ShellInput::validate(input, context)
    }

    fn permission_intents(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let parsed = ShellInput::parse(input, context).map_err(|error| error.to_string())?;
        // 一条命令 = 一条意图 = 一个审批键；**不拆**成多条（拆了会让「A && B 只批了 A」成立）。
        Ok(vec![PermissionIntent::new("execute", parsed.command)])
    }

    async fn preview_for_approval(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Option<ApprovalPreview> {
        let parsed = ShellInput::parse(input, context).ok()?;
        let class = classify::classify(&parsed.command);
        Some(ApprovalPreview {
            summary: summary_for(class, self.probe.enforcement),
            // 命令的实际影响在执行前不可知（TOCTOU），禁止声称精确量。
            truncated: false,
            estimated: true,
        })
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        Ok(run::execute(input, context, self.probe, self.storage.clone()).await)
    }
}

/// 审批摘要：`执行 1 条命令 · <类别> · 隔离：<强度>`。
///
/// 类别来自纯函数分类器，只用于展示；强度来自注册期探测结论。
fn summary_for(class: CommandClass, enforcement: SandboxEnforcement) -> String {
    format!(
        "执行 1 条命令 · {} · 隔离：{}",
        class.label_zh(),
        enforcement.label_zh()
    )
}
