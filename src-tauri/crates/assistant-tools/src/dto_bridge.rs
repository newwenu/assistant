// 域类型 → DTO 桥接（I62 §4.4）：按 Rust 孤儿规则归属本 crate。

use crate::ToolErrorKind;
use crate::{
    ApprovalContext, ApprovalExtension, ApprovalExtensionSeverity, ApprovalPreview, ApprovalReason,
    BehaviorHints, PermissionIntent, ToolTier,
};
use assistant_protocol::*;

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

impl From<ApprovalLevelDto> for crate::ApprovalLevel {
    fn from(level: ApprovalLevelDto) -> Self {
        match level {
            ApprovalLevelDto::None => Self::None,
            ApprovalLevelDto::DangerousOnly => Self::DangerousOnly,
            ApprovalLevelDto::AllOsTools => Self::AllOsTools,
        }
    }
}

impl From<crate::ToolDescriptor> for ToolDescriptorDto {
    fn from(descriptor: crate::ToolDescriptor) -> Self {
        Self {
            id: descriptor.id,
            name: descriptor.name,
            description: descriptor.description,
            tier: descriptor.tier.into(),
            companion_of: descriptor.companion_of,
        }
    }
}

impl From<crate::compact::TruncatedUnit> for TruncatedUnitDto {
    fn from(unit: crate::compact::TruncatedUnit) -> Self {
        match unit {
            crate::compact::TruncatedUnit::Lines => Self::Lines,
            crate::compact::TruncatedUnit::Items => Self::Items,
            crate::compact::TruncatedUnit::Bytes => Self::Bytes,
            crate::compact::TruncatedUnit::Unknown => Self::Unknown,
        }
    }
}

impl From<crate::compact::ToolResultCompact> for ToolResultCompactDto {
    fn from(compact: crate::compact::ToolResultCompact) -> Self {
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

impl From<PermissionIntent> for PermissionIntentDto {
    fn from(intent: PermissionIntent) -> Self {
        Self {
            action: intent.action,
            resources: intent.resources,
        }
    }
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

impl From<ApprovalPreview> for ApprovalPreviewDto {
    fn from(preview: ApprovalPreview) -> Self {
        Self {
            summary: preview.summary,
            truncated: preview.truncated,
            estimated: preview.estimated,
        }
    }
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

impl From<ToolErrorKind> for ToolErrorKindDto {
    fn from(kind: ToolErrorKind) -> Self {
        match kind {
            ToolErrorKind::Business => Self::Business,
            ToolErrorKind::Timeout => Self::Timeout,
            ToolErrorKind::Cancelled => Self::Cancelled,
            ToolErrorKind::Internal => Self::Internal,
            ToolErrorKind::Denied => Self::Denied,
        }
    }
}

impl From<crate::ToolResult> for OsToolResultDto {
    fn from(result: crate::ToolResult) -> Self {
        Self {
            ok: result.ok,
            data: result.data,
            result_for_assistant: result.result_for_assistant,
            error: result.error,
            error_kind: result.error_kind.map(ToolErrorKindDto::from),
        }
    }
}

impl From<crate::mcp::McpServerStatus> for McpServerStatusDto {
    fn from(status: crate::mcp::McpServerStatus) -> Self {
        match status {
            crate::mcp::McpServerStatus::Stopped => Self::Stopped,
            crate::mcp::McpServerStatus::Starting => Self::Starting,
            crate::mcp::McpServerStatus::Ready => Self::Ready,
            crate::mcp::McpServerStatus::Error => Self::Error,
        }
    }
}
