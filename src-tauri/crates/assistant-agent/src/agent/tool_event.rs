use serde::Serialize;

use super::round_executor::ToolErrorClass;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolEvent {
    pub turn_id: String,
    pub iteration: u32,
    pub call_id: String,
    pub name: String,
    pub payload: ToolEventPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "phase")]
pub enum ToolEventPayload {
    Started,
    ArgsDelta {
        delta: String,
    },
    Finished {
        result: String,
        confirmation_wait_ms: Option<u64>,
    },
    Failed {
        error: String,
        error_kind: ToolErrorClass,
        confirmation_wait_ms: Option<u64>,
    },
}
