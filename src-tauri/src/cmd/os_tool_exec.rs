use crate::protocol::{OsToolExecRequestDto, OsToolResultDto};
use assistant_tools::{ToolRegistry, ToolUseContext};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn os_tool_exec(
    registry: State<'_, Arc<ToolRegistry>>,
    request: OsToolExecRequestDto,
) -> Result<OsToolResultDto, String> {
    let tool = registry
        .find(&request.tool)
        .ok_or_else(|| format!("tool '{}' not found in registry", request.tool))?;

    let mut input = request.params;
    if let Some(obj) = input.as_object_mut() {
        obj.insert("action".to_string(), Value::String(request.action));
    } else {
        input = serde_json::json!({ "action": request.action });
    }

    let context = ToolUseContext::for_toolbox(None);

    let result = tool
        .execute(&input, &context)
        .await
        .map_err(|e| e.to_string())?;

    Ok(OsToolResultDto::from(result))
}
