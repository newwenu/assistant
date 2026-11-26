use crate::protocol::{OsToolExecRequestDto, OsToolResultDto};
use assistant_tools::{invoke, AuditEntry, ToolUseContext, DEFAULT_TOOL_TIMEOUT};
use serde_json::Value;
use tauri::AppHandle;

#[tauri::command]
pub async fn os_tool_exec(
    app: AppHandle,
    request: OsToolExecRequestDto,
) -> Result<OsToolResultDto, String> {
    // 能力门禁（C3 / C6）：一律经 `access::*` 取用，未就绪返回可判定错误而非 panic。
    let registry = crate::boot::tools(&app).map_err(|e| e.to_string())?;
    let audit = crate::boot::permission_audit(&app).map_err(|e| e.to_string())?;
    let registered = registry
        .find_for_toolbox(&request.tool)
        .ok_or_else(|| format!("tool '{}' not found in registry", request.tool))?;

    let mut input = request.params;
    if let Some(obj) = input.as_object_mut() {
        obj.insert("action".to_string(), Value::String(request.action));
    } else {
        input = serde_json::json!({ "action": request.action });
    }

    // ToolBox 没有会话、没有基准、没有边界（I38h §4.2）：相对路径直接报错。
    let context = ToolUseContext::for_toolbox(None);

    // ToolBox 直调不经网关：必须在执行前留 `bypassed`，与「策略允许」区分。
    // 审计失败只 warn，执行照常；放在 invoke 之前才能覆盖 panic / 超时路径。
    let entry = AuditEntry::bypassed(registered.tool.id(), "toolbox");
    if let Err(e) = audit.log_decision(entry).await {
        log::warn!("ToolBox audit failed for '{}': {}", registered.tool.id(), e);
    }

    let result = invoke(
        registered.tool.as_ref(),
        &input,
        &context,
        DEFAULT_TOOL_TIMEOUT,
    )
    .await;

    Ok(OsToolResultDto::from(result))
}
