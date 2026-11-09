use assistant_agent::chat_options::{self, ReasoningOverride};
use assistant_agent::prompt_builder::{self, AgentKind, PromptFacts};
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
            mode_id: None,
            tool_config_json: None,
            workspace_root: String::new(),
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
        status: "completed".to_string(),
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
        presentation_rank: 0,
        is_builtin: true,
        created_at: 0,
        updated_at: 0,
        tool_config_json: None,
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
fn compose_system_prompt_appends_fact_after_prompt() {
    let facts = PromptFacts {
        working_directory: Some("G:/ws".to_string()),
    };

    let composed = prompt_builder::compose_system_prompt("You are helpful", &facts);

    assert_eq!(composed, "You are helpful\n\nWorking directory: G:/ws");
}

#[test]
fn compose_system_prompt_fact_only_when_prompt_empty() {
    let facts = PromptFacts {
        working_directory: Some("G:/ws".to_string()),
    };

    let composed = prompt_builder::compose_system_prompt("", &facts);

    assert_eq!(composed, "Working directory: G:/ws");
}

#[test]
fn compose_system_prompt_unchanged_without_facts() {
    let composed =
        prompt_builder::compose_system_prompt("You are helpful", &PromptFacts::default());
    assert_eq!(composed, "You are helpful");

    let composed = prompt_builder::compose_system_prompt(
        "You are helpful",
        &PromptFacts {
            working_directory: Some(String::new()),
        },
    );
    assert_eq!(composed, "You are helpful");

    assert!(prompt_builder::compose_system_prompt("", &prompt_builder::NO_FACTS).is_empty());
}

#[test]
fn build_chat_options_no_temperature_fallback() {
    let mode = make_mode("chat", None, None);

    let opts =
        chat_options::build_chat_options(&mode, Some(128000), &ReasoningOverride::default(), None);

    assert_eq!(opts.temperature, 0.7);
    assert_eq!(opts.max_tokens, 4096);
    assert!(opts.stream);
    assert_eq!(opts.context_window_tokens, Some(128000));
}

fn make_tool_row(id: &str, call_id: &str, content: &str, is_error: bool) -> MessageRow {
    let mut row = make_message_row("tool", content);
    row.id = id.to_string();
    row.tool_call_id = Some(call_id.to_string());
    row.is_error = is_error;
    row
}

fn make_assistant_with_calls(id: &str, calls: &[(&str, &str, &str)]) -> MessageRow {
    let json: Vec<serde_json::Value> = calls
        .iter()
        .map(|(call_id, name, arguments)| {
            serde_json::json!({ "id": call_id, "name": name, "arguments": arguments })
        })
        .collect();
    let mut row = make_message_row("assistant", "");
    row.id = id.to_string();
    row.tool_calls_json = Some(serde_json::Value::Array(json).to_string());
    row
}

#[test]
fn prepare_messages_rebuilds_tool_history() {
    let detail = make_detail(vec![
        make_message_row("user", "read a.txt"),
        make_assistant_with_calls(
            "a1",
            &[("c1", "read", "{\"path\":\"a.txt\"}"), ("c2", "list", "{}")],
        ),
        make_tool_row("t1", "c1", "file body", false),
        make_tool_row("t2", "c2", "entries", true),
    ]);

    let wire = prompt_builder::prepare_messages(&detail, "S", None);

    assert_eq!(wire.len(), 5);
    let calls = wire[2]
        .tool_calls
        .as_ref()
        .expect("assistant 应带 tool_calls");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].id, "c1");
    assert_eq!(calls[0].name, "read");
    assert_eq!(calls[1].id, "c2");
    assert_eq!(wire[3].tool_call_id.as_deref(), Some("c1"));
    assert_eq!(wire[3].name.as_deref(), Some("read"));
    assert!(!wire[3].is_error);
    assert_eq!(wire[3].content, "file body");
    assert_eq!(wire[4].tool_call_id.as_deref(), Some("c2"));
    assert_eq!(wire[4].name.as_deref(), Some("list"));
    assert!(wire[4].is_error);
}

#[test]
fn prepare_messages_synthesizes_missing_tool_result() {
    let detail = make_detail(vec![
        make_message_row("user", "go"),
        make_assistant_with_calls("a1", &[("c1", "read", "{}"), ("c2", "list", "{}")]),
        make_tool_row("t1", "c1", "ok", false),
    ]);
    let rows_before = detail.messages.len();

    let wire = prompt_builder::prepare_messages(&detail, "", None);

    assert_eq!(detail.messages.len(), rows_before, "投影修复不得写库");
    assert_eq!(wire.len(), 4);
    assert_eq!(wire[3].role, base::MessageRole::Tool);
    assert_eq!(wire[3].tool_call_id.as_deref(), Some("c2"));
    assert_eq!(wire[3].name.as_deref(), Some("list"));
    assert_eq!(wire[3].content, prompt_builder::INTERRUPTED_TOOL_RESULT);
    assert!(wire[3].is_error);
}

#[test]
fn prepare_messages_drops_orphan_tool_result() {
    let detail = make_detail(vec![
        make_message_row("user", "go"),
        make_tool_row("t1", "ghost", "orphan", false),
        make_message_row("assistant", "done"),
    ]);

    let wire = prompt_builder::prepare_messages(&detail, "", None);

    assert_eq!(wire.len(), 2);
    assert!(matches!(wire[0].role, base::MessageRole::User));
    assert!(matches!(wire[1].role, base::MessageRole::Assistant));
}

#[test]
fn prepare_messages_pairing_repair_is_idempotent() {
    let detail = make_detail(vec![
        make_message_row("user", "go"),
        make_assistant_with_calls("a1", &[("c1", "read", "{}"), ("c2", "list", "{}")]),
        make_tool_row("t1", "c1", "ok", false),
        make_tool_row("t2", "ghost", "orphan", false),
    ]);

    let first = prompt_builder::prepare_messages(&detail, "S", None);
    let second = prompt_builder::prepare_messages(&detail, "S", None);

    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
}

#[test]
fn prepare_messages_replaces_empty_tool_body() {
    let detail = make_detail(vec![
        make_message_row("user", "go"),
        make_assistant_with_calls("a1", &[("c1", "read", "{}")]),
        make_tool_row("t1", "c1", "", true),
    ]);

    let wire = prompt_builder::prepare_messages(&detail, "", None);

    assert_eq!(wire[2].content, prompt_builder::EMPTY_TOOL_RESULT);
    assert!(wire[2].is_error);
}

#[test]
fn tool_history_roundtrip_rebuilds_wire_shape() {
    let dir = tempfile::tempdir().unwrap();
    let conn = assistant_storage::db::init_db(&dir.path().join("wire.db")).unwrap();
    let conv = ConversationRow {
        id: "wire-conv".to_string(),
        title: "Wire".to_string(),
        title_source: "manual".to_string(),
        provider_id: None,
        model_id: "test-model".to_string(),
        system_prompt: None,
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: 0,
        updated_at: 0,
        is_full: false,
        active_branch_id: None,
        mode_id: None,
        tool_config_json: None,
        workspace_root: String::new(),
    };
    assistant_storage::conversations::insert_conversation(&conn, &conv).unwrap();

    let row = |id: &str, role: &str, content: &str, ts: i64, parent: Option<&str>| MessageRow {
        id: id.to_string(),
        conversation_id: "wire-conv".to_string(),
        role: role.to_string(),
        content: content.to_string(),
        tokens: None,
        timestamp: ts,
        parent_id: parent.map(str::to_string),
        is_pinned: false,
        reasoning: None,
        tool_calls_json: None,
        sibling_index: 0,
        provider_id: None,
        model_id: None,
        tool_call_id: None,
        is_error: false,
        status: "completed".to_string(),
    };

    assistant_storage::conversations::insert_message(
        &conn,
        &row("u1", "user", "read a.txt", 100, None),
    )
    .unwrap();
    assistant_storage::conversations::insert_message(
        &conn,
        &row("a1", "assistant", "", 200, Some("u1")),
    )
    .unwrap();
    assistant_storage::conversations::update_message_tool_calls(
        &conn,
        "a1",
        r#"[{"id":"c1","name":"read","arguments":"{\"path\":\"a.txt\"}"},{"id":"c2","name":"list","arguments":"{}"}]"#,
    )
    .unwrap();

    let mut t1 = row("t1", "tool", "file body", 300, Some("a1"));
    t1.tool_call_id = Some("c1".to_string());
    let mut t2 = row("t2", "tool", "entries", 400, Some("a1"));
    t2.tool_call_id = Some("c2".to_string());
    t2.is_error = true;
    assistant_storage::conversations::insert_message(&conn, &t1).unwrap();
    assistant_storage::conversations::insert_message(&conn, &t2).unwrap();

    let detail = assistant_storage::conversations::get_conversation_detail(&conn, "wire-conv")
        .unwrap()
        .expect("conversation detail");

    let wire = prompt_builder::prepare_messages(&detail, "", None);

    assert_eq!(wire.len(), 4, "落库行链重建为 4 条 wire 消息");
    assert!(matches!(wire[0].role, base::MessageRole::User));
    assert!(matches!(wire[1].role, base::MessageRole::Assistant));
    let calls = wire[1]
        .tool_calls
        .as_ref()
        .expect("assistant 应带 tool_calls");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].id, "c1");
    assert_eq!(calls[1].id, "c2");
    assert_eq!(wire[2].tool_call_id.as_deref(), Some("c1"));
    assert_eq!(wire[2].name.as_deref(), Some("read"));
    assert_eq!(wire[2].content, "file body");
    assert!(!wire[2].is_error);
    assert_eq!(wire[3].tool_call_id.as_deref(), Some("c2"));
    assert_eq!(wire[3].name.as_deref(), Some("list"));
    assert_eq!(wire[3].content, "entries");
    assert!(wire[3].is_error);
}

#[test]
fn build_chat_options_with_temperature() {
    let mode = make_mode("chat", Some(0.5), Some(2048));

    let opts =
        chat_options::build_chat_options(&mode, Some(128000), &ReasoningOverride::default(), None);

    assert_eq!(opts.temperature, 0.5);
    assert_eq!(opts.max_tokens, 2048);
}

#[test]
fn build_chat_options_unknown_window_stays_none() {
    let mode = make_mode("chat", None, None);

    let opts = chat_options::build_chat_options(&mode, None, &ReasoningOverride::default(), None);

    assert_eq!(opts.context_window_tokens, None);
}

#[test]
fn build_chat_options_agent_mode_uses_mode_temperature() {
    let mode = make_mode("agent", Some(0.8), Some(6000));

    let opts =
        chat_options::build_chat_options(&mode, Some(128000), &ReasoningOverride::default(), None);

    assert_eq!(opts.temperature, 0.8);
    assert_eq!(opts.max_tokens, 6000);
}

#[test]
fn build_chat_options_legacy_deep_kind_no_longer_overrides_temperature() {
    let mode = make_mode("deep", None, None);

    let opts =
        chat_options::build_chat_options(&mode, Some(128000), &ReasoningOverride::default(), None);

    assert_eq!(opts.temperature, 0.7);
    assert_eq!(opts.max_tokens, 4096);
}

#[test]
fn build_chat_options_reasoning() {
    let mut mode = make_mode("agent", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = chat_options::build_chat_options(
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
    let mut mode = make_mode("agent", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = chat_options::build_chat_options(
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
    let mut mode = make_mode("agent", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts =
        chat_options::build_chat_options(&mode, Some(128000), &ReasoningOverride::default(), None);

    assert_eq!(opts.reasoning_enabled, None);
    assert_eq!(opts.reasoning_effort, None);
}

#[test]
fn build_chat_options_reasoning_override_off_wins() {
    let mut mode = make_mode("agent", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = chat_options::build_chat_options(
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
    let mut mode = make_mode("agent", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = chat_options::build_chat_options(
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
    let mut mode = make_mode("agent", Some(0.7), Some(8192));
    mode.reasoning_enabled = None;
    mode.reasoning_effort = None;

    let opts = chat_options::build_chat_options(
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
    assert_eq!(AgentKind::from_mode_kind("agent"), AgentKind::Agent);
    assert_eq!(AgentKind::from_mode_kind("chat"), AgentKind::Chat);
    assert_eq!(AgentKind::from_mode_kind("default"), AgentKind::Chat);
    assert_eq!(AgentKind::from_mode_kind("quick"), AgentKind::Chat);
    assert_eq!(AgentKind::from_mode_kind("deep"), AgentKind::Chat);
    assert_eq!(AgentKind::from_mode_kind("custom"), AgentKind::Chat);
    assert_eq!(AgentKind::from_mode_kind("unknown"), AgentKind::Chat);
    assert_eq!(AgentKind::from_mode_kind(""), AgentKind::Chat);
}

#[test]
fn agent_kind_supports_tool_call() {
    assert!(!AgentKind::Chat.supports_tool_call());
    assert!(AgentKind::Agent.supports_tool_call());
}
