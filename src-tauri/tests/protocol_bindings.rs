use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use assistant_app::protocol::{
    AgentModeDto, AgentModeInputDto, AgentModeKindDto, AgentQuestionAnswerDto,
    AgentQuestionAnswerItemDto, AgentQuestionDto, AgentQuestionOptionDto, AgentQuestionRequestDto,
    AgentToolCallDto, AgentToolCallPayloadDto, AppConfDto, ApprovalContextDto,
    ApprovalExtensionDto, ApprovalExtensionSeverityDto, ApprovalLevelDto, ApprovalPreviewDto,
    ApprovalReasonDto, AttachmentDeleteResultDto, AttachmentSummary, ChatDeltaPayloadDto,
    ChatMessagesChangedPayloadDto, ChatRegeneratePayloadDto, ChatStreamStartPayloadDto,
    ChatUserMessageConfirmedPayloadDto, ClockModeDto, ClockSnapshotDto, ClockStatusDto,
    ContentBlockDto, ConversationAttachmentSummary, ConversationDetailDto, ConversationDto,
    ConversationListChangeKindDto, ConversationListChangedPayloadDto, ConversationPatchDto,
    ConversationQueryModeDto, ConversationQueryOptsDto, ConversationTerminatedPayloadDto,
    ConversationToolConfigDto, ConversationUnlockedPayloadDto, FileAttachmentRefDto, GrantScopeDto,
    ImageAttachmentRefDto, InstallMascotAssetResultDto, MascotConfDto, MessageChangeKindDto,
    MessageDto, MessageToolCallDto, MessageToolCallStatusDto, ModelConfigDto, MousePositionDto,
    OrbConfDto, OsToolExecRequestDto, OsToolResultDto, PermissionGrantDto, PermissionIntentDto,
    ProviderErrorCodeDto, ProviderErrorPayloadDto, ProviderInputDto, ProviderListDto,
    ReasoningLevelsDto, RetryNotificationDto, ScreenRectDto, ScreenSizeDto, SendChatMessageArgsDto,
    SessionStateChangedPayloadDto, SessionStateDto, SetGrantScopeArgsDto,
    SettingProviderChangedActionDto, SettingProviderChangedPayloadDto, ShortcutWarningPayloadDto,
    SiblingInfoDto, StorageBreakdownDto, ThemeChangedPayloadDto, TokenUsageCacheDto,
    TokenUsageCumulativeDto, TokenUsageDto, TokenUsageEstimatedSourceDto, TokenUsageOccupancyDto,
    TokenUsageSummaryDto, ToolApprovalDecisionDto, ToolApprovalKindDto, ToolApprovalRequestDto,
    ToolBehaviorDto, ToolErrorKindDto, ToolTierDto, TurnPhaseDto, VaultCipherAlgorithmDto,
    VaultConfDto, WindowModeChangedPayloadDto, WindowModeDto,
};
use ts_rs::{Config, TS};

fn ts_derived_dto_names() -> BTreeSet<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/protocol/dto");
    let mut names = BTreeSet::new();
    let entries = fs::read_dir(&dir).expect("protocol dto directory is readable");
    for entry in entries {
        let path = entry.expect("dto entry is readable").path();
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
    names
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
        AppConfDto::decl(&cfg),
        ApprovalContextDto::decl(&cfg),
        ApprovalExtensionDto::decl(&cfg),
        ApprovalExtensionSeverityDto::decl(&cfg),
        ApprovalLevelDto::decl(&cfg),
        ApprovalPreviewDto::decl(&cfg),
        ApprovalReasonDto::decl(&cfg),
        AttachmentDeleteResultDto::decl(&cfg),
        AttachmentSummary::decl(&cfg),
        ChatDeltaPayloadDto::decl(&cfg),
        ChatMessagesChangedPayloadDto::decl(&cfg),
        ChatRegeneratePayloadDto::decl(&cfg),
        ChatStreamStartPayloadDto::decl(&cfg),
        ChatUserMessageConfirmedPayloadDto::decl(&cfg),
        ClockModeDto::decl(&cfg),
        ClockSnapshotDto::decl(&cfg),
        ClockStatusDto::decl(&cfg),
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
        FileAttachmentRefDto::decl(&cfg),
        GrantScopeDto::decl(&cfg),
        ImageAttachmentRefDto::decl(&cfg),
        InstallMascotAssetResultDto::decl(&cfg),
        MascotConfDto::decl(&cfg),
        MessageChangeKindDto::decl(&cfg),
        MessageDto::decl(&cfg),
        MessageToolCallDto::decl(&cfg),
        MessageToolCallStatusDto::decl(&cfg),
        ModelConfigDto::decl(&cfg),
        MousePositionDto::decl(&cfg),
        OrbConfDto::decl(&cfg),
        OsToolExecRequestDto::decl(&cfg),
        OsToolResultDto::decl(&cfg),
        PermissionGrantDto::decl(&cfg),
        PermissionIntentDto::decl(&cfg),
        ProviderErrorCodeDto::decl(&cfg),
        ProviderErrorPayloadDto::decl(&cfg),
        ProviderInputDto::decl(&cfg),
        ProviderListDto::decl(&cfg),
        ReasoningLevelsDto::decl(&cfg),
        RetryNotificationDto::decl(&cfg),
        ScreenRectDto::decl(&cfg),
        ScreenSizeDto::decl(&cfg),
        SendChatMessageArgsDto::decl(&cfg),
        SessionStateChangedPayloadDto::decl(&cfg),
        SessionStateDto::decl(&cfg),
        SetGrantScopeArgsDto::decl(&cfg),
        SettingProviderChangedActionDto::decl(&cfg),
        SettingProviderChangedPayloadDto::decl(&cfg),
        ShortcutWarningPayloadDto::decl(&cfg),
        SiblingInfoDto::decl(&cfg),
        StorageBreakdownDto::decl(&cfg),
        ThemeChangedPayloadDto::decl(&cfg),
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
        ToolErrorKindDto::decl(&cfg),
        ToolTierDto::decl(&cfg),
        TurnPhaseDto::decl(&cfg),
        VaultCipherAlgorithmDto::decl(&cfg),
        VaultConfDto::decl(&cfg),
        WindowModeChangedPayloadDto::decl(&cfg),
        WindowModeDto::decl(&cfg),
    ];

    let mut output = String::from(
        "// This file was generated by `npm run contract:gen` (ts-rs). Do not edit manually.\n\n",
    );
    for declaration in &declarations {
        output.push_str("export ");
        output.push_str(declaration);
        output.push('\n');
    }

    let out_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/types/protocol.contract.ts");
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
        "declarations 与 src/protocol/dto 的 TS 派生类型不一致：漏注册 {missing:?}，多注册 {stale:?}"
    );
    fs::write(&out_path, output).expect("write protocol contract");
}
