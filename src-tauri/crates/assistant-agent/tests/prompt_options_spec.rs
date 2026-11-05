use assistant_agent::chat_options::{self, ReasoningOverride};
use assistant_agent::prompt_builder::{self, AgentKind};
use assistant_providers::base;
use assistant_storage::agent_modes::AgentModeRow;
use assistant_storage::conversations::{ConversationDetail, ConversationRow, MessageRow};

fn make_detail(messages: Vec<MessageRow>) -> ConversationDetail {
    ConversationDetail {
        conversation: ConversationRow {
            id: "test-conv".to_string(),
            title: "Test".to_string(),
            title_source: "manual".to_string(),
            provider_id: Some("test-provider".to_string()),
            model_id: "test-model".to_string(),
            system_prompt: Some("You are helpful".to_string()),
            is_pinned: false,
            sort_order: 0,
            generation: 0,
            created_at: 0,
            updated_at: 0,
            is_full: false,
            active_branch_id: None,
        },
        messages,
    }
}

fn make_message_row(role: &str, content: &str) -> MessageRow {
    MessageRow {
        id: format!("msg-{}", role),
        conversation_id: "test-conv".to_string(),
        role: role.to_string(),
        content: content.to_string(),
        tokens: None,
        timestamp: 0,
        parent_id: None,
        is_pinned: false,
        reasoning: None,
        tool_calls_json: None,
        sibling_index: 0,
        provider_id: None,
        model_id: None,
        tool_call_id: None,
        is_error: false,
    }
}

fn make_mode(kind: &str, temperature: Option<f32>, max_tokens: Option<u32>) -> AgentModeRow {
    AgentModeRow {
        id: format!("mode-{}", kind),
        name: kind.to_string(),
        description: String::new(),
        kind: kind.to_string(),
        prompt: String::new(),
        model: None,
        temperature,
        max_tokens,
        reasoning_enabled: None,
        reasoning_effort: None,
        tools_json: "[]".to_string(),
        presentation_rank: 0,
        is_builtin: true,
        created_at: 0,
        updated_at: 0,
    }
}

#[test]
fn prepare_messages_filters_compact_summary() {
    let detail = make_detail(vec![
        make_message_row("user", "hello"),
        make_message_row("compact_summary", "old summary"),
        make_message_row("assistant", "hi there"),
    ]);

    let result = prompt_builder::prepare_messages(&detail, "You are helpful", None);

    assert_eq!(result.len(), 3);
    assert!(matches!(result[0].role, base::MessageRole::System));
    assert!(matches!(result[1].role, base::MessageRole::User));
    assert!(matches!(result[2].role, base::MessageRole::Assistant));
    assert_eq!(result[1].content, "hello");
    assert_eq!(result[2].content, "hi there");
}

#[test]
fn prepare_messages_system_prompt_insertion() {
    let detail = make_detail(vec![
        make_message_row("user", "hello"),
        make_message_row("assistant", "hi"),
    ]);

    let result = prompt_builder::prepare_messages(&detail, "Custom system prompt", None);

    assert_eq!(result.len(), 3);
    assert!(matches!(result[0].role, base::MessageRole::System));
    assert_eq!(result[0].content, "Custom system prompt");
}

#[test]
fn prepare_messages_empty_system_prompt_no_insert() {
    let detail = make_detail(vec![make_message_row("user", "hello")]);

    let result = prompt_builder::prepare_messages(&detail, "", None);

    assert_eq!(result.len(), 1);
    assert!(matches!(result[0].role, base::MessageRole::User));
}

#[test]
fn prepare_messages_append_user_prompt_chat() {
    let detail = make_detail(vec![
        make_message_row("user", "hello"),
        make_message_row("assistant", "hi"),
    ]);

    let result = prompt_builder::prepare_messages(&detail, "System", None);

    assert_eq!(result.len(), 3);
    assert!(matches!(result[2].role, base::MessageRole::Assistant));
}

#[test]
fn prepare_messages_append_user_prompt_compaction() {
    let detail = make_detail(vec![
        make_message_row("user", "hello"),
        make_message_row("assistant", "hi"),
    ]);

    let result = prompt_builder::prepare_messages(
        &detail,
        "System",
        Some("Please summarize the conversation"),
    );

    assert_eq!(result.len(), 4);
    assert!(matches!(result[3].role, base::MessageRole::User));
    assert_eq!(result[3].content, "Please summarize the conversation");
}

#[test]
fn build_chat_options_default_no_temperature() {
    let kind = AgentKind::Default;
    let mode = make_mode("default", None, None);

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride::default(),
        None,
    );

    assert_eq!(opts.temperature, 0.7);
    assert_eq!(opts.max_tokens, 4096);
    assert!(opts.stream);
    assert_eq!(opts.context_window_tokens, Some(128000));
}

#[test]
fn build_chat_options_default_with_temperature() {
    let kind = AgentKind::Default;
    let mode = make_mode("default", Some(0.5), Some(2048));

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride::default(),
        None,
    );

    assert_eq!(opts.temperature, 0.5);
    assert_eq!(opts.max_tokens, 2048);
}

#[test]
fn build_chat_options_quick() {
    let kind = AgentKind::Quick;
    let mode = make_mode("quick", Some(0.3), Some(1024));

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride::default(),
        None,
    );

    assert_eq!(opts.temperature, 0.3);
    assert_eq!(opts.max_tokens, 1024);
}

#[test]
fn build_chat_options_unknown_window_stays_none() {
    let kind = AgentKind::Default;
    let mode = make_mode("default", None, None);

    let opts =
        chat_options::build_chat_options(&kind, &mode, None, &ReasoningOverride::default(), None);

    assert_eq!(opts.context_window_tokens, None);
}

#[test]
fn build_chat_options_deep_with_temperature() {
    let kind = AgentKind::Deep;
    let mode = make_mode("deep", Some(0.7), Some(8192));

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride::default(),
        None,
    );

    assert_eq!(opts.temperature, 0.7);
    assert_eq!(opts.max_tokens, 8192);
}

#[test]
fn build_chat_options_deep_no_temperature_fallback() {
    let kind = AgentKind::Deep;
    let mode = make_mode("deep", None, None);

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride::default(),
        None,
    );

    assert_eq!(opts.temperature, 0.3);
    assert_eq!(opts.max_tokens, 4096);
}

#[test]
fn build_chat_options_custom() {
    let kind = AgentKind::Custom;
    let mode = make_mode("custom", Some(0.8), Some(6000));

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride::default(),
        None,
    );

    assert_eq!(opts.temperature, 0.8);
    assert_eq!(opts.max_tokens, 6000);
}

#[test]
fn build_chat_options_reasoning() {
    let kind = AgentKind::Deep;
    let mut mode = make_mode("deep", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride {
            level: Some("high".to_string()),
        },
        None,
    );

    assert_eq!(opts.reasoning_enabled, Some(true));
    assert_eq!(opts.reasoning_effort.as_deref(), Some("high"));
}

#[test]
fn build_chat_options_reasoning_default_level_ignores_mode() {
    let kind = AgentKind::Deep;
    let mut mode = make_mode("deep", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride {
            level: Some("default".to_string()),
        },
        None,
    );

    assert_eq!(opts.reasoning_enabled, None);
    assert_eq!(opts.reasoning_effort, None);
}

#[test]
fn build_chat_options_reasoning_absent_level_equals_default() {
    let kind = AgentKind::Deep;
    let mut mode = make_mode("deep", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride::default(),
        None,
    );

    assert_eq!(opts.reasoning_enabled, None);
    assert_eq!(opts.reasoning_effort, None);
}

#[test]
fn build_chat_options_reasoning_override_off_wins() {
    let kind = AgentKind::Deep;
    let mut mode = make_mode("deep", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride {
            level: Some("off".to_string()),
        },
        None,
    );

    assert_eq!(opts.reasoning_enabled, Some(false));
    assert_eq!(opts.reasoning_effort, None);
}

#[test]
fn build_chat_options_reasoning_override_effort_wins() {
    let kind = AgentKind::Deep;
    let mut mode = make_mode("deep", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride {
            level: Some("low".to_string()),
        },
        None,
    );

    assert_eq!(opts.reasoning_enabled, Some(true));
    assert_eq!(opts.reasoning_effort.as_deref(), Some("low"));
}

#[test]
fn build_chat_options_reasoning_override_enables_without_effort() {
    let kind = AgentKind::Deep;
    let mut mode = make_mode("deep", Some(0.7), Some(8192));
    mode.reasoning_enabled = None;
    mode.reasoning_effort = None;

    let opts = chat_options::build_chat_options(
        &kind,
        &mode,
        Some(128000),
        &ReasoningOverride {
            level: Some("on".to_string()),
        },
        None,
    );

    assert_eq!(opts.reasoning_enabled, Some(true));
    assert_eq!(opts.reasoning_effort, None);
}

#[test]
fn agent_kind_from_mode_kind() {
    assert_eq!(AgentKind::from_mode_kind("default"), AgentKind::Default);
    assert_eq!(AgentKind::from_mode_kind("quick"), AgentKind::Quick);
    assert_eq!(AgentKind::from_mode_kind("deep"), AgentKind::Deep);
    assert_eq!(AgentKind::from_mode_kind("custom"), AgentKind::Custom);
    assert_eq!(AgentKind::from_mode_kind("unknown"), AgentKind::Default);
    assert_eq!(AgentKind::from_mode_kind(""), AgentKind::Default);
}

#[test]
fn agent_kind_supports_tool_call() {
    assert!(!AgentKind::Default.supports_tool_call());
    assert!(!AgentKind::Quick.supports_tool_call());
    assert!(!AgentKind::Deep.supports_tool_call());
    assert!(AgentKind::Custom.supports_tool_call());
}
