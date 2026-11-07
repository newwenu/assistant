use assistant_agent::agent::round_executor::ToolErrorClass;
use assistant_agent::agent::tool_event::ToolEventPayload;
use assistant_tools::ToolTier;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::os_tool::ToolErrorKindDto;

/// 工具层级（wire 侧）。与 `assistant_tools::ToolTier` 同形，但内部类型不外泄到契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ToolTierDto {
    Sandbox,
    Os,
    External,
}

impl From<ToolTier> for ToolTierDto {
    fn from(tier: ToolTier) -> Self {
        match tier {
            ToolTier::Sandbox => Self::Sandbox,
            ToolTier::Os => Self::Os,
            ToolTier::External => Self::External,
        }
    }
}

impl From<ToolTierDto> for ToolTier {
    fn from(tier: ToolTierDto) -> Self {
        match tier {
            ToolTierDto::Sandbox => Self::Sandbox,
            ToolTierDto::Os => Self::Os,
            ToolTierDto::External => Self::External,
        }
    }
}

/// 审批级别。`snake_case` 与 I38d §4.2 的 `'none' | 'dangerous_only' | 'all_os_tools'` 对齐。
///
/// P5 只承载不消费：级别 → Policy 规则的映射由 P6 完成。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLevelDto {
    None,
    DangerousOnly,
    AllOsTools,
}

/// 一次发送携带的工具配置（前端内存态，随请求下发）。
///
/// 空白名单即 deny-by-default：模型看不到任何工具。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationToolConfigDto {
    pub max_tool_tier: ToolTierDto,
    pub approval_level: ApprovalLevelDto,
    pub enabled_tools: Vec<String>,
    pub disabled_tools: Vec<String>,
}

impl Default for ConversationToolConfigDto {
    /// 未下发配置时的 fail-closed 兜底：层级到 `Os`、审批按 `DangerousOnly`，
    /// 名单留空 → 解析出空工具集（模型看不到工具），而不是自动全量注入。
    fn default() -> Self {
        Self {
            max_tool_tier: ToolTierDto::Os,
            approval_level: ApprovalLevelDto::DangerousOnly,
            enabled_tools: Vec::new(),
            disabled_tools: Vec::new(),
        }
    }
}

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
