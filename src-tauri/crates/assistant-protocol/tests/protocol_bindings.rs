use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use assistant_protocol::{
    AfterIterationDetailDto, AfterToolDetailDto, AgentModeDto, AgentModeInputDto, AgentModeKindDto,
    AgentNodeDetailDto, AgentNodeDto, AgentNodeEventDto, AgentQuestionAnswerDto,
    AgentQuestionAnswerItemDto, AgentQuestionDto, AgentQuestionOptionDto, AgentQuestionRequestDto,
    AgentToolCallDto, AgentToolCallPayloadDto, AppConfDto, ApprovalContextDto,
    ApprovalExtensionDto, ApprovalExtensionSeverityDto, ApprovalLevelDto, ApprovalPreviewDto,
    ApprovalReasonDto, ApprovalRuleDto, ApprovalRuleEffectDto, AssetDirsDto,
    AttachmentDeleteResultDto, AttachmentImportDto, AttachmentLimitsDto, AttachmentSummary,
    BrakeDetailDto, BrakeVerdictDto, BranchDetailDto, BudgetRemainingDto, BudgetSplitDetailDto,
    CapIssueDto, CapStateDto, CapabilitySourceDto, CapabilityStateDto, CapabilitySupportDto,
    ChatDeltaPayloadDto, ChatMessagesChangedPayloadDto, ChatRegeneratePayloadDto,
    ChatStreamStartPayloadDto, ChatUserMessageConfirmedPayloadDto, ClockModeDto, ClockSnapshotDto,
    ClockStatusDto, CommandErrorCodeDto, CommandErrorDto, ContentBlockDto,
    ConversationAttachmentSummary, ConversationDetailDto, ConversationDto,
    ConversationListChangeKindDto, ConversationListChangedPayloadDto, ConversationPatchDto,
    ConversationQueryModeDto, ConversationQueryOptsDto, ConversationTerminatedPayloadDto,
    ConversationToolConfigDto, ConversationUnlockedPayloadDto, DocAnchorDto, DocLineRangeDto,
    FileAttachmentRefDto, FrameRecordDto, GrantScopeDto, ImageAttachmentRefDto,
    InstallMascotAssetResultDto, LadderDetailDto, LaneDto, MascotConfDto, McpLaunchInputDto,
    McpServerCreateDto, McpServerDto, McpServerStatusDto, McpServerUpdateDto, MessageChangeKindDto,
    MessageDto, MessageRecordDto, MessageStatusDto, MessageToolCallDto, MessageToolCallStatusDto,
    MiddlewareActionDto, MiddlewareVerdictDto, ModelCapabilityDto, ModelConfigDto, ModelDetailDto,
    ModelEffortSupportDto, MousePositionDto, NodeStateDto, OrbConfDto, OsToolExecRequestDto,
    OsToolResultDto, OutcomeDto, PermissionAuditEntryDto, PermissionAuditPageDto,
    PermissionGrantDto, PermissionIntentDto, PersistDetailDto, PersistedResultDto, PhaseRecordDto,
    PipelineDetailDto, PipelineStageDto, ProjectPermissionStatusDto, ProviderErrorPayloadDto,
    ProviderInputDto, ProviderListDto, ReasoningLevelsDto, ReclaimPolicyDto, ReclaimViewDto,
    ResolvedThemeDto, RetryNotificationDto, RiveManifestDto, RiveManifestPropertiesDto,
    ScreenRectDto, ScreenSizeDto, ScreenshotCropArgsDto, ScreenshotCropResultDto,
    ScreenshotSessionDto, SendChatMessageArgsDto, SessionStateChangedPayloadDto, SessionStateDto,
    SetGrantScopeArgsDto, SettingProviderChangedActionDto, SettingProviderChangedPayloadDto,
    ShellPreviewDto, ShortcutWarningPayloadDto, SiblingInfoDto, StartupReportDto,
    StorageAvailabilityDto, StorageBreakdownDto, StorageByteBasisDto, StorageConfirmLevelDto,
    StorageEligibilityDto, StorageEntryDto, StorageInventoryClassDto, StorageInventoryDto,
    StorageInventoryItemDto, StorageInventoryNodeDto, StorageInventoryScopeTotalsDto,
    StorageInventorySnapshotStatusDto, StorageInventoryStatusDto, StorageInventoryTotalsDto,
    StorageLayerDto, StoragePolicySourceDto, StorageReclaimActionDto,
    StorageReclaimEntryRequestDto, StorageReclaimEntryResultDto, StorageReclaimErrorCodeDto,
    StorageReclaimErrorDto, StorageReclaimPlanDto, StorageReclaimPlanEntryDto,
    StorageReclaimPlanRequestDto, StorageReclaimReasonDto, StorageReclaimRequestDto,
    StorageReclaimResultDto, StorageReclaimStatusDto, ThemeChangedPayloadDto, ThemeModeDto,
    TokenUsageCacheDto, TokenUsageCumulativeDto, TokenUsageDto, TokenUsageEstimatedSourceDto,
    TokenUsageOccupancyDto, TokenUsageSummaryDto, ToolApprovalDecisionDto, ToolApprovalKindDto,
    ToolApprovalRequestDto, ToolBehaviorDto, ToolDescriptorDto, ToolErrorKindDto,
    ToolResultCompactDto, ToolTierDto, TruncatedDto, TruncatedUnitDto, TurnPhaseDto,
    WindowModeChangedPayloadDto, WindowModeDto, WorkspaceDto,
};
use ts_rs::{Config, TS};

fn ts_derived_dto_names() -> BTreeSet<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut names = BTreeSet::new();
    collect_ts_derived_dto_names(&dir, &mut names);
    names
}

fn collect_ts_derived_dto_names(dir: &Path, names: &mut BTreeSet<String>) {
    let entries = fs::read_dir(dir).expect("protocol dto directory is readable");
    for entry in entries {
        let path = entry.expect("dto entry is readable").path();
        if path.is_dir() {
            collect_ts_derived_dto_names(&path, names);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is readable: {}", path.display(), e));
        let mut pending = false;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("#[derive(") {
                let inner = rest.trim_end_matches(']').trim_end_matches(')');
                pending = inner.split(',').map(str::trim).any(|item| item == "TS");
                continue;
            }
            if !pending {
                continue;
            }
            if line.starts_with("#[") || line.starts_with("//") || line.is_empty() {
                continue;
            }
            let rest = line
                .strip_prefix("pub struct ")
                .or_else(|| line.strip_prefix("pub enum "));
            if let Some(rest) = rest {
                let name = rest
                    .split(|c: char| c.is_whitespace() || c == '{' || c == '(' || c == ';')
                    .next()
                    .unwrap_or("");
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
            pending = false;
        }
    }
}

fn declared_dto_names(declarations: &[String]) -> BTreeSet<String> {
    declarations
        .iter()
        .map(|declaration| {
            let rest = declaration
                .strip_prefix("type ")
                .unwrap_or_else(|| panic!("declaration must start with `type `: {declaration}"));
            rest.split(|c: char| c.is_whitespace() || c == '<' || c == '=')
                .next()
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

#[test]
fn export_protocol_contract() {
    let cfg = Config::from_env().with_large_int("number");

    let declarations = [
        AgentModeDto::decl(&cfg),
        AgentModeInputDto::decl(&cfg),
        AgentModeKindDto::decl(&cfg),
        AgentQuestionAnswerDto::decl(&cfg),
        AgentQuestionAnswerItemDto::decl(&cfg),
        AgentQuestionDto::decl(&cfg),
        AgentQuestionOptionDto::decl(&cfg),
        AgentQuestionRequestDto::decl(&cfg),
        AgentToolCallDto::decl(&cfg),
        AgentToolCallPayloadDto::decl(&cfg),
        AfterIterationDetailDto::decl(&cfg),
        AfterToolDetailDto::decl(&cfg),
        AgentNodeDetailDto::decl(&cfg),
        AgentNodeDto::decl(&cfg),
        AgentNodeEventDto::decl(&cfg),
        AppConfDto::decl(&cfg),
        ApprovalContextDto::decl(&cfg),
        ApprovalExtensionDto::decl(&cfg),
        ApprovalExtensionSeverityDto::decl(&cfg),
        ApprovalLevelDto::decl(&cfg),
        ApprovalPreviewDto::decl(&cfg),
        ApprovalReasonDto::decl(&cfg),
        ApprovalRuleDto::decl(&cfg),
        ApprovalRuleEffectDto::decl(&cfg),
        AssetDirsDto::decl(&cfg),
        AttachmentDeleteResultDto::decl(&cfg),
        AttachmentImportDto::decl(&cfg),
        AttachmentLimitsDto::decl(&cfg),
        AttachmentSummary::decl(&cfg),
        BrakeDetailDto::decl(&cfg),
        BrakeVerdictDto::decl(&cfg),
        BranchDetailDto::decl(&cfg),
        BudgetRemainingDto::decl(&cfg),
        BudgetSplitDetailDto::decl(&cfg),
        CapabilitySourceDto::decl(&cfg),
        CapabilityStateDto::decl(&cfg),
        CapabilitySupportDto::decl(&cfg),
        CapIssueDto::decl(&cfg),
        CapStateDto::decl(&cfg),
        ChatDeltaPayloadDto::decl(&cfg),
        ChatMessagesChangedPayloadDto::decl(&cfg),
        ChatRegeneratePayloadDto::decl(&cfg),
        ChatStreamStartPayloadDto::decl(&cfg),
        ChatUserMessageConfirmedPayloadDto::decl(&cfg),
        ClockModeDto::decl(&cfg),
        ClockSnapshotDto::decl(&cfg),
        ClockStatusDto::decl(&cfg),
        CommandErrorCodeDto::decl(&cfg),
        CommandErrorDto::decl(&cfg),
        ContentBlockDto::decl(&cfg),
        ConversationAttachmentSummary::decl(&cfg),
        ConversationDetailDto::decl(&cfg),
        ConversationDto::decl(&cfg),
        ConversationListChangeKindDto::decl(&cfg),
        ConversationListChangedPayloadDto::decl(&cfg),
        ConversationPatchDto::decl(&cfg),
        ConversationQueryModeDto::decl(&cfg),
        ConversationQueryOptsDto::decl(&cfg),
        ConversationTerminatedPayloadDto::decl(&cfg),
        ConversationToolConfigDto::decl(&cfg),
        ConversationUnlockedPayloadDto::decl(&cfg),
        DocAnchorDto::decl(&cfg),
        DocLineRangeDto::decl(&cfg),
        FileAttachmentRefDto::decl(&cfg),
        FrameRecordDto::decl(&cfg),
        GrantScopeDto::decl(&cfg),
        ImageAttachmentRefDto::decl(&cfg),
        InstallMascotAssetResultDto::decl(&cfg),
        LadderDetailDto::decl(&cfg),
        LaneDto::decl(&cfg),
        MascotConfDto::decl(&cfg),
        McpLaunchInputDto::decl(&cfg),
        McpServerCreateDto::decl(&cfg),
        McpServerDto::decl(&cfg),
        McpServerStatusDto::decl(&cfg),
        McpServerUpdateDto::decl(&cfg),
        MessageChangeKindDto::decl(&cfg),
        MessageDto::decl(&cfg),
        MessageRecordDto::decl(&cfg),
        MessageStatusDto::decl(&cfg),
        MessageToolCallDto::decl(&cfg),
        MessageToolCallStatusDto::decl(&cfg),
        MiddlewareActionDto::decl(&cfg),
        MiddlewareVerdictDto::decl(&cfg),
        ModelCapabilityDto::decl(&cfg),
        ModelConfigDto::decl(&cfg),
        ModelDetailDto::decl(&cfg),
        ModelEffortSupportDto::decl(&cfg),
        MousePositionDto::decl(&cfg),
        NodeStateDto::decl(&cfg),
        OrbConfDto::decl(&cfg),
        OsToolExecRequestDto::decl(&cfg),
        OsToolResultDto::decl(&cfg),
        OutcomeDto::decl(&cfg),
        PermissionGrantDto::decl(&cfg),
        PermissionAuditEntryDto::decl(&cfg),
        PermissionAuditPageDto::decl(&cfg),
        PermissionIntentDto::decl(&cfg),
        PersistDetailDto::decl(&cfg),
        PhaseRecordDto::decl(&cfg),
        PersistedResultDto::decl(&cfg),
        PipelineDetailDto::decl(&cfg),
        PipelineStageDto::decl(&cfg),
        ProjectPermissionStatusDto::decl(&cfg),
        ProviderErrorPayloadDto::decl(&cfg),
        ProviderInputDto::decl(&cfg),
        ProviderListDto::decl(&cfg),
        ReasoningLevelsDto::decl(&cfg),
        ReclaimPolicyDto::decl(&cfg),
        ReclaimViewDto::decl(&cfg),
        ResolvedThemeDto::decl(&cfg),
        RetryNotificationDto::decl(&cfg),
        RiveManifestDto::decl(&cfg),
        RiveManifestPropertiesDto::decl(&cfg),
        ScreenRectDto::decl(&cfg),
        ScreenSizeDto::decl(&cfg),
        ScreenshotCropArgsDto::decl(&cfg),
        ScreenshotCropResultDto::decl(&cfg),
        ScreenshotSessionDto::decl(&cfg),
        SendChatMessageArgsDto::decl(&cfg),
        SessionStateChangedPayloadDto::decl(&cfg),
        SessionStateDto::decl(&cfg),
        SetGrantScopeArgsDto::decl(&cfg),
        SettingProviderChangedActionDto::decl(&cfg),
        SettingProviderChangedPayloadDto::decl(&cfg),
        ShortcutWarningPayloadDto::decl(&cfg),
        ShellPreviewDto::decl(&cfg),
        SiblingInfoDto::decl(&cfg),
        StartupReportDto::decl(&cfg),
        StorageAvailabilityDto::decl(&cfg),
        StorageBreakdownDto::decl(&cfg),
        StorageByteBasisDto::decl(&cfg),
        StorageConfirmLevelDto::decl(&cfg),
        StorageEligibilityDto::decl(&cfg),
        StorageEntryDto::decl(&cfg),
        StorageInventoryClassDto::decl(&cfg),
        StorageInventoryDto::decl(&cfg),
        StorageInventoryItemDto::decl(&cfg),
        StorageInventoryNodeDto::decl(&cfg),
        StorageInventoryScopeTotalsDto::decl(&cfg),
        StorageInventorySnapshotStatusDto::decl(&cfg),
        StorageInventoryStatusDto::decl(&cfg),
        StorageInventoryTotalsDto::decl(&cfg),
        StorageLayerDto::decl(&cfg),
        StoragePolicySourceDto::decl(&cfg),
        StorageReclaimActionDto::decl(&cfg),
        StorageReclaimEntryRequestDto::decl(&cfg),
        StorageReclaimEntryResultDto::decl(&cfg),
        StorageReclaimErrorCodeDto::decl(&cfg),
        StorageReclaimErrorDto::decl(&cfg),
        StorageReclaimPlanDto::decl(&cfg),
        StorageReclaimPlanEntryDto::decl(&cfg),
        StorageReclaimPlanRequestDto::decl(&cfg),
        StorageReclaimReasonDto::decl(&cfg),
        StorageReclaimRequestDto::decl(&cfg),
        StorageReclaimResultDto::decl(&cfg),
        StorageReclaimStatusDto::decl(&cfg),
        ThemeChangedPayloadDto::decl(&cfg),
        ThemeModeDto::decl(&cfg),
        TokenUsageCacheDto::decl(&cfg),
        TokenUsageCumulativeDto::decl(&cfg),
        TokenUsageDto::decl(&cfg),
        TokenUsageEstimatedSourceDto::decl(&cfg),
        TokenUsageOccupancyDto::decl(&cfg),
        TokenUsageSummaryDto::decl(&cfg),
        ToolApprovalDecisionDto::decl(&cfg),
        ToolApprovalKindDto::decl(&cfg),
        ToolApprovalRequestDto::decl(&cfg),
        ToolBehaviorDto::decl(&cfg),
        ToolDescriptorDto::decl(&cfg),
        ToolErrorKindDto::decl(&cfg),
        ToolResultCompactDto::decl(&cfg),
        ToolTierDto::decl(&cfg),
        TruncatedDto::decl(&cfg),
        TruncatedUnitDto::decl(&cfg),
        TurnPhaseDto::decl(&cfg),
        WindowModeChangedPayloadDto::decl(&cfg),
        WindowModeDto::decl(&cfg),
        WorkspaceDto::decl(&cfg),
    ];

    let mut output = String::from(
        "// This file was generated by `npm run contract:gen` (ts-rs). Do not edit manually.\n\n",
    );
    for declaration in &declarations {
        output.push_str("export ");
        output.push_str(declaration);
        output.push('\n');
    }

    let out_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../src/types/protocol.contract.ts");
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).expect("create contract directory");
    }
    let source_names = ts_derived_dto_names();
    let declared_names = declared_dto_names(&declarations);
    assert_eq!(
        declarations.len(),
        declared_names.len(),
        "declarations 存在重复注册，每个 DTO 只应出现一次"
    );
    let missing: Vec<&String> = source_names.difference(&declared_names).collect();
    let stale: Vec<&String> = declared_names.difference(&source_names).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "declarations 与 crate 源码根的 TS 派生类型不一致：漏注册 {missing:?}，多注册 {stale:?}"
    );
    fs::write(&out_path, output).expect("write protocol contract");
}

#[test]
fn legacy_tool_config_without_approval_rules_still_decodes_and_reemits_the_key() {
    let legacy = r#"{"maxToolTier":"external","approvalLevel":"dangerous_only","enabledTools":[],"disabledTools":[]}"#;
    let config: ConversationToolConfigDto =
        serde_json::from_str(legacy).expect("legacy json without approvalRules decodes");
    assert!(
        config.approval_rules.is_empty(),
        "A9: missing key defaults to []"
    );

    let encoded = serde_json::to_value(&config).expect("tool config is serializable");
    assert_eq!(
        encoded.get("approvalRules"),
        Some(&serde_json::json!([])),
        "A9: output always contains approvalRules"
    );
}

#[test]
fn unknown_rule_effect_fails_decoding_instead_of_defaulting() {
    let raw = r#"{"maxToolTier":"external","approvalLevel":"none","enabledTools":[],"disabledTools":[],"approvalRules":[{"action":"edit","resource":"**","effect":"always"}]}"#;
    assert!(
        serde_json::from_str::<ConversationToolConfigDto>(raw).is_err(),
        "E6: unknown effect must fail closed"
    );
}

#[test]
fn approval_rules_round_trip_in_camel_case() {
    let config = ConversationToolConfigDto {
        max_tool_tier: ToolTierDto::External,
        approval_level: ApprovalLevelDto::WorkspaceWrite,
        enabled_tools: Vec::new(),
        disabled_tools: Vec::new(),
        approval_rules: vec![ApprovalRuleDto {
            action: "execute".to_string(),
            resource: "**".to_string(),
            effect: ApprovalRuleEffectDto::Ask,
        }],
    };

    let encoded = serde_json::to_string(&config).expect("tool config is serializable");
    assert!(encoded.contains("\"approvalRules\""));
    assert!(encoded.contains("\"effect\":\"ask\""));
    let decoded: ConversationToolConfigDto = serde_json::from_str(&encoded).expect("round trip");
    assert_eq!(decoded, config);
}
