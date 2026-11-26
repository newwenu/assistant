// 参考: docs/store/I75-startup-layering-spec.md §9 #15 — MCP 拉起（Idle，单 server 失败只记日志）

use std::sync::Arc;

use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};
use assistant_tools::mcp::McpServerManager;

pub fn build(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    // 依赖 `tool_runtime` 的产物：未装配时 `NotReady`（I4），已失败则等下一次 `ensure` 重试。
    ctx.get::<McpServerManager>("mcp_manager")?;
    crate::mcp::config::start_enabled_servers(ctx.app);
    Ok(Arc::new(()))
}
