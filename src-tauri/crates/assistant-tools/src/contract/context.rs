use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::catalog::ToolCatalog;

/// 一次工具调用的来源：回答「这次调用是谁发起的」，**不参与**任何策略求值。
///
/// 与「有无会话」不是一回事：编排节点调用既有会话也有审批，只是不是轮内派发。
/// 封闭集合：新增变体必须同时补 `as_str()` 与审计词表说明。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolCallSource {
    /// 轮内派发：Agent 的一轮里由模型选择并调用。
    #[default]
    Agent,
    /// ToolBox 直调：用户手动执行，不走网关、无会话边界。
    Toolbox,
    /// 编排节点调用：图里的一等执行体（当前未接线）。
    Orchestration,
}

impl ToolCallSource {
    /// 落库串（`permission_audit.source`），与变体一一对应。
    pub fn as_str(self) -> &'static str {
        match self {
            ToolCallSource::Agent => "agent",
            ToolCallSource::Toolbox => "toolbox",
            ToolCallSource::Orchestration => "orchestration",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUseContext {
    /// 权限边界：路径必须落在其内（`os::path::ensure_within_root`）。
    /// 只有 Agent 链路填它；ToolBox 没有越界概念，保持 `None`（I18b §2.2）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    /// 相对路径的解析基准（`os::path::join_input`）。
    /// 与 `workspace_root` 是两件事：填了它不产生边界，只是让相对路径有根可依。
    /// 未填时回退到 `workspace_root`，保持既有调用点行为不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolve_base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// 当前轮次 id：审批事件必须带上它，前端才能按 `turnId` 丢弃迟到事件。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// 当前迭代序号，与 `turn_id` 一起构成审批事件的路由信息。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// turn 取消信号：MCP 客户端在 `call_tool` 内用它中止在途请求（I38e §4.3）。
    /// 运行时信号，不是 wire 字段，故不参与序列化。
    #[serde(skip)]
    pub cancel: Option<CancellationToken>,
    /// 本会话工具输出的落盘目录（回读域，I52 §5.11.4）。
    ///
    /// 运行时字段，不进 wire；由 `ToolResultStorage::namespace_root` 派生，
    /// **不得**在第二处重算路径。`None` = 不提供回读（ToolBox 无会话归属）。
    #[serde(skip)]
    pub tool_output_root: Option<PathBuf>,
    /// 行索引缓存根（`super_read` 的 `index_root`，I58 §3.5）。
    ///
    /// 运行时字段，不进 wire；由应用层注入（`AppDirs::indexes()`），
    /// 库 crate 不解析基根。`None` = 不提供索引，`super_read` 报业务错误，
    /// **不**静默降级为线性扫描。
    #[serde(skip)]
    pub index_root: Option<PathBuf>,
    /// 本轮真正可调用的工具名（`ToolSpec.name`，I58 §3.1）。
    ///
    /// 运行时字段，不进 wire；由 Agent 适配层从 `ToolExecCtx.tools` 映射。
    /// `None` = 没有本轮工具集事实（ToolBox 入口），`request_tool` 据此回退注册表。
    #[serde(skip)]
    pub available_tool_names: Option<Vec<String>>,
    /// 工具目录：`request_tool` 的唯一能力来源（I58 §3.1）。
    ///
    /// 运行时字段，不进 wire；由 `ToolPipeline` 每次调用前注入（注册表是唯一实现）。
    /// 工具实现不持有注册表，避免 `Arc<ToolRegistry>` → `Arc<dyn Tool>` 的环。
    #[serde(skip)]
    pub tool_catalog: Option<Arc<dyn ToolCatalog>>,
    /// 升级端口：`request_tool(detail = "enable")` 的唯一出口（I58 §3.8）。
    ///
    /// 运行时字段，不进 wire；由适配层每次 `execute` 现构造并注入。
    /// `None` = 没有升级通道（ToolBox / 单测），`enable` 分支返回业务错误。
    #[serde(skip)]
    pub escalation: Option<Arc<dyn crate::escalation::ToolEscalationPort>>,
    /// 调用来源：只用于审计归因（`permission_audit.source`），不进 wire。
    ///
    /// 运行时字段；缺省为 `Agent`（轮内派发是默认入口），ToolBox 构造点显式改填。
    #[serde(skip)]
    pub source: ToolCallSource,
}

impl ToolUseContext {
    /// ToolBox 上下文：只给相对路径一个解析基准，**不设权限边界**。
    ///
    /// 直接把 `workspace_root` 填进 ToolBox 会凭空造出越界规则（I18b §2.2），
    /// 故这里只填 `resolve_base`。
    pub fn for_toolbox(resolve_base: Option<String>) -> Self {
        Self {
            workspace_root: None,
            resolve_base,
            session_id: None,
            turn_id: None,
            iteration: None,
            tool_call_id: None,
            cancel: None,
            // ToolBox 无会话归属，因此没有输出域可读。
            tool_output_root: None,
            // ToolBox 直调不经应用层装配，因此没有索引缓存根。
            index_root: None,
            // ToolBox 不经 `resolve_available_tools`，没有「本轮工具集」事实。
            available_tool_names: None,
            // ToolBox 直调不经 `ToolPipeline`，因此没有目录可查。
            tool_catalog: None,
            // ToolBox 直调没有本轮工具集，因此没有升级通道。
            escalation: None,
            // 来源归因：ToolBox 直调不是轮内派发。
            source: ToolCallSource::Toolbox,
        }
    }
}
