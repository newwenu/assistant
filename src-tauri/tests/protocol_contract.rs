use assistant_app::protocol::{ChatDeltaPayload, ChatRegeneratePayload, MousePosition, ScreenRect, ScreenSize};
use assistant_providers::base::{ProviderErrorCode, ProviderErrorPayload};
use assistant_providers::types::chat_delta::{RetryNotification, TokenUsage};
use assistant_storage::conversations::{ConversationRow, MessageRow};
use assistant_storage::providers::ProviderRow;
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

fn sample_chat_delta_payload() -> ChatDeltaPayload {
    ChatDeltaPayload {
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
fn chat_delta_payload_skip_serializing_none() {
    let payload = ChatDeltaPayload {
        ..sample_chat_delta_payload()
    };
    let v = serde_json::to_value(&payload).unwrap();
    let obj = v.as_object().unwrap();

    assert!(!obj.contains_key("error"), "error should be skipped when None");
    assert!(!obj.contains_key("delta_retry"), "delta_retry should be skipped when None");
}

#[test]
fn chat_delta_payload_error_field_present_when_some() {
    let payload = ChatDeltaPayload {
        error: Some(sample_error_payload()),
        ..sample_chat_delta_payload()
    };
    let v = serde_json::to_value(&payload).unwrap();
    let obj = v.as_object().unwrap();

    assert!(obj.contains_key("error"), "error should be present when Some");
    let error_obj = obj["error"].as_object().unwrap();
    assert!(error_obj.contains_key("code"), "error.code missing");
    assert!(error_obj.contains_key("message"), "error.message missing");
    assert!(error_obj.contains_key("retry_after_ms"), "error.retry_after_ms missing");
    assert!(error_obj.contains_key("retryable"), "error.retryable missing");
}

#[test]
fn conversation_row_no_camel_case_leak() {
    let row = sample_conversation_row();
    let v = serde_json::to_value(&row).unwrap();
    let obj = v.as_object().unwrap();

    let camel_keys: Vec<&String> = obj
        .keys()
        .filter(|k| k.chars().any(|c| c.is_ascii_uppercase()))
        .collect();
    assert!(
        camel_keys.is_empty(),
        "ConversationRow has camelCase keys in wire format: {:?}",
        camel_keys
    );
}

#[test]
fn chat_delta_payload_round_trip() {
    let original = ChatDeltaPayload {
        conversation_id: "c".into(),
        attempt_id: "a".into(),
        message_id: "m".into(),
        seq: 42,
        delta_text: Some("hello".into()),
        delta_reasoning: Some("thinking".into()),
        finish_reason: Some("stop".into()),
        usage: Some(sample_token_usage()),
        delta_retry: Some(sample_retry_notification()),
        error: Some(sample_error_payload()),
        ..Default::default()
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(restored["conversation_id"], "c");
    assert_eq!(restored["attempt_id"], "a");
    assert_eq!(restored["message_id"], "m");
    assert_eq!(restored["seq"], 42);
    assert_eq!(restored["delta_text"], "hello");
    assert_eq!(restored["delta_reasoning"], "thinking");
    assert_eq!(restored["finish_reason"], "stop");
    assert_eq!(restored["usage"]["prompt_tokens"], 10);
    assert_eq!(restored["usage"]["total_tokens"], 30);
    assert_eq!(restored["delta_retry"]["attempt"], 1);
    assert_eq!(restored["delta_retry"]["delay_ms"], 500);
    assert_eq!(restored["error"]["code"], "RateLimited");
    assert_eq!(restored["error"]["retryable"], true);
}

#[test]
fn conversation_detail_response_wire_format() {
    use assistant_app::protocol::ConversationDetailResponse;

    let resp = ConversationDetailResponse {
        conversation: sample_conversation_row(),
        messages: vec![sample_message_row()],
        total_count: 1,
    };
    let v = serde_json::to_value(&resp).unwrap();
    let obj = v.as_object().unwrap();

    assert!(obj.contains_key("conversation"));
    assert!(obj.contains_key("messages"));
    assert!(obj.contains_key("total_count"));
    assert!(!obj.contains_key("totalCount"));

    let conv = obj["conversation"].as_object().unwrap();
    assert!(conv.contains_key("provider_id"));
    assert!(!conv.contains_key("providerId"));

    let msgs = obj["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert!(msgs[0].as_object().unwrap().contains_key("conversation_id"));
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
fn token_usage_serde_default_fields() {
    let json = r#"{"prompt_tokens":5,"completion_tokens":10,"total_tokens":15}"#;
    let usage: TokenUsage = serde_json::from_str(json).unwrap();
    assert_eq!(usage.prompt_tokens, 5);
    assert_eq!(usage.completion_tokens, 10);
    assert_eq!(usage.total_tokens, 15);
    assert_eq!(usage.cache_read_tokens, 0, "cache_read_tokens should default to 0");
    assert_eq!(usage.cache_write_tokens, 0, "cache_write_tokens should default to 0");
    assert_eq!(usage.context_window_tokens, 0, "context_window_tokens should default to 0");
}

#[test]
fn token_usage_round_trip() {
    let usage = sample_token_usage();
    let json = serde_json::to_string(&usage).unwrap();
    let restored: TokenUsage = serde_json::from_str(&json).unwrap();
    assert_eq!(usage.prompt_tokens, restored.prompt_tokens);
    assert_eq!(usage.completion_tokens, restored.completion_tokens);
    assert_eq!(usage.total_tokens, restored.total_tokens);
    assert_eq!(usage.cache_read_tokens, restored.cache_read_tokens);
    assert_eq!(usage.cache_write_tokens, restored.cache_write_tokens);
    assert_eq!(usage.context_window_tokens, restored.context_window_tokens);
}

#[test]
fn provider_row_wire_format_is_snake_case() {
    let row = ProviderRow {
        id: "p1".into(),
        name: "OpenAI".into(),
        type_: "openai".into(),
        base_url: "https://api.openai.com".into(),
        api_key_encrypted: "enc".into(),
        models_json: "[]".into(),
        default_model: "gpt-4".into(),
        updated_at: 1000,
    };
    let v = serde_json::to_value(&row).unwrap();
    let obj = v.as_object().unwrap();

    assert!(obj.contains_key("api_key_encrypted"));
    assert!(obj.contains_key("models_json"));
    assert!(obj.contains_key("default_model"));
    assert!(obj.contains_key("updated_at"));
    assert!(!obj.contains_key("apiKeyEncrypted"), "camelCase leak: apiKeyEncrypted");
    assert!(!obj.contains_key("modelsJson"), "camelCase leak: modelsJson");
    assert!(!obj.contains_key("defaultModel"), "camelCase leak: defaultModel");

    assert!(
        obj.contains_key("type_"),
        "ProviderRow.type_ serializes as type_ (Rust identifier), NOT type - cmd layer must use json! macro or serde rename"
    );
}

#[test]
fn message_row_no_camel_case_leak() {
    let row = sample_message_row();
    let v = serde_json::to_value(&row).unwrap();
    let obj = v.as_object().unwrap();

    let camel_keys: Vec<&String> = obj
        .keys()
        .filter(|k| k.chars().any(|c| c.is_ascii_uppercase()))
        .collect();
    assert!(
        camel_keys.is_empty(),
        "MessageRow has camelCase keys in wire format: {:?}",
        camel_keys
    );
}

#[test]
fn chat_regenerate_payload_wire_format() {
    let payload = ChatRegeneratePayload {
        conversation_id: "conv-1".into(),
        deleted_message_id: "msg-1".into(),
    };
    let v = serde_json::to_value(&payload).unwrap();
    let obj = v.as_object().unwrap();

    assert_eq!(obj["conversation_id"], "conv-1");
    assert_eq!(obj["deleted_message_id"], "msg-1");
    assert!(!obj.contains_key("deletedMessageId"), "camelCase leak: deletedMessageId");
}

#[test]
fn mouse_position_wire_format() {
    let pos = MousePosition {
        client_x: 100,
        client_y: 200,
    };
    let v = serde_json::to_value(&pos).unwrap();
    let obj = v.as_object().unwrap();

    assert_eq!(obj["client_x"], 100);
    assert_eq!(obj["client_y"], 200);
    assert!(!obj.contains_key("clientX"), "camelCase leak: clientX");
    assert!(!obj.contains_key("clientY"), "camelCase leak: clientY");
}

#[test]
fn screen_size_wire_format() {
    let size = ScreenSize {
        screen: ScreenRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
    };
    let v = serde_json::to_value(&size).unwrap();
    let obj = v.as_object().unwrap();

    let screen = obj["screen"].as_object().unwrap();
    assert_eq!(screen["x"], 0);
    assert_eq!(screen["y"], 0);
    assert_eq!(screen["width"], 1920);
    assert_eq!(screen["height"], 1080);
}