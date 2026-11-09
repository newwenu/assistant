use assistant_app::protocol::{
    ChatDeltaPayloadDto, ChatMessagesChangedPayloadDto, ConversationDetailDto, ConversationDto,
    MessageChangeKindDto, MessageDto, ProviderErrorPayloadDto, RetryNotificationDto,
    SessionStateChangedPayloadDto, SessionStateDto, TokenUsageDto, TurnPhaseDto,
};
use assistant_providers::base::{ProviderErrorCode, ProviderErrorPayload};
use assistant_providers::types::chat_delta::{RetryNotification, TokenUsage};
use assistant_storage::conversations::{ConversationRow, MessageRow};

fn sample_conversation_row() -> ConversationRow {
    ConversationRow {
        id: "conv-1".into(),
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
        tool_config_json: None,
        workspace_root: String::new(),
    }
}

fn sample_message_row() -> MessageRow {
    MessageRow {
        id: "msg-1".into(),
        conversation_id: "conv-1".into(),
        role: "user".into(),
        content: "hello".into(),
        tokens: None,
        timestamp: 1000,
        parent_id: None,
        is_pinned: false,
        reasoning: None,
        tool_calls_json: None,
        sibling_index: 0,
        provider_id: None,
        model_id: None,
        tool_call_id: None,
        is_error: false,
        status: "completed".to_string(),
    }
}

fn sample_token_usage() -> TokenUsage {
    TokenUsage {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
        cache_read_tokens: None,
        cache_write_tokens: None,
        reasoning_tokens: None,
        context_window_tokens: None,
    }
}

fn sample_error_payload() -> ProviderErrorPayload {
    ProviderErrorPayload {
        code: ProviderErrorCode::RateLimited,
        message: "rate limited".into(),
        retry_after_ms: Some(1000),
        retryable: true,
        http_status: None,
        provider: None,
        provider_code: None,
    }
}

fn sample_retry_notification() -> RetryNotification {
    RetryNotification {
        attempt: 1,
        max_attempts: 3,
        delay_ms: 500,
        message: "retrying".into(),
        error_code: None,
    }
}

fn sample_chat_delta_dto() -> ChatDeltaPayloadDto {
    ChatDeltaPayloadDto {
        conversation_id: "c".into(),
        attempt_id: "a".into(),
        message_id: "m".into(),
        seq: 0,
        iteration: 0,
        delta_text: None,
        delta_reasoning: None,
        finish_reason: None,
        usage: None,
        delta_retry: None,
        error: None,
    }
}

#[test]
fn chat_delta_payload_dto_keeps_null_optional_fields() {
    let payload = ChatDeltaPayloadDto {
        ..sample_chat_delta_dto()
    };
    let v = serde_json::to_value(&payload).unwrap();
    let obj = v.as_object().unwrap();

    assert_eq!(obj["error"], serde_json::Value::Null);
    assert_eq!(obj["deltaRetry"], serde_json::Value::Null);
}

#[test]
fn chat_delta_payload_dto_error_field_present_when_some() {
    let payload = ChatDeltaPayloadDto {
        error: Some(ProviderErrorPayloadDto::from(sample_error_payload())),
        ..sample_chat_delta_dto()
    };
    let v = serde_json::to_value(&payload).unwrap();
    let obj = v.as_object().unwrap();

    assert!(
        obj.contains_key("error"),
        "error should be present when Some"
    );
    let error_obj = obj["error"].as_object().unwrap();
    assert!(error_obj.contains_key("code"), "error.code missing");
    assert!(error_obj.contains_key("message"), "error.message missing");
    assert!(
        error_obj.contains_key("retryAfterMs"),
        "error.retryAfterMs missing"
    );
    assert!(
        error_obj.contains_key("retryable"),
        "error.retryable missing"
    );
}

#[test]
fn conversation_dto_outputs_camel_case() {
    let dto = ConversationDto::from(sample_conversation_row());
    let v = serde_json::to_value(&dto).unwrap();
    let obj = v.as_object().unwrap();

    assert!(
        obj.contains_key("titleSource"),
        "titleSource should be camelCase"
    );
    assert!(
        obj.contains_key("providerId"),
        "providerId should be camelCase"
    );
    assert!(obj.contains_key("modelId"), "modelId should be camelCase");
    assert!(
        obj.contains_key("systemPrompt"),
        "systemPrompt should be camelCase"
    );
    assert!(obj.contains_key("isPinned"), "isPinned should be camelCase");
    assert!(
        obj.contains_key("sortOrder"),
        "sortOrder should be camelCase"
    );
    assert!(
        obj.contains_key("createdAt"),
        "createdAt should be camelCase"
    );
    assert!(
        obj.contains_key("updatedAt"),
        "updatedAt should be camelCase"
    );
    assert!(obj.contains_key("isFull"), "isFull should be camelCase");

    assert!(
        !obj.contains_key("title_source"),
        "no snake_case in wire format"
    );
    assert!(
        !obj.contains_key("provider_id"),
        "no snake_case in wire format"
    );
    assert!(
        !obj.contains_key("created_at"),
        "no snake_case in wire format"
    );
}

#[test]
fn chat_delta_payload_dto_round_trip() {
    let original = ChatDeltaPayloadDto {
        conversation_id: "c".into(),
        attempt_id: "a".into(),
        message_id: "m".into(),
        seq: 42,
        iteration: 2,
        delta_text: Some("hello".into()),
        delta_reasoning: Some("thinking".into()),
        finish_reason: Some("stop".into()),
        usage: Some(TokenUsageDto::from(sample_token_usage())),
        delta_retry: Some(RetryNotificationDto::from(sample_retry_notification())),
        error: Some(ProviderErrorPayloadDto::from(sample_error_payload())),
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(restored["conversationId"], "c");
    assert_eq!(restored["attemptId"], "a");
    assert_eq!(restored["messageId"], "m");
    assert_eq!(restored["seq"], 42);
    assert_eq!(restored["iteration"], 2);
    assert_eq!(restored["deltaText"], "hello");
    assert_eq!(restored["deltaReasoning"], "thinking");
    assert_eq!(restored["finishReason"], "stop");
    assert_eq!(restored["usage"]["promptTokens"], 10);
    assert_eq!(restored["usage"]["totalTokens"], 30);
    assert_eq!(restored["deltaRetry"]["attempt"], 1);
    assert_eq!(restored["deltaRetry"]["delayMs"], 500);
    assert_eq!(restored["error"]["code"], "RateLimited");
    assert_eq!(restored["error"]["retryable"], true);
}

#[test]
fn conversation_detail_dto_wire_format() {
    let dto = ConversationDetailDto {
        conversation: ConversationDto::from(sample_conversation_row()),
        messages: vec![MessageDto::from(sample_message_row())],
        total_count: 1,
    };
    let v = serde_json::to_value(&dto).unwrap();
    let obj = v.as_object().unwrap();

    assert!(obj.contains_key("conversation"));
    assert!(obj.contains_key("messages"));
    assert!(
        obj.contains_key("totalCount"),
        "totalCount should be camelCase"
    );
    assert!(
        !obj.contains_key("total_count"),
        "no snake_case in wire format"
    );

    let conv = obj["conversation"].as_object().unwrap();
    assert!(
        conv.contains_key("providerId"),
        "providerId should be camelCase"
    );
    assert!(
        !conv.contains_key("provider_id"),
        "no snake_case in wire format"
    );

    let msgs = obj["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert!(
        msgs[0].as_object().unwrap().contains_key("conversationId"),
        "conversationId should be camelCase"
    );
}

#[test]
fn chat_messages_changed_payload_uses_seq_key() {
    let payload = ChatMessagesChangedPayloadDto {
        conversation_id: "conv-1".into(),
        change_kind: MessageChangeKindDto::BranchSwitched,
        messages: vec![],
        active_branch_id: None,
        seq: Some(1),
    };
    let v = serde_json::to_value(&payload).unwrap();
    assert_eq!(v["seq"], 1, "seq must survive camelCase renaming");
    assert!(v.get("_seq").is_none(), "wire key must be seq, not _seq");
}

#[test]
fn provider_error_code_serializes_to_pascal_case() {
    let v = serde_json::to_value(&ProviderErrorCode::RateLimited).unwrap();
    assert_eq!(
        v, "RateLimited",
        "ProviderErrorCode serializes as PascalCase by default (no rename_all)"
    );

    let v2 = serde_json::to_value(&ProviderErrorCode::ApiKeyMissing).unwrap();
    assert_eq!(v2, "ApiKeyMissing");

    let v3 = serde_json::to_value(&ProviderErrorCode::ContextOverflow).unwrap();
    assert_eq!(v3, "ContextOverflow");
}

#[test]
fn provider_error_code_round_trip() {
    let codes = vec![
        ProviderErrorCode::ApiKeyMissing,
        ProviderErrorCode::AuthFailed,
        ProviderErrorCode::RateLimited,
        ProviderErrorCode::ModelNotFound,
        ProviderErrorCode::ContextOverflow,
        ProviderErrorCode::NetworkError,
        ProviderErrorCode::Timeout,
        ProviderErrorCode::ServerError,
        ProviderErrorCode::RequestRejected,
        ProviderErrorCode::ParseError,
        ProviderErrorCode::Cancelled,
        ProviderErrorCode::StreamError,
        ProviderErrorCode::InternalError,
    ];
    for code in &codes {
        let json = serde_json::to_string(code).unwrap();
        let restored: ProviderErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(*code, restored, "round-trip failed for {:?}", code);
    }
}

#[test]
fn token_usage_dto_serde_default_fields() {
    let json = r#"{"promptTokens":5,"completionTokens":10,"totalTokens":15}"#;
    let usage: TokenUsageDto = serde_json::from_str(json).unwrap();
    assert_eq!(usage.prompt_tokens, 5);
    assert_eq!(usage.completion_tokens, 10);
    assert_eq!(usage.total_tokens, 15);
    assert_eq!(
        usage.cache_read_tokens, None,
        "cacheReadTokens should default to None (provider did not report cache stats)"
    );
    assert_eq!(
        usage.cache_write_tokens, None,
        "cacheWriteTokens should default to None (provider did not report cache stats)"
    );
    assert_eq!(
        usage.context_window_tokens, None,
        "contextWindowTokens should default to None (window unknown)"
    );
    let serialized = serde_json::to_string(&usage).unwrap();
    assert!(
        serialized.contains("\"cacheReadTokens\":null"),
        "cacheReadTokens must be present as null: {}",
        serialized
    );
    assert!(
        serialized.contains("\"cacheWriteTokens\":null"),
        "cacheWriteTokens must be present as null: {}",
        serialized
    );
}

#[test]
fn token_usage_dto_round_trip() {
    let usage = TokenUsageDto::from(sample_token_usage());
    let json = serde_json::to_string(&usage).unwrap();
    let restored: TokenUsageDto = serde_json::from_str(&json).unwrap();
    assert_eq!(usage.prompt_tokens, restored.prompt_tokens);
    assert_eq!(usage.completion_tokens, restored.completion_tokens);
    assert_eq!(usage.total_tokens, restored.total_tokens);
    assert_eq!(usage.cache_read_tokens, restored.cache_read_tokens);
    assert_eq!(usage.cache_write_tokens, restored.cache_write_tokens);
    assert_eq!(usage.context_window_tokens, restored.context_window_tokens);
}

#[test]
fn turn_phase_dto_serializes_identically_to_kernel_turn_phase() {
    use assistant_agent::kernel::TurnPhase;

    let pairs = [
        (TurnPhase::Starting, TurnPhaseDto::Starting),
        (TurnPhase::Compacting, TurnPhaseDto::Compacting),
        (TurnPhase::Thinking, TurnPhaseDto::Thinking),
        (TurnPhase::Streaming, TurnPhaseDto::Streaming),
        (TurnPhase::ToolCalling, TurnPhaseDto::ToolCalling),
        (TurnPhase::AwaitingApproval, TurnPhaseDto::AwaitingApproval),
        (TurnPhase::Finalizing, TurnPhaseDto::Finalizing),
    ];

    for (kernel, dto) in pairs {
        assert_eq!(
            serde_json::to_value(kernel).unwrap(),
            serde_json::to_value(dto).unwrap(),
            "wire value drifted from kernel for {kernel:?}"
        );
        assert_eq!(TurnPhaseDto::from(kernel), dto, "From mapping drifted");
    }
}

#[test]
fn session_state_changed_payload_wire_shape() {
    let processing = SessionStateChangedPayloadDto {
        conversation_id: "c1".into(),
        turn_id: "t1".into(),
        state: SessionStateDto::Processing {
            turn_id: "t1".into(),
            phase: TurnPhaseDto::ToolCalling,
        },
    };
    let v = serde_json::to_value(&processing).unwrap();

    assert_eq!(v["conversationId"], "c1");
    assert_eq!(v["turnId"], "t1");
    assert_eq!(v["state"]["processing"]["turnId"], "t1");
    assert_eq!(v["state"]["processing"]["phase"], "toolCalling");
    assert!(
        v.get("seq").is_none(),
        "session-state-changed is a snapshot, not a stream fragment: {v}"
    );

    let idle = serde_json::to_value(SessionStateDto::Idle).unwrap();
    assert_eq!(idle, "idle", "unit variant must serialize as bare string");

    let error = serde_json::to_value(SessionStateDto::Error {
        message: "boom".into(),
        recoverable: true,
    })
    .unwrap();
    assert_eq!(error["error"]["message"], "boom");
    assert_eq!(error["error"]["recoverable"], true);
}

#[test]
fn session_state_dto_round_trips_through_json() {
    let states = [
        SessionStateDto::Idle,
        SessionStateDto::Processing {
            turn_id: "t1".into(),
            phase: TurnPhaseDto::Finalizing,
        },
        SessionStateDto::Error {
            message: "boom".into(),
            recoverable: false,
        },
    ];

    for state in states {
        let payload = SessionStateChangedPayloadDto {
            conversation_id: "c1".into(),
            turn_id: "t1".into(),
            state,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let restored: SessionStateChangedPayloadDto = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, payload, "round-trip failed for {json}");
    }
}
