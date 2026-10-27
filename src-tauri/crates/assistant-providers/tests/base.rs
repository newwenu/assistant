use assistant_providers::base::{
    get_context_window_tokens, message_role_from_str, provider_kind_from_str, MessageRole,
    ProviderError, ProviderKind, RetryConfig,
};

#[test]
fn context_window_known_models() {
    assert_eq!(get_context_window_tokens("gpt-4"), 8_192);
    assert_eq!(get_context_window_tokens("gpt-4-turbo"), 128_000);
    assert_eq!(get_context_window_tokens("gpt-4o"), 128_000);
    assert_eq!(get_context_window_tokens("claude-3.5-sonnet"), 200_000);
}

#[test]
fn context_window_unknown_returns_default() {
    assert_eq!(get_context_window_tokens(""), 128_000);
    assert_eq!(get_context_window_tokens("unknown"), 128_000);
    assert_eq!(
        get_context_window_tokens("GPT-4O"),
        get_context_window_tokens("gpt-4o")
    );
}

#[test]
fn provider_kind_fallback() {
    let kind = provider_kind_from_str("Ollama", "http://localhost:11434".to_string());
    assert!(matches!(kind, ProviderKind::Custom { .. }));
    let kind2 = provider_kind_from_str("", "http://x".to_string());
    assert!(matches!(kind2, ProviderKind::Custom { .. }));
}

#[test]
fn message_role_unknown_defaults_user() {
    assert_eq!(message_role_from_str("tool"), MessageRole::Tool);
    assert_eq!(message_role_from_str(""), MessageRole::User);
}

#[test]
fn error_retryable_classification() {
    let retryable: Vec<ProviderError> = vec![
        ProviderError::RateLimited {
            retry_after_ms: None,
        },
        ProviderError::NetworkError("conn reset".into()),
        ProviderError::Timeout("30s".into()),
        ProviderError::ServerError("500".into()),
        ProviderError::StreamError("eof".into()),
    ];
    for e in &retryable {
        assert!(e.is_retryable(), "{:?} should be retryable", e);
    }
    let non_retryable: Vec<ProviderError> = vec![
        ProviderError::ApiKeyMissing("test".into()),
        ProviderError::AuthFailed("401".into()),
        ProviderError::ModelNotFound("gpt-5".into()),
        ProviderError::ContextOverflow("too long".into()),
        ProviderError::RequestRejected("policy".into()),
        ProviderError::ParseError("bad json".into()),
        ProviderError::Cancelled,
    ];
    for e in &non_retryable {
        assert!(!e.is_retryable(), "{:?} should NOT be retryable", e);
    }
}

#[test]
fn retry_config_behavior() {
    let cfg = RetryConfig::default();
    let net_err = ProviderError::NetworkError("timeout".to_string());
    let rate_err = ProviderError::RateLimited {
        retry_after_ms: None,
    };

    let d0 = cfg.delay_ms(0, &net_err);
    let d1 = cfg.delay_ms(1, &net_err);
    let d2 = cfg.delay_ms(2, &net_err);
    assert!(d1 > d0, "exponential backoff: {} > {}", d1, d0);
    assert!(d2 > d1, "exponential backoff: {} > {}", d2, d1);
    assert!(d2 <= cfg.max_delay_ms, "capped at max_delay_ms");

    let d_net = cfg.delay_ms(0, &net_err);
    let d_rate = cfg.delay_ms(0, &rate_err);
    assert!(
        d_rate > d_net,
        "rate limit base larger: {} > {}",
        d_rate,
        d_net
    );

    let hint_err = ProviderError::RateLimited {
        retry_after_ms: Some(10_000),
    };
    let delay = cfg.delay_ms(0, &hint_err);
    assert!(
        delay >= 10_000,
        "retry_after_ms honored: {} >= 10000",
        delay
    );
    assert!(
        delay <= cfg.rate_limit_max_ms,
        "capped at rate_limit_max_ms"
    );

    let huge_err = ProviderError::RateLimited {
        retry_after_ms: Some(999_999_999),
    };
    assert!(
        cfg.delay_ms(0, &huge_err) <= cfg.rate_limit_max_ms,
        "extreme hint capped"
    );
}

#[test]
fn provider_kind_use_native_api() {
    assert!(!ProviderKind::OpenAI.use_native_api());
    assert!(ProviderKind::Anthropic.use_native_api());
    assert!(ProviderKind::Gemini.use_native_api());
    assert!(!ProviderKind::Custom {
        base_url: String::new()
    }
    .use_native_api());
}