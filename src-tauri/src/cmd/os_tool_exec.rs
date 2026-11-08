use crate::protocol::{OsToolExecRequestDto, OsToolResultDto};
use assistant_tools::{invoke, ToolRegistry, ToolUseContext, DEFAULT_TOOL_TIMEOUT};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn os_tool_exec(
    registry: State<'_, Arc<ToolRegistry>>,
    request: OsToolExecRequestDto,
) -> Result<OsToolResultDto, String> {
    let registered = registry
        .find_for_toolbox(&request.tool)
        .ok_or_else(|| format!("tool '{}' not found in registry", request.tool))?;

    let mut input = request.params;
    if let Some(obj) = input.as_object_mut() {
        obj.insert("action".to_string(), Value::String(request.action));
    } else {
        input = serde_json::json!({ "action": request.action });
    }

    let context = ToolUseContext::for_toolbox(None);

    let result = invoke(
        registered.tool.as_ref(),
        &input,
        &context,
        DEFAULT_TOOL_TIMEOUT,
    )
    .await;

    Ok(OsToolResultDto::from(result))
}
