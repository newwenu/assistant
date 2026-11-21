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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ToolErrorKindDto {
    Business,
    Timeout,
    Cancelled,
    Internal,
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct OsToolResultDto {
    pub ok: bool,
    #[serde(default)]
    #[ts(type = "unknown")]
    pub data: Option<Value>,
    #[serde(default)]
    pub result_for_assistant: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_kind: Option<ToolErrorKindDto>,
}
