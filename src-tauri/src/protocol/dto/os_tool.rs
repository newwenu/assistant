use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OsToolExecRequestDto {
    pub tool: String,
    pub action: String,
    #[serde(default)]
    #[ts(type = "unknown")]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct OsToolResultDto {
    pub ok: bool,
    #[serde(default)]
    #[ts(type = "unknown")]
    pub data: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

impl From<assistant_tools::ToolResult> for OsToolResultDto {
    fn from(result: assistant_tools::ToolResult) -> Self {
        Self {
            ok: result.ok,
            data: result.data,
            error: result.error,
        }
    }
}
