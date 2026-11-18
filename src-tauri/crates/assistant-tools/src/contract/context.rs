use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

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
        }
    }
}
