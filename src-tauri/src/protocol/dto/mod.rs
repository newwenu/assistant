pub mod branch;
pub mod chat;
pub mod config;
pub mod orb;
pub mod settings;
pub mod storage;

pub use branch::*;
pub use chat::*;
pub use config::*;
pub use orb::*;
pub use settings::*;
pub use storage::*;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(
        value: &T,
    ) {
        let json = serde_json::to_string(value).unwrap();
        let back: T = serde_json::from_str(&json).unwrap();
        assert_eq!(*value, back, "round-trip failed for {}", json);
    }

    #[test]
    fn test_conversation_dto_roundtrip() {
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
        };
        roundtrip(&dto);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("titleSource"), "camelCase rename: {}", json);
        assert!(!json.contains("title_source"), "no snake_case: {}", json);
    }

    #[test]
    fn test_message_dto_roundtrip() {
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
            tool_calls_json: None,
            sibling_index: 0,
            sibling_count: None,
            provider_id: None,
            model_id: None,
        };
        roundtrip(&dto);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("conversationId"), "camelCase: {}", json);
    }

    #[test]
    fn test_chat_delta_payload_dto_roundtrip() {
        let dto = ChatDeltaPayloadDto {
            conversation_id: "c1".into(),
            attempt_id: "a1".into(),
            message_id: "m1".into(),
            seq: 1,
            delta_text: Some("hi".into()),
            delta_reasoning: None,
            delta_tool_call_id: None,
            delta_tool_call_name: None,
            delta_tool_call_arguments: None,
            finish_reason: None,
            usage: None,
            delta_retry: None,
            error: None,
        };
        roundtrip(&dto);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("conversationId"), "camelCase: {}", json);
        assert!(json.contains("deltaText"), "camelCase: {}", json);
        assert!(!json.contains("delta_text"), "no snake_case: {}", json);
    }

    #[test]
    fn test_token_usage_dto_roundtrip() {
        let dto = TokenUsageDto {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            context_window_tokens: 128000,
        };
        roundtrip(&dto);
    }

    #[test]
    fn test_retry_notification_dto_roundtrip() {
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
    fn test_provider_error_payload_dto_roundtrip() {
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
    fn test_agent_mode_dto_roundtrip() {
        let dto = AgentModeDto {
            id: "mode1".into(),
            name: "Quick".into(),
            description: "quick mode".into(),
            kind: "quick".into(),
            prompt: "prompt".into(),
            model: None,
            temperature: None,
            max_tokens: None,
            reasoning_enabled: None,
            reasoning_effort: None,
            tools: vec![],
            presentation_rank: 0,
            is_builtin: true,
            created_at: 1000,
            updated_at: 1000,
        };
        roundtrip(&dto);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("presentationRank"), "camelCase: {}", json);
        assert!(json.contains("isBuiltin"), "camelCase: {}", json);
    }

    #[test]
    fn test_provider_list_dto_roundtrip() {
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
                context_window: 128000,
                support_vision: true,
            }],
            default_model: "gpt-4".into(),
            updated_at: 1000,
        };
        roundtrip(&dto);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("baseUrl"), "camelCase: {}", json);
        assert!(json.contains("apiKeyMasked"), "camelCase: {}", json);
        assert!(json.contains("defaultModel"), "camelCase: {}", json);
    }

    #[test]
    fn test_conversation_detail_dto_roundtrip() {
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
            },
            messages: vec![],
            total_count: 0,
        };
        roundtrip(&dto);
    }

    #[test]
    fn test_event_payload_dtos_roundtrip() {
        let start = ChatStreamStartPayloadDto {
            conversation_id: "c1".into(),
            attempt_id: "a1".into(),
            message_id: "m1".into(),
        };
        roundtrip(&start);

        let terminated = ConversationTerminatedPayloadDto {
            conversation_id: "c1".into(),
            summary: "summary".into(),
            is_full: true,
        };
        roundtrip(&terminated);

        let compact_failed = ConversationCompactFailedPayloadDto {
            conversation_id: "c1".into(),
            error: "err".into(),
        };
        roundtrip(&compact_failed);

        let unlocked = ConversationUnlockedPayloadDto {
            conversation_id: "c1".into(),
        };
        roundtrip(&unlocked);

        let regenerate = ChatRegeneratePayloadDto {
            conversation_id: "c1".into(),
            deleted_message_id: "m1".into(),
        };
        roundtrip(&regenerate);
    }

    #[test]
    fn test_setting_dtos_roundtrip() {
        let provider_changed = SettingProviderChangedPayloadDto {
            action: "saved".into(),
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
    fn test_orb_dtos_roundtrip() {
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
    fn test_skip_serializing_if_none() {
        let dto = ChatDeltaPayloadDto {
            conversation_id: "c1".into(),
            attempt_id: "a1".into(),
            message_id: "m1".into(),
            seq: 1,
            delta_text: None,
            delta_reasoning: None,
            delta_tool_call_id: None,
            delta_tool_call_name: None,
            delta_tool_call_arguments: None,
            finish_reason: None,
            usage: None,
            delta_retry: None,
            error: None,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(!json.contains("deltaText"), "None fields omitted: {}", json);
        assert!(!json.contains("usage"), "None fields omitted: {}", json);
    }

    #[test]
    fn test_conversation_patch_dto_roundtrip() {
        let dto = ConversationPatchDto {
            title: Some("new title".into()),
            title_source: Some("user".into()),
            model_id: None,
            provider_id: Some("p1".into()),
        };
        roundtrip(&dto);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("titleSource"), "camelCase: {}", json);
        assert!(json.contains("providerId"), "camelCase: {}", json);
        assert!(!json.contains("modelId"), "None omitted: {}", json);
    }

    #[test]
    fn test_conversation_query_opts_dto_roundtrip() {
        let dto = ConversationQueryOptsDto {
            mode: "paginated".into(),
            limit: 50,
            before_timestamp: Some(1000),
        };
        roundtrip(&dto);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("beforeTimestamp"), "camelCase: {}", json);
    }
}
