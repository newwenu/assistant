use assistant_providers::base::ProviderError;
use assistant_providers::client::quirks::*;

#[test]
fn url_detection_table() {
    let openai = "https://api.openai.com/v1/chat/completions";
    assert!(is_deepseek_url(
        "https://api.deepseek.com/v1/chat/completions"
    ));
    assert!(!is_deepseek_url(openai));

    assert!(is_zhipuai_url(
        "https://open.bigmodel.cn/api/paas/v4/chat/completions"
    ));
    assert!(is_zhipuai_url("https://api.z.ai/v1/chat/completions"));
    assert!(!is_zhipuai_url(openai));

    assert!(is_dashscope_url(
        "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
    ));
    assert!(is_dashscope_url(
        "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions"
    ));
    assert!(!is_dashscope_url(openai));

    assert!(is_siliconflow_url(
        "https://api.siliconflow.cn/v1/chat/completions"
    ));
    assert!(is_siliconflow_url(
        "https://api.siliconflow.com/v1/chat/completions"
    ));
    assert!(!is_siliconflow_url(openai));

    assert!(is_minimax_url(
        "https://api.minimax.chat/v1/chat/completions"
    ));
    assert!(!is_minimax_url(openai));

    assert!(is_glm_url(
        "https://open.bigmodel.cn/api/paas/v4/chat/completions",
        "glm-4"
    ));
    assert!(!is_glm_url(
        "https://open.bigmodel.cn/api/paas/v4/chat/completions",
        "chatglm"
    ));
    assert!(!is_glm_url(openai, "glm-4"));
}

#[test]
fn reasoning_effort_models() {
    assert!(is_deepseek_reasoning_effort_model("deepseek-v4-flash"));
    assert!(is_deepseek_reasoning_effort_model("deepseek-v4-pro"));
    assert!(!is_deepseek_reasoning_effort_model("deepseek-r1"));
    assert!(is_glm_reasoning_effort_model("glm-5.2"));
    assert!(is_glm_reasoning_effort_model("glm-5-2"));
    assert!(!is_glm_reasoning_effort_model("glm-4"));
}

#[test]
fn normalize_effort_table() {
    assert_eq!(
        normalize_deepseek_reasoning_effort("deepseek-v4-flash", "low"),
        Some("low")
    );
    assert_eq!(
        normalize_deepseek_reasoning_effort("deepseek-v4-pro", "low"),
        Some("high")
    );
    assert_eq!(
        normalize_deepseek_reasoning_effort("deepseek-v4-pro", "medium"),
        Some("high")
    );
    assert_eq!(
        normalize_deepseek_reasoning_effort("deepseek-v4-pro", "high"),
        Some("high")
    );
    assert_eq!(
        normalize_deepseek_reasoning_effort("deepseek-v4-pro", "max"),
        Some("max")
    );
    assert_eq!(
        normalize_deepseek_reasoning_effort("deepseek-v4-pro", "xhigh"),
        Some("max")
    );
    assert_eq!(
        normalize_deepseek_reasoning_effort("deepseek-v4-pro", "none"),
        None
    );
    assert_eq!(normalize_glm_reasoning_effort("high"), Some("high"));
    assert_eq!(normalize_glm_reasoning_effort("low"), Some("high"));
    assert_eq!(normalize_glm_reasoning_effort("max"), Some("max"));
    assert_eq!(normalize_glm_reasoning_effort("none"), None);
}

#[test]
fn parse_glm_version() {
    assert_eq!(parse_glm_major_minor("glm-4"), Some((4, 0)));
    assert_eq!(parse_glm_major_minor("glm-4.5"), Some((4, 5)));
    assert_eq!(parse_glm_major_minor("glm-5.2"), Some((5, 2)));
    assert_eq!(parse_glm_major_minor("chatglm"), None);
}

#[test]
fn tool_stream_rules() {
    assert!(should_append_tool_stream(
        "https://open.bigmodel.cn/api/paas/v4/chat/completions",
        "glm-4"
    ));
    assert!(should_append_tool_stream(
        "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        "glm-4.5"
    ));
    assert!(!should_append_tool_stream(
        "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        "glm-4"
    ));
    assert!(!should_append_tool_stream(
        "https://api.openai.com/v1/chat/completions",
        "gpt-4o"
    ));
}

#[test]
fn reasoning_toggle() {
    let mut body = serde_json::json!({});
    assert!(apply_reasoning_toggle(
        &mut body,
        true,
        "https://api.deepseek.com/v1/chat/completions"
    ));
    assert_eq!(body["thinking"]["type"], "enabled");

    let mut body = serde_json::json!({});
    assert!(apply_reasoning_toggle(
        &mut body,
        true,
        "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
    ));
    assert_eq!(body["enable_thinking"], true);

    let mut body = serde_json::json!({});
    assert!(apply_reasoning_toggle(
        &mut body,
        false,
        "https://api.siliconflow.cn/v1/chat/completions"
    ));
    assert_eq!(body["enable_thinking"], false);

    let mut body = serde_json::json!({});
    assert!(!apply_reasoning_toggle(
        &mut body,
        true,
        "https://api.openai.com/v1/chat/completions"
    ));
}

#[test]
fn bearer_auth_rules() {
    assert!(!wants_bearer_auth("https://api.anthropic.com/v1/messages"));
    assert!(wants_bearer_auth(
        "https://open.bigmodel.cn/api/paas/v4/chat/completions"
    ));
    assert!(wants_bearer_auth("https://api.kimi.com/coding/v1/models"));
    assert!(wants_bearer_auth(
        "https://api.moonshot.cn/anthropic/v1/messages"
    ));
    assert!(!wants_bearer_auth("https://api.moonshot.cn/v1/models"));
}

#[test]
fn valid_chunk_detection() {
    assert!(is_valid_chat_completion_chunk(
        &serde_json::json!({"object": "chat.completion.chunk"})
    ));
    assert!(is_valid_chat_completion_chunk(
        &serde_json::json!({"object": "chat.completion"})
    ));
    assert!(!is_valid_chat_completion_chunk(
        &serde_json::json!({"object": "ping"})
    ));
    assert!(!is_valid_chat_completion_chunk(
        &serde_json::json!({"id": "chatcmpl-123"})
    ));
}

#[test]
fn extract_sse_api_errors() {
    let ctx = serde_json::json!({"error": {"code": "context_length_exceeded", "message": "max 8192 tokens"}});
    assert!(matches!(
        extract_sse_api_error(&ctx).unwrap(),
        ProviderError::ContextOverflow(_)
    ));

    let rate = serde_json::json!({"error": {"type": "rate_limit_error", "message": "Rate limit"}});
    assert!(matches!(
        extract_sse_api_error(&rate).unwrap(),
        ProviderError::RateLimited { .. }
    ));

    let auth = serde_json::json!({"error": {"code": "authentication_error", "message": "Invalid API key"}});
    assert!(matches!(
        extract_sse_api_error(&auth).unwrap(),
        ProviderError::AuthFailed(_)
    ));

    let generic = serde_json::json!({"error": {"message": "Internal server error"}});
    assert!(matches!(
        extract_sse_api_error(&generic).unwrap(),
        ProviderError::StreamError(_)
    ));

    let string_err = serde_json::json!({"error": "something went wrong"});
    assert!(matches!(
        extract_sse_api_error(&string_err).unwrap(),
        ProviderError::StreamError(_)
    ));

    assert!(extract_sse_api_error(&serde_json::json!({"choices": []})).is_none());
}
