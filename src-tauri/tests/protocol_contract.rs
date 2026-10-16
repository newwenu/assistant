use assistant_app::protocol::{
    ChatDeltaPayloadDto, ConversationDetailDto, ConversationDto, MessageDto, TokenUsageDto,
    RetryNotificationDto, ProviderErrorPayloadDto,
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
    }
}

fn sample_token_usage() -> TokenUsage {
    TokenUsage {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        context_window_tokens: 0,
    }
}

fn sample_error_payload() -> ProviderErrorPayload {
    ProviderErrorPayload {
        code: ProviderErrorCode::RateLimited,
        message: "rate limited".into(),
        retry_after_ms: Some(1000),
        retryable: true,
    }
}

fn sample_retry_notification() -> RetryNotification {
    RetryNotification {
        attempt: 1,
        max_attempts: 3,
        delay_ms: 500,
        message: "retrying".into(),
    }
}

fn sample_chat_delta_dto() -> ChatDeltaPayloadDto {
    ChatDeltaPayloadDto {
        conversation_id: "c".into(),
        attempt_id: "a".into(),
        message_id: "m".into(),
        seq: 0,
        delta_text: None,
        delta_reasoning: None,
        delta_tool_call_id: None,
        delta_tool_call_name: None,
        delta_tool_call_arguments: None,
        finish_reason: None,
        usage: None,
        delta_retry: None,
        error: None,
    }
}

#[test]
fn chat_delta_payload_dto_skip_serializing_none() {
    let payload = ChatDeltaPayloadDto {
        ..sample_chat_delta_dto()
    };
    let v = serde_json::to_value(&payload).unwrap();
    let obj = v.as_object().unwrap();

    assert!(!obj.contains_key("error"), "error should be skipped when None");
    assert!(!obj.contains_key("deltaRetry"), "deltaRetry should be skipped when None");
}

#[test]
fn chat_delta_payload_dto_error_field_present_when_some() {
    let payload = ChatDeltaPayloadDto {
        error: Some(ProviderErrorPayloadDto::from(sample_error_payload())),
        ..sample_chat_delta_dto()
    };
    let v = serde_json::to_value(&payload).unwrap();
    let obj = v.as_object().unwrap();

    assert!(obj.contains_key("error"), "error should be present when Some");
    let error_obj = obj["error"].as_object().unwrap();
    assert!(error_obj.contains_key("code"), "error.code missing");
    assert!(error_obj.contains_key("message"), "error.message missing");
    assert!(error_obj.contains_key("retryAfterMs"), "error.retryAfterMs missing");
    assert!(error_obj.contains_key("retryable"), "error.retryable missing");
}

#[test]
fn conversation_dto_outputs_camel_case() {
    let dto = ConversationDto::from(sample_conversation_row());
    let v = serde_json::to_value(&dto).unwrap();
    let obj = v.as_object().unwrap();

    assert!(obj.contains_key("titleSource"), "titleSource should be camelCase");
    assert!(obj.contains_key("providerId"), "providerId should be camelCase");
    assert!(obj.contains_key("modelId"), "modelId should be camelCase");
    assert!(obj.contains_key("systemPrompt"), "systemPrompt should be camelCase");
    assert!(obj.contains_key("isPinned"), "isPinned should be camelCase");
    assert!(obj.contains_key("sortOrder"), "sortOrder should be camelCase");
    assert!(obj.contains_key("createdAt"), "createdAt should be camelCase");
    assert!(obj.contains_key("updatedAt"), "updatedAt should be camelCase");
    assert!(obj.contains_key("isFull"), "isFull should be camelCase");

    assert!(!obj.contains_key("title_source"), "no snake_case in wire format");
    assert!(!obj.contains_key("provider_id"), "no snake_case in wire format");
    assert!(!obj.contains_key("created_at"), "no snake_case in wire format");
}

#[test]
fn chat_delta_payload_dto_round_trip() {
    let original = ChatDeltaPayloadDto {
        conversation_id: "c".into(),
        attempt_id: "a".into(),
        message_id: "m".into(),
        seq: 42,
        delta_text: Some("hello".into()),
        delta_reasoning: Some("thinking".into()),
        delta_tool_call_id: None,
        delta_tool_call_name: None,
        delta_tool_call_arguments: None,
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
    assert!(obj.contains_key("totalCount"), "totalCount should be camelCase");
    assert!(!obj.contains_key("total_count"), "no snake_case in wire format");

    let conv = obj["conversation"].as_object().unwrap();
    assert!(conv.contains_key("providerId"), "providerId should be camelCase");
    assert!(!conv.contains_key("provider_id"), "no snake_case in wire format");

    let msgs = obj["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].as_object().unwrap().contains_key("conversationId"), "conversationId should be camelCase");
}

#[test]
fn provider_error_code_serializes_to_pascal_case() {
    let v = serde_json::to_value(&ProviderErrorCode::RateLimited).unwrap();
    assert_eq!(v, "RateLimited", "ProviderErrorCode serializes as PascalCase by default (no rename_all)");

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
    assert_eq!(usage.cache_read_tokens, 0, "cacheReadTokens should default to 0");
    assert_eq!(usage.cache_write_tokens, 0, "cacheWriteTokens should default to 0");
    assert_eq!(usage.context_window_tokens, 0, "contextWindowTokens should default to 0");
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