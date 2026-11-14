use assistant_agent::agent::round_executor::ToolErrorClass;
use assistant_agent::agent::tool_event::{CompactUnit, ToolCompact, ToolEventPayload};
use assistant_tools::{
    ApprovalContext, ApprovalExtension, ApprovalExtensionSeverity, ApprovalPreview, ApprovalReason,
    BehaviorHints, PermissionIntent, ToolTier,
};
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
            ToolTierDto::Sandbox => ToolTier::Sandbox,
            ToolTierDto::Os => ToolTier::Os,
            ToolTierDto::External => ToolTier::External,
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

impl From<&str> for ApprovalLevelDto {
    /// 未知值按 fail-closed 兜底到最严级别（全部 OS 工具都要审批）。
    fn from(value: &str) -> Self {
        match value {
            "none" => Self::None,
            "dangerous_only" => Self::DangerousOnly,
            _ => Self::AllOsTools,
        }
    }
}

impl From<ApprovalLevelDto> for assistant_tools::ApprovalLevel {
    fn from(level: ApprovalLevelDto) -> Self {
        match level {
            ApprovalLevelDto::None => Self::None,
            ApprovalLevelDto::DangerousOnly => Self::DangerousOnly,
            ApprovalLevelDto::AllOsTools => Self::AllOsTools,
        }
    }
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
    /// 未下发配置时的 fail-closed 兜底（I38d §4.4）：沙箱层级、不额外审批，
    /// 名单留空 → 解析出空工具集（模型看不到工具），而不是自动全量注入。
    fn default() -> Self {
        Self {
            max_tool_tier: ToolTierDto::Sandbox,
            approval_level: ApprovalLevelDto::None,
            enabled_tools: Vec::new(),
            disabled_tools: Vec::new(),
        }
    }
}

/// 工具装配清单条目（`ToolConfigPanel` 渲染用，I38d §4.7）。
///
/// 只含面板需要的事实：`id` 是白名单取值，`name` 是展示名，`tier` 决定分组。
/// 不下发 `inputSchema`：装配面板不展示参数结构，且它体积大。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDescriptorDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: ToolTierDto,
}

impl From<assistant_tools::ToolDescriptor> for ToolDescriptorDto {
    fn from(descriptor: assistant_tools::ToolDescriptor) -> Self {
        Self {
            id: descriptor.id,
            name: descriptor.name,
            description: descriptor.description,
            tier: descriptor.tier.into(),
        }
    }
}

/// 工具结果的展示投影（wire 侧）：渲染与历史重载的唯一事实源（I46 compact §3.4）。
///
/// 输出 DTO：所有 `Option` 键恒在、值为 `null`（不加 `skip_serializing_if`）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolResultCompactDto {
    pub head: String,
    pub truncated: Option<TruncatedDto>,
    pub persisted: Option<PersistedResultDto>,
    pub error_kind: Option<ToolErrorKindDto>,
    pub anchor: Option<DocAnchorDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TruncatedUnitDto {
    Lines,
    Items,
    Bytes,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TruncatedDto {
    pub unit: TruncatedUnitDto,
    pub returned: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedResultDto {
    pub path: String,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocAnchorDto {
    pub path: String,
    pub ranges: Vec<DocLineRangeDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocLineRangeDto {
    pub start: u64,
    pub end: u64,
}

impl From<ToolCompact> for ToolResultCompactDto {
    fn from(compact: ToolCompact) -> Self {
        Self {
            head: compact.head,
            truncated: compact.truncated.map(|info| TruncatedDto {
                unit: info.unit.into(),
                returned: info.returned,
                total: info.total,
            }),
            persisted: compact.persisted.map(|info| PersistedResultDto {
                path: info.path,
                bytes: info.bytes,
            }),
            error_kind: compact.error_kind.map(Into::into),
            anchor: compact.anchor.map(|anchor| DocAnchorDto {
                path: anchor.path,
                ranges: anchor
                    .ranges
                    .into_iter()
                    .map(|range| DocLineRangeDto {
                        start: range.start,
                        end: range.end,
                    })
                    .collect(),
            }),
        }
    }
}

impl From<CompactUnit> for TruncatedUnitDto {
    fn from(unit: CompactUnit) -> Self {
        match unit {
            CompactUnit::Lines => Self::Lines,
            CompactUnit::Items => Self::Items,
            CompactUnit::Bytes => Self::Bytes,
            CompactUnit::Unknown => Self::Unknown,
        }
    }
}

impl From<assistant_tools::compact::TruncatedUnit> for TruncatedUnitDto {
    fn from(unit: assistant_tools::compact::TruncatedUnit) -> Self {
        match unit {
            assistant_tools::compact::TruncatedUnit::Lines => Self::Lines,
            assistant_tools::compact::TruncatedUnit::Items => Self::Items,
            assistant_tools::compact::TruncatedUnit::Bytes => Self::Bytes,
            assistant_tools::compact::TruncatedUnit::Unknown => Self::Unknown,
        }
    }
}

impl From<assistant_tools::compact::ToolResultCompact> for ToolResultCompactDto {
    fn from(compact: assistant_tools::compact::ToolResultCompact) -> Self {
        Self {
            head: compact.head,
            truncated: compact.truncated.map(|info| TruncatedDto {
                unit: info.unit.into(),
                returned: info.returned,
                total: info.total,
            }),
            persisted: compact.persisted.map(|info| PersistedResultDto {
                path: info.path,
                bytes: info.bytes,
            }),
            error_kind: compact.error_kind.map(Into::into),
            anchor: compact.anchor.map(|anchor| DocAnchorDto {
                path: anchor.path,
                ranges: anchor
                    .ranges
                    .into_iter()
                    .map(|range| DocLineRangeDto {
                        start: range.start,
                        end: range.end,
                    })
                    .collect(),
            }),
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
    Started {
        /// 参数摘要（`Started` 首帧只带摘要，完整参数走 `ArgsDelta` 或终态）。
        #[serde(rename = "argsPreview")]
        args_preview: Option<String>,
    },
    ArgsDelta {
        delta: String,
    },
    Finished {
        result: String,
        #[serde(rename = "confirmationWaitMs")]
        confirmation_wait_ms: Option<u64>,
        compact: Option<ToolResultCompactDto>,
    },
    Failed {
        error: String,
        #[serde(rename = "errorKind")]
        error_kind: ToolErrorKindDto,
        #[serde(rename = "confirmationWaitMs")]
        confirmation_wait_ms: Option<u64>,
        compact: Option<ToolResultCompactDto>,
    },
    AwaitingApproval {
        approval: ToolApprovalRequestDto,
    },
    /// 等待被结束（超时或 turn 取消），卡片必须转为不可操作。
    ///
    /// 后端在 drop 点无法区分两者（超时是网关丢弃 future，取消是任务被丢弃），
    /// 因此只表达「这张卡已失效」，不冒充「用户拒绝了」。
    ApprovalClosed {
        #[serde(rename = "timeoutMs")]
        timeout_ms: u64,
    },
    AwaitingAnswer {
        question: AgentQuestionRequestDto,
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
            ToolEventPayload::Started { args_preview } => Self::Started { args_preview },
            ToolEventPayload::ArgsDelta { delta } => Self::ArgsDelta { delta },
            ToolEventPayload::Finished {
                result,
                confirmation_wait_ms,
                compact,
            } => Self::Finished {
                result,
                confirmation_wait_ms,
                compact: compact.map(Into::into),
            },
            ToolEventPayload::Failed {
                error,
                error_kind,
                confirmation_wait_ms,
                compact,
            } => Self::Failed {
                error,
                error_kind: error_kind.into(),
                confirmation_wait_ms,
                compact: compact.map(Into::into),
            },
        }
    }
}

/// 权限意图（wire 侧）。与 `assistant_tools::PermissionIntent` 同形，内部类型不外泄到契约。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionIntentDto {
    pub action: String,
    pub resources: Vec<String>,
}

impl From<PermissionIntent> for PermissionIntentDto {
    fn from(intent: PermissionIntent) -> Self {
        Self {
            action: intent.action,
            resources: intent.resources,
        }
    }
}

/// 工具行为提示（wire 侧）：卡片据此回答「能不能撤销」。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolBehaviorDto {
    pub read_only: bool,
    pub destructive: bool,
    pub open_world: bool,
}

impl From<BehaviorHints> for ToolBehaviorDto {
    fn from(hints: BehaviorHints) -> Self {
        Self {
            read_only: hints.read_only,
            destructive: hints.destructive,
            open_world: hints.open_world,
        }
    }
}

/// 审批辅助信息的唯一包裹。新增审批信息只在此扩展，不再加 `ToolApprovalRequestDto` 顶层字段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalContextDto {
    pub tier: ToolTierDto,
    pub behavior: ToolBehaviorDto,
    pub reasons: Vec<ApprovalReasonDto>,
    pub preview: Option<ApprovalPreviewDto>,
    pub extensions: Vec<ApprovalExtensionDto>,
}

/// 「为什么问我」。封闭枚举：前端不得窄化，未知分支须能通用降级（`advisor` P6 恒不出现）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ApprovalReasonDto {
    DefaultPolicy { rule: String },
    ApprovalLevel { level: ApprovalLevelDto },
    ProjectRule { action: String, resource: String },
    TierCeiling { tier: ToolTierDto },
    Advisor { rationale: String },
}

/// 一句话量级摘要。整体为 `null` 表示「未知」，与「零」可区分。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalPreviewDto {
    pub summary: String,
    pub truncated: bool,
    pub estimated: bool,
}

impl From<ApprovalPreview> for ApprovalPreviewDto {
    fn from(preview: ApprovalPreview) -> Self {
        Self {
            summary: preview.summary,
            truncated: preview.truncated,
            estimated: preview.estimated,
        }
    }
}

/// 开放检测结论：`kind` 开放、`severity` 封闭（仅用于着色）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalExtensionDto {
    pub kind: String,
    pub label: String,
    pub value: String,
    pub severity: ApprovalExtensionSeverityDto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalExtensionSeverityDto {
    Info,
    Caution,
    Danger,
}

impl From<ApprovalExtensionSeverity> for ApprovalExtensionSeverityDto {
    fn from(severity: ApprovalExtensionSeverity) -> Self {
        match severity {
            ApprovalExtensionSeverity::Info => Self::Info,
            ApprovalExtensionSeverity::Caution => Self::Caution,
            ApprovalExtensionSeverity::Danger => Self::Danger,
        }
    }
}

impl From<ApprovalExtension> for ApprovalExtensionDto {
    fn from(extension: ApprovalExtension) -> Self {
        Self {
            kind: extension.kind,
            label: extension.label,
            value: extension.value,
            severity: extension.severity.into(),
        }
    }
}

impl From<ApprovalReason> for ApprovalReasonDto {
    fn from(reason: ApprovalReason) -> Self {
        match reason {
            ApprovalReason::DefaultPolicy { rule } => Self::DefaultPolicy { rule },
            ApprovalReason::ApprovalLevel { level } => Self::ApprovalLevel {
                level: ApprovalLevelDto::from(level.as_str()),
            },
            ApprovalReason::ProjectRule { action, resource } => {
                Self::ProjectRule { action, resource }
            }
            ApprovalReason::TierCeiling { tier } => Self::TierCeiling { tier: tier.into() },
            ApprovalReason::Advisor { rationale } => Self::Advisor { rationale },
        }
    }
}

impl From<ApprovalContext> for ApprovalContextDto {
    fn from(context: ApprovalContext) -> Self {
        Self {
            tier: context.tier.into(),
            behavior: context.behavior.into(),
            reasons: context.reasons.into_iter().map(Into::into).collect(),
            preview: context.preview.map(Into::into),
            extensions: context.extensions.into_iter().map(Into::into).collect(),
        }
    }
}

/// 审批请求（`agent-tool-call` 的 `awaitingApproval` 载荷）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolApprovalRequestDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: String,
    #[ts(type = "unknown")]
    pub arguments: serde_json::Value,
    pub intents: Vec<PermissionIntentDto>,
    pub timeout_ms: u64,
    pub context: ApprovalContextDto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum ToolApprovalKindDto {
    Approve,
    Deny,
    AllowSession,
    AllowAlways,
}

/// 审批决策（输入 DTO）。
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolApprovalDecisionDto {
    pub conversation_id: String,
    pub call_id: String,
    pub decision: ToolApprovalKindDto,
    #[ts(optional = nullable)]
    pub reason: Option<String>,
}

/// 授权作用域（封闭两档，与卡片按钮一一对应）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum GrantScopeDto {
    /// 仅 `conversationId` 相同的会话。
    Session,
    /// 所有会话。
    Global,
}

/// 已授权操作（`permission_grants` 的一行）。
///
/// `grantKey` 是审批缓存的精确主键（作用域前缀 + `ApprovalKey` 的规范 JSON），
/// 撤销时原样回传；`scopeSummary` 由后端归一成人可读文本，前端**不得**解析
/// `grantKey` 内部结构——作用域形状会随工具增长，解析逻辑只保留在 Rust 侧一处。
/// `scope` / `conversationId` 由后端解析前缀得出，历史无前缀行按 `global` 兼容。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionGrantDto {
    pub grant_key: String,
    pub tool: String,
    pub scope: GrantScopeDto,
    pub conversation_id: Option<String>,
    pub scope_summary: String,
    pub created_at: i64,
}

/// 修改授权作用域（输入 DTO）。`scope = session` 时 `conversationId` 必填。
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetGrantScopeArgsDto {
    pub grant_key: String,
    pub scope: GrantScopeDto,
    #[ts(optional = nullable)]
    pub conversation_id: Option<String>,
}

/// 提问请求（`agent-tool-call` 的 `awaitingAnswer` 载荷）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionRequestDto {
    pub conversation_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub questions: Vec<AgentQuestionDto>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionDto {
    pub question: String,
    pub header: String,
    pub options: Vec<AgentQuestionOptionDto>,
    pub multi_select: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionOptionDto {
    pub label: String,
    pub description: String,
}

/// 提问回答（输入 DTO）。`answers` 长度必须等于 `questions`。
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionAnswerDto {
    pub conversation_id: String,
    pub call_id: String,
    pub answers: Vec<AgentQuestionAnswerItemDto>,
}

/// 单题回答：单选也走数组（恒 0–1 项）；自由输入写 `other`，不塞进 `selected`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentQuestionAnswerItemDto {
    pub selected: Vec<String>,
    #[ts(optional = nullable)]
    pub other: Option<String>,
}
