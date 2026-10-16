use assistant_providers::base::{ProviderErrorCode, ProviderErrorPayload};
use assistant_providers::types::chat_delta::{RetryNotification, TokenUsage};
use assistant_storage::{
    agent_modes::{AgentModeRow, TokenUsageSummary},
    conversations::{ConversationRow, MessageRow},
};
use serde::{Deserialize, Serialize};

// ── Chat DTOs ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationDto {
    pub id: String,
    pub title: String,
    pub title_source: String,
    pub provider_id: Option<String>,
    pub model_id: String,
    pub system_prompt: Option<String>,
    pub is_pinned: bool,
    pub sort_order: i64,
    pub generation: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_full: bool,
}

impl From<ConversationRow> for ConversationDto {
    fn from(r: ConversationRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            title_source: r.title_source,
            provider_id: r.provider_id,
            model_id: r.model_id,
            system_prompt: r.system_prompt,
            is_pinned: r.is_pinned,
            sort_order: r.sort_order,
            generation: r.generation,
            created_at: r.created_at,
            updated_at: r.updated_at,
            is_full: r.is_full,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageDto {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<i64>,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub is_pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls_json: Option<String>,
}

impl From<MessageRow> for MessageDto {
    fn from(r: MessageRow) -> Self {
        Self {
            id: r.id,
            conversation_id: r.conversation_id,
            role: r.role,
            content: r.content,
            tokens: r.tokens,
            timestamp: r.timestamp,
            parent_id: r.parent_id,
            is_pinned: r.is_pinned,
            reasoning: r.reasoning,
            tool_calls_json: r.tool_calls_json,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationDetailDto {
    pub conversation: ConversationDto,
    pub messages: Vec<MessageDto>,
    pub total_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageDto {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub cache_read_tokens: u32,
    #[serde(default)]
    pub cache_write_tokens: u32,
    #[serde(default)]
    pub context_window_tokens: u32,
}

impl From<TokenUsage> for TokenUsageDto {
    fn from(u: TokenUsage) -> Self {
        Self {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            cache_read_tokens: u.cache_read_tokens,
            cache_write_tokens: u.cache_write_tokens,
            context_window_tokens: u.context_window_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenUsageSummaryDto {
    pub total_tokens: i64,
    pub context_window_tokens: u32,
    pub percentage: f32,
}

impl From<TokenUsageSummary> for TokenUsageSummaryDto {
    fn from(s: TokenUsageSummary) -> Self {
        Self {
            total_tokens: s.total_tokens,
            context_window_tokens: s.context_window_tokens,
            percentage: s.percentage,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryNotificationDto {
    pub attempt: u32,
    pub max_attempts: u32,
    pub delay_ms: u64,
    pub message: String,
}

impl From<RetryNotification> for RetryNotificationDto {
    fn from(r: RetryNotification) -> Self {
        Self {
            attempt: r.attempt,
            max_attempts: r.max_attempts,
            delay_ms: r.delay_ms,
            message: r.message,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderErrorPayloadDto {
    pub code: ProviderErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    pub retryable: bool,
}

impl From<ProviderErrorPayload> for ProviderErrorPayloadDto {
    fn from(p: ProviderErrorPayload) -> Self {
        Self {
            code: p.code,
            message: p.message,
            retry_after_ms: p.retry_after_ms,
            retryable: p.retryable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatDeltaPayloadDto {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
    pub seq: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_tool_call_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_tool_call_arguments: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsageDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_retry: Option<RetryNotificationDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProviderErrorPayloadDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatStreamStartPayloadDto {
    pub conversation_id: String,
    pub attempt_id: String,
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationTerminatedPayloadDto {
    pub conversation_id: String,
    pub summary: String,
    pub is_full: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationCompactFailedPayloadDto {
    pub conversation_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationUnlockedPayloadDto {
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatRegeneratePayloadDto {
    pub conversation_id: String,
    pub deleted_message_id: String,
}

// ── Settings DTOs ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentModeDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    pub tools: Vec<String>,
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl AgentModeDto {
    pub fn from_row(row: &AgentModeRow) -> Self {
        let tools: Vec<String> = serde_json::from_str(&row.tools_json).unwrap_or_default();
        Self {
            id: row.id.clone(),
            name: row.name.clone(),
            description: row.description.clone(),
            kind: row.kind.clone(),
            prompt: row.prompt.clone(),
            model: row.model.clone(),
            temperature: row.temperature,
            max_tokens: row.max_tokens,
            reasoning_enabled: row.reasoning_enabled,
            reasoning_effort: row.reasoning_effort.clone(),
            tools,
            presentation_rank: row.presentation_rank,
            is_builtin: row.is_builtin,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentModeInputDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    pub tools: Vec<String>,
    pub presentation_rank: u32,
    pub is_builtin: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelConfigDto {
    pub id: String,
    pub name: String,
    pub owned_by: String,
    #[serde(default)]
    pub context_window: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderInputDto {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub api_key_changed: bool,
    pub models: String,
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderListDto {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub base_url: String,
    pub api_key_masked: String,
    pub models: Vec<ModelConfigDto>,
    pub default_model: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingProviderChangedPayloadDto {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutWarningPayloadDto {
    pub id: String,
    pub binding: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeChangedPayloadDto {
    pub resolved: String,
}

// ── Config DTOs ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppConfDto {
    pub theme: String,
    pub language: String,
    pub hotkey: String,
    pub stay_on_top: bool,
    pub launch_at_login: bool,
    pub close_to_tray: bool,
    pub keybindings: std::collections::HashMap<String, String>,
    pub auto_compact: bool,
    pub compact_threshold_percent: u8,
    pub compact_summary_prompt: Option<String>,
    pub compact_summary_max_tokens: u16,
}

impl From<assistant_config::app_conf::AppConf> for AppConfDto {
    fn from(c: assistant_config::app_conf::AppConf) -> Self {
        Self {
            theme: c.theme,
            language: c.language,
            hotkey: c.hotkey,
            stay_on_top: c.stay_on_top,
            launch_at_login: c.launch_at_login,
            close_to_tray: c.close_to_tray,
            keybindings: c.keybindings,
            auto_compact: c.auto_compact,
            compact_threshold_percent: c.compact_threshold_percent,
            compact_summary_prompt: c.compact_summary_prompt,
            compact_summary_max_tokens: c.compact_summary_max_tokens,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OrbConfDto {
    pub click_through: bool,
    pub always_on_top: bool,
    pub scale_percent: u16,
    pub opacity_percent: u8,
    pub hide_on_pointer_hover: bool,
    pub hover_delay_ms: u64,
    pub idle_opacity_percent: u8,
    pub position_x: Option<i32>,
    pub position_y: Option<i32>,
}

impl From<assistant_config::orb_conf::OrbConf> for OrbConfDto {
    fn from(c: assistant_config::orb_conf::OrbConf) -> Self {
        Self {
            click_through: c.click_through,
            always_on_top: c.always_on_top,
            scale_percent: c.scale_percent,
            opacity_percent: c.opacity_percent,
            hide_on_pointer_hover: c.hide_on_pointer_hover,
            hover_delay_ms: c.hover_delay_ms,
            idle_opacity_percent: c.idle_opacity_percent,
            position_x: c.position_x,
            position_y: c.position_y,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MascotConfDto {
    pub renderer: String,
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub rive_idle_pose_rotation: bool,
    pub rive_src: Option<String>,
    pub svg_animated: bool,
    pub gif_src: Option<String>,
    pub gif_border_radius: u32,
}

impl From<assistant_config::mascot_conf::MascotConf> for MascotConfDto {
    fn from(c: assistant_config::mascot_conf::MascotConf) -> Self {
        Self {
            renderer: c.renderer,
            primary_color: c.primary_color,
            secondary_color: c.secondary_color,
            rive_idle_pose_rotation: c.rive_idle_pose_rotation,
            rive_src: c.rive_src,
            svg_animated: c.svg_animated,
            gif_src: c.gif_src,
            gif_border_radius: c.gif_border_radius,
        }
    }
}

// ── Orb DTOs ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MousePositionDto {
    pub client_x: i32,
    pub client_y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenSizeDto {
    pub screen: ScreenRectDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenRectDto {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

// ── Round-trip tests ────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
            cache_read_tokens: 0,
            cache_write_tokens: 0,
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
        };
        roundtrip(&dto);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("maxAttempts"), "camelCase: {}", json);
        assert!(json.contains("delayMs"), "camelCase: {}", json);
    }

    #[test]
    fn test_provider_error_payload_dto_roundtrip() {
        let dto = ProviderErrorPayloadDto {
            code: ProviderErrorCode::RateLimited,
            message: "rate limited".into(),
            retry_after_ms: Some(5000),
            retryable: true,
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
}