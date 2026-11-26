// 参考: docs/store/I75-startup-layering-spec.md §4.4 / C6 — 每资源一行类型化访问器

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::app_dirs::AppDirs;
use crate::boot::conf::BootConf;
use crate::boot::registry::Registry;
use crate::boot::resources::{CapErr, CapValue};

/// 未就绪返回 `NotReady`：不 panic、不隐式装配、不回落默认实例。
fn slot(app: &AppHandle, id: &'static str) -> Result<CapValue, CapErr> {
    app.state::<Registry>().get(id).ok_or(CapErr::not_ready(id))
}

fn typed<T: std::any::Any + Send + Sync>(
    value: CapValue,
    id: &'static str,
) -> Result<Arc<T>, CapErr> {
    value
        .downcast::<T>()
        .map_err(|_| CapErr::build(id, "capability type mismatch"))
}

pub fn dirs(app: &AppHandle) -> Result<Arc<AppDirs>, CapErr> {
    typed(slot(app, "dirs")?, "dirs")
}

pub fn boot_conf(app: &AppHandle) -> Result<Arc<BootConf>, CapErr> {
    typed(slot(app, "boot_conf")?, "boot_conf")
}

pub fn tools(app: &AppHandle) -> Result<Arc<assistant_tools::ToolRegistry>, CapErr> {
    typed(slot(app, "tool_runtime")?, "tool_runtime")
}

pub fn tool_exec(app: &AppHandle) -> Result<Arc<crate::adapters::PipelineToolExecution>, CapErr> {
    typed(slot(app, "tool_exec")?, "tool_exec")
}

/// MCP 管理器（`tool_runtime` 顺带产出）：命令层与启动序列共用这一份实例。
pub fn mcp_manager(app: &AppHandle) -> Result<Arc<assistant_tools::mcp::McpServerManager>, CapErr> {
    typed(slot(app, "mcp_manager")?, "mcp_manager")
}

pub fn pending_approval(
    app: &AppHandle,
) -> Result<Arc<crate::adapters::PendingApprovalRegistry>, CapErr> {
    typed(slot(app, "pending_approval")?, "pending_approval")
}

pub fn pending_question(
    app: &AppHandle,
) -> Result<Arc<crate::adapters::PendingQuestionRegistry>, CapErr> {
    typed(slot(app, "pending_question")?, "pending_question")
}

/// 审计存储：ToolBox 直调与网关必须落在同一份实例上，禁止各自新建。
pub fn permission_audit(
    app: &AppHandle,
) -> Result<Arc<dyn assistant_tools::PermissionAuditStore>, CapErr> {
    let store = typed::<crate::adapters::DbPermissionAuditStore>(
        slot(app, "permission_audit")?,
        "permission_audit",
    )?;
    Ok(store)
}
