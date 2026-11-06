use assistant_agent::agent::round_executor::ToolErrorClass;
use assistant_agent::agent::tool_event::ToolEventPayload;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::os_tool::ToolErrorKindDto;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentToolCallDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub iteration: u32,
    pub call_id: String,
    pub name: String,
    pub payload: AgentToolCallPayloadDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum AgentToolCallPayloadDto {
    Started,
    ArgsDelta {
        delta: String,
    },
    Finished {
        result: String,
        #[serde(rename = "confirmationWaitMs")]
        confirmation_wait_ms: Option<u64>,
    },
    Failed {
        error: String,
        #[serde(rename = "errorKind")]
        error_kind: ToolErrorKindDto,
        #[serde(rename = "confirmationWaitMs")]
        confirmation_wait_ms: Option<u64>,
    },
}

impl From<ToolErrorClass> for ToolErrorKindDto {
    fn from(kind: ToolErrorClass) -> Self {
        match kind {
            ToolErrorClass::Business => Self::Business,
            ToolErrorClass::Timeout => Self::Timeout,
            ToolErrorClass::Cancelled => Self::Cancelled,
            ToolErrorClass::Internal => Self::Internal,
            ToolErrorClass::Denied => Self::Denied,
        }
    }
}

impl From<ToolEventPayload> for AgentToolCallPayloadDto {
    fn from(payload: ToolEventPayload) -> Self {
        match payload {
            ToolEventPayload::Started => Self::Started,
            ToolEventPayload::ArgsDelta { delta } => Self::ArgsDelta { delta },
            ToolEventPayload::Finished {
                result,
                confirmation_wait_ms,
            } => Self::Finished {
                result,
                confirmation_wait_ms,
            },
            ToolEventPayload::Failed {
                error,
                error_kind,
                confirmation_wait_ms,
            } => Self::Failed {
                error,
                error_kind: error_kind.into(),
                confirmation_wait_ms,
            },
        }
    }
}
