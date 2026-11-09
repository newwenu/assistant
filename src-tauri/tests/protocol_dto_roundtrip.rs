use assistant_app::protocol::{
    AgentModeDto, AgentModeKindDto, AgentToolCallDto, AgentToolCallPayloadDto, ChatDeltaPayloadDto,
    ChatRegeneratePayloadDto, ChatStreamStartPayloadDto, ConversationDetailDto, ConversationDto,
    ConversationPatchDto, ConversationQueryModeDto, ConversationQueryOptsDto,
    ConversationTerminatedPayloadDto, ConversationToolConfigDto, ConversationUnlockedPayloadDto,
    InstallMascotAssetResultDto, MessageDto, MessageStatusDto, MessageToolCallDto,
    MessageToolCallStatusDto, ModelConfigDto, MousePositionDto, OsToolExecRequestDto,
    OsToolResultDto, ProviderErrorCodeDto, ProviderErrorPayloadDto, ProviderListDto,
    ReasoningLevelsDto, RetryNotificationDto, ScreenRectDto, ScreenSizeDto, SendChatMessageArgsDto,
    SessionStateChangedPayloadDto, SessionStateDto, SettingProviderChangedActionDto,
    SettingProviderChangedPayloadDto, ShortcutWarningPayloadDto, ThemeChangedPayloadDto,
    TokenUsageDto, ToolErrorKindDto, TurnPhaseDto,
};
use assistant_storage::agent_modes::AgentModeRow;
use serde::{Deserialize, Serialize};

fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(value: &T) {
    let json = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&json).unwrap();
    assert_eq!(*value, back, "round-trip failed for {}", json);
}

#[test]
fn conversation_dto_roundtrip() {
    let dto = ConversationDto {
        id: "c1".into(),
        title: "Test".into(),
        title_source: "placeholder".into(),
        provider_id: None,
        model_id: "gpt-4".into(),
        system_prompt: None,
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: 1000,
        updated_at: 1000,
        is_full: false,
        active_branch_id: None,
        mode_id: None,
        workspace_root: String::new(),
        tool_config: None,
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("titleSource"), "camelCase rename: {}", json);
    assert!(!json.contains("title_source"), "no snake_case: {}", json);
}

#[test]
fn message_dto_roundtrip() {
    let dto = MessageDto {
        id: "m1".into(),
        conversation_id: "c1".into(),
        role: "user".into(),
        content: "hello".into(),
        tokens: None,
        timestamp: 1000,
        parent_id: None,
        is_pinned: false,
        reasoning: None,
        sibling_index: 0,
        sibling_count: None,
        provider_id: None,
        model_id: None,
        status: MessageStatusDto::Completed,
        tool_calls: Vec::new(),
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("conversationId"), "camelCase: {}", json);
}

#[test]
fn message_dto_roundtrip_keeps_tool_call_terminal_states() {
    let dto = MessageDto {
        id: "m1".into(),
        conversation_id: "c1".into(),
        role: "assistant".into(),
        content: String::new(),
        tokens: None,
        timestamp: 1000,
        parent_id: None,
        is_pinned: false,
        reasoning: None,
        sibling_index: 0,
        sibling_count: None,
        provider_id: None,
        model_id: None,
        status: MessageStatusDto::Interrupted,
        tool_calls: vec![
            MessageToolCallDto {
                call_id: "call_1".into(),
                name: "fs_read".into(),
                arguments: serde_json::json!({ "path": "a" }),
                status: MessageToolCallStatusDto::Success,
                result: Some("ok".into()),
            },
            MessageToolCallDto {
                call_id: "call_2".into(),
                name: "fs_write".into(),
                arguments: serde_json::Value::Null,
                status: MessageToolCallStatusDto::Cancelled,
                result: None,
            },
        ],
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(
        json.contains("\"callId\":\"call_1\""),
        "camelCase: {}",
        json
    );
    assert!(
        json.contains("\"status\":\"success\"") && json.contains("\"status\":\"cancelled\""),
        "snake_case enum: {}",
        json
    );
    assert!(
        json.contains("\"result\":null"),
        "output Option keeps the key with null: {}",
        json
    );
}

#[test]
fn message_status_dto_maps_db_values_and_falls_back_to_completed() {
    assert_eq!(
        MessageStatusDto::from_db("streaming"),
        MessageStatusDto::Streaming
    );
    assert_eq!(
        MessageStatusDto::from_db("completed"),
        MessageStatusDto::Completed
    );
    assert_eq!(
        MessageStatusDto::from_db("interrupted"),
        MessageStatusDto::Interrupted
    );
    assert_eq!(
        MessageStatusDto::from_db("failed"),
        MessageStatusDto::Failed
    );
    assert_eq!(
        MessageStatusDto::from_db("future-unknown-status"),
        MessageStatusDto::Completed,
        "未知取值必须兜底 completed，不得把存量数据读成终态之外的态"
    );

    let json = serde_json::to_string(&MessageStatusDto::Interrupted).unwrap();
    assert_eq!(json, "\"interrupted\"", "wire 形态为 snake_case");
}

#[test]
fn token_usage_dto_roundtrip_keeps_context_window() {
    let dto = TokenUsageDto {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
        cache_read_tokens: None,
        cache_write_tokens: None,
        reasoning_tokens: None,
        context_window_tokens: Some(128000),
    };
    roundtrip(&dto);
}

#[test]
fn retry_notification_dto_roundtrip() {
    let dto = RetryNotificationDto {
        attempt: 1,
        max_attempts: 3,
        delay_ms: 1000,
        message: "retry".into(),
        error_code: None,
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("maxAttempts"), "camelCase: {}", json);
    assert!(json.contains("delayMs"), "camelCase: {}", json);
}

#[test]
fn provider_error_payload_dto_roundtrip() {
    let dto = ProviderErrorPayloadDto {
        code: ProviderErrorCodeDto::RateLimited,
        message: "rate limited".into(),
        retry_after_ms: Some(5000),
        retryable: true,
        http_status: None,
        provider: None,
        provider_code: None,
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("retryAfterMs"), "camelCase: {}", json);
}

#[test]
fn agent_mode_dto_roundtrip() {
    let dto = AgentModeDto {
        id: "mode1".into(),
        name: "Agent".into(),
        description: "agent mode".into(),
        kind: AgentModeKindDto::Agent,
        prompt: "prompt".into(),
        model: None,
        temperature: None,
        max_tokens: None,
        reasoning_enabled: None,
        reasoning_effort: None,
        presentation_rank: 0,
        is_builtin: true,
        created_at: 1000,
        updated_at: 1000,
        tool_config: ConversationToolConfigDto::default(),
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("presentationRank"), "camelCase: {}", json);
    assert!(json.contains("isBuiltin"), "camelCase: {}", json);
    assert!(json.contains("toolConfig"), "camelCase: {}", json);
}

fn mode_row_without_tool_config(id: &str) -> AgentModeRow {
    AgentModeRow {
        id: id.into(),
        name: "mode".into(),
        description: String::new(),
        kind: "agent".into(),
        prompt: String::new(),
        model: None,
        temperature: None,
        max_tokens: None,
        reasoning_enabled: None,
        reasoning_effort: None,
        presentation_rank: 0,
        is_builtin: true,
        created_at: 1000,
        updated_at: 1000,
        tool_config_json: None,
    }
}

#[test]
fn agent_mode_tool_config_falls_back_to_contract_default() {
    let dto = AgentModeDto::from_row(&mode_row_without_tool_config("builtin-agent"));
    assert_eq!(dto.tool_config, ConversationToolConfigDto::default());
}

#[test]
fn provider_list_dto_roundtrip() {
    let dto = ProviderListDto {
        id: "p1".into(),
        name: "OpenAI".into(),
        type_: "openai".into(),
        base_url: "https://api.openai.com".into(),
        api_key_masked: "sk-****1234".into(),
        models: vec![ModelConfigDto {
            id: "gpt-4".into(),
            name: "GPT-4".into(),
            owned_by: "openai".into(),
            context_window: Some(128000),
            support_vision: true,
            input_modalities: vec!["text".into(), "image".into()],
            reasoning_levels: Some(ReasoningLevelsDto {
                toggle: false,
                efforts: vec!["low".into(), "medium".into(), "high".into()],
            }),
        }],
        default_model: "gpt-4".into(),
        updated_at: 1000,
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("baseUrl"), "camelCase: {}", json);
    assert!(json.contains("apiKeyMasked"), "camelCase: {}", json);
    assert!(json.contains("inputModalities"), "camelCase: {}", json);
    assert!(json.contains("defaultModel"), "camelCase: {}", json);
    assert!(json.contains("reasoningLevels"), "camelCase: {}", json);
}

#[test]
fn model_config_dto_accepts_legacy_models_json() {
    let legacy = r#"[{"id":"gpt-4","name":"GPT-4","ownedBy":"openai","contextWindow":128000,"supportVision":true,"inputModalities":["text","image"]}]"#;
    let models: Vec<ModelConfigDto> = serde_json::from_str(legacy).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].reasoning_levels, None);
}

#[test]
fn conversation_detail_dto_roundtrip() {
    let dto = ConversationDetailDto {
        conversation: ConversationDto {
            id: "c1".into(),
            title: "Test".into(),
            title_source: "placeholder".into(),
            provider_id: None,
            model_id: "gpt-4".into(),
            system_prompt: None,
            is_pinned: false,
            sort_order: 0,
            generation: 0,
            created_at: 1000,
            updated_at: 1000,
            is_full: false,
            active_branch_id: None,
            mode_id: None,
            workspace_root: String::new(),
            tool_config: None,
        },
        messages: vec![],
        total_count: 0,
    };
    roundtrip(&dto);
}

#[test]
fn event_payload_dtos_roundtrip() {
    let start = ChatStreamStartPayloadDto {
        conversation_id: "c1".into(),
        attempt_id: "a1".into(),
        message_id: "m1".into(),
        phase: TurnPhaseDto::Thinking,
    };
    roundtrip(&start);

    let terminated = ConversationTerminatedPayloadDto {
        conversation_id: "c1".into(),
        is_full: true,
    };
    roundtrip(&terminated);

    let unlocked = ConversationUnlockedPayloadDto {
        conversation_id: "c1".into(),
    };
    roundtrip(&unlocked);

    let regenerate = ChatRegeneratePayloadDto {
        conversation_id: "c1".into(),
        deleted_message_id: "m1".into(),
    };
    roundtrip(&regenerate);

    for state in [
        SessionStateDto::Idle,
        SessionStateDto::Processing {
            turn_id: "t1".into(),
            phase: TurnPhaseDto::ToolCalling,
        },
        SessionStateDto::Error {
            message: "boom".into(),
            recoverable: true,
        },
    ] {
        let session_state = SessionStateChangedPayloadDto {
            conversation_id: "c1".into(),
            turn_id: "t1".into(),
            state,
        };
        roundtrip(&session_state);
    }
}

#[test]
fn setting_dtos_roundtrip() {
    let provider_changed = SettingProviderChangedPayloadDto {
        action: SettingProviderChangedActionDto::ProviderSaved,
        id: None,
    };
    roundtrip(&provider_changed);

    let shortcut = ShortcutWarningPayloadDto {
        id: "s1".into(),
        binding: "Ctrl+K".into(),
        error: "conflict".into(),
    };
    roundtrip(&shortcut);

    let theme = ThemeChangedPayloadDto {
        resolved: "dark".into(),
    };
    roundtrip(&theme);
}

#[test]
fn orb_dtos_roundtrip() {
    let mouse = MousePositionDto {
        client_x: 100,
        client_y: 200,
    };
    roundtrip(&mouse);
    let json = serde_json::to_string(&mouse).unwrap();
    assert!(json.contains("clientX"), "camelCase: {}", json);

    let screen = ScreenSizeDto {
        screen: ScreenRectDto {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
    };
    roundtrip(&screen);
}

#[test]
fn os_tool_dtos_roundtrip() {
    let request = OsToolExecRequestDto {
        tool: "fs_link".into(),
        action: "create".into(),
        params: serde_json::json!({ "source": "a", "target": "b", "kind": "symlink" }),
    };
    roundtrip(&request);

    let ok = OsToolResultDto {
        ok: true,
        data: Some(serde_json::json!({ "created": true })),
        result_for_assistant: Some("created".into()),
        error: None,
        error_kind: None,
    };
    roundtrip(&ok);
    let json = serde_json::to_string(&ok).unwrap();
    assert!(
        json.contains("\"error\":null"),
        "None fields present: {}",
        json
    );
    assert!(
        json.contains("\"errorKind\":null"),
        "None fields present: {}",
        json
    );

    let failed = OsToolResultDto {
        ok: false,
        data: None,
        result_for_assistant: None,
        error: Some("boom".into()),
        error_kind: Some(ToolErrorKindDto::Internal),
    };
    roundtrip(&failed);
}

#[test]
fn os_tool_exec_request_rejects_unknown_fields() {
    let err = serde_json::from_str::<OsToolExecRequestDto>(
        r#"{"tool":"fs_link","action":"create","params":{},"extra":1}"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("unknown field"), "{}", err);
}

#[test]
fn agent_tool_call_dto_roundtrip() {
    let base = |payload: AgentToolCallPayloadDto| AgentToolCallDto {
        conversation_id: "c1".into(),
        turn_id: "t1".into(),
        iteration: 2,
        call_id: "call_1".into(),
        name: "fs_read".into(),
        payload,
    };

    let started = base(AgentToolCallPayloadDto::Started);
    roundtrip(&started);
    let json = serde_json::to_string(&started).unwrap();
    assert!(json.contains("\"phase\":\"started\""), "tag: {}", json);
    assert!(
        json.contains("\"callId\":\"call_1\""),
        "camelCase: {}",
        json
    );
    assert!(
        json.contains("\"conversationId\":\"c1\""),
        "camelCase: {}",
        json
    );
    assert!(!json.contains("call_id"), "no snake_case: {}", json);

    let args = base(AgentToolCallPayloadDto::ArgsDelta {
        delta: "{\"path\":".into(),
    });
    roundtrip(&args);

    let finished = base(AgentToolCallPayloadDto::Finished {
        result: "ok".into(),
        confirmation_wait_ms: Some(120),
    });
    roundtrip(&finished);
    let json = serde_json::to_string(&finished).unwrap();
    assert!(
        json.contains("\"confirmationWaitMs\":120"),
        "explicit rename: {}",
        json
    );

    let finished_without_wait = base(AgentToolCallPayloadDto::Finished {
        result: "ok".into(),
        confirmation_wait_ms: None,
    });
    roundtrip(&finished_without_wait);
    let json = serde_json::to_string(&finished_without_wait).unwrap();
    assert!(
        json.contains("\"confirmationWaitMs\":null"),
        "None field present: {}",
        json
    );

    let failed = base(AgentToolCallPayloadDto::Failed {
        error: "boom".into(),
        error_kind: ToolErrorKindDto::Denied,
        confirmation_wait_ms: Some(7),
    });
    roundtrip(&failed);
    let json = serde_json::to_string(&failed).unwrap();
    assert!(
        json.contains("\"errorKind\":\"denied\""),
        "explicit rename: {}",
        json
    );

    let failed_without_wait = base(AgentToolCallPayloadDto::Failed {
        error: "boom".into(),
        error_kind: ToolErrorKindDto::Internal,
        confirmation_wait_ms: None,
    });
    roundtrip(&failed_without_wait);
}

#[test]
fn agent_tool_call_dto_rejects_unknown_fields() {
    let err = serde_json::from_str::<AgentToolCallDto>(
        r#"{"conversationId":"c1","turnId":"t1","iteration":1,"callId":"c","name":"n","payload":{"phase":"started"},"extra":1}"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("unknown field"), "{}", err);
}

#[test]
fn install_mascot_asset_result_dto_roundtrip() {
    let dto = InstallMascotAssetResultDto {
        installed_path: "mascot/rive/custom.riv".into(),
        bytes: 1024,
        version: 1700000000000,
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("installedPath"), "camelCase: {}", json);
}

#[test]
fn output_optional_fields_are_present_as_null() {
    let dto = ChatDeltaPayloadDto {
        conversation_id: "c1".into(),
        attempt_id: "a1".into(),
        message_id: "m1".into(),
        seq: 1,
        iteration: 1,
        delta_text: None,
        delta_reasoning: None,
        finish_reason: None,
        usage: None,
        delta_retry: None,
        error: None,
    };
    let json = serde_json::to_string(&dto).unwrap();
    assert!(
        json.contains("\"deltaText\":null"),
        "None fields present: {}",
        json
    );
    assert!(
        json.contains("\"usage\":null"),
        "None fields present: {}",
        json
    );
}

#[test]
fn conversation_patch_dto_roundtrip() {
    let dto = ConversationPatchDto {
        title: Some("new title".into()),
        title_source: Some("user".into()),
        model_id: None,
        provider_id: Some("p1".into()),
        mode_id: Some("builtin-default".into()),
        tool_config: None,
        workspace_root: None,
        clear_workspace_root: None,
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("titleSource"), "camelCase: {}", json);
    assert!(json.contains("providerId"), "camelCase: {}", json);
    assert!(json.contains("\"modelId\":null"), "None present: {}", json);
    assert!(
        json.contains("\"modeId\":\"builtin-default\""),
        "camelCase: {}",
        json
    );
}

#[test]
fn conversation_query_opts_dto_roundtrip() {
    let dto = ConversationQueryOptsDto {
        mode: ConversationQueryModeDto::Paginated,
        limit: 50,
        before_timestamp: Some(1000),
    };
    roundtrip(&dto);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("beforeTimestamp"), "camelCase: {}", json);
}

#[test]
fn send_chat_message_args_dto_minimal() {
    let args: SendChatMessageArgsDto =
        serde_json::from_str(r#"{"content":"x","providerId":"p","modelId":"m"}"#).unwrap();
    assert!(args.mode_id.is_none());
    assert!(args.branch_point_message_id.is_none());
    assert!(args.reasoning_level.is_none());
}

#[test]
fn send_chat_message_args_dto_missing_required() {
    let err = serde_json::from_str::<SendChatMessageArgsDto>(r#"{"content":"x","modelId":"m"}"#)
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("missing field"), "{}", msg);
    assert!(msg.contains("providerId"), "{}", msg);
}

#[test]
fn send_chat_message_args_dto_rejects_unknown_fields() {
    let err = serde_json::from_str::<SendChatMessageArgsDto>(
        r#"{"content":"x","providerId":"p","modelId":"m","branchPointMsgId":"b"}"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("unknown field"), "{}", err);
}
