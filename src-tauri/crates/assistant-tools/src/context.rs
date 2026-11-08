use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUseContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
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
}

impl ToolUseContext {
    pub fn for_toolbox(workspace_root: Option<String>) -> Self {
        Self {
            workspace_root,
            session_id: None,
            turn_id: None,
            iteration: None,
            tool_call_id: None,
        }
    }
}
