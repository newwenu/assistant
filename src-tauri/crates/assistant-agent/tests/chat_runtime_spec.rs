use assistant_agent::chat_runtime::{
    self, ApplyDeltaOutcome, StreamAccumulator, StreamError, ToolCallAccumulator,
};
use assistant_agent::prompt_builder::{self, AgentKind};
use assistant_providers::base::{self, ChatDelta, ProviderError, TokenUsage};
use assistant_storage::agent_modes::AgentModeRow;
use assistant_storage::conversations::{ConversationDetail, ConversationRow, MessageRow};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

fn text_delta(text: &str) -> ChatDelta {
    ChatDelta {
        delta_text: Some(text.to_string()),
        ..Default::default()
    }
}

fn reasoning_delta(text: &str) -> ChatDelta {
    ChatDelta {
        delta_reasoning: Some(text.to_string()),
        ..Default::default()
    }
}

fn usage_delta(prompt: u32, completion: u32) -> ChatDelta {
    ChatDelta {
        usage: Some(TokenUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            context_window_tokens: 0,
        }),
        ..Default::default()
    }
}

fn finish_delta(reason: &str) -> ChatDelta {
    ChatDelta {
        finish_reason: Some(reason.to_string()),
        ..Default::default()
    }
}

fn tool_call_id_delta(id: &str) -> ChatDelta {
    ChatDelta {
        delta_tool_call_id: Some(id.to_string()),
        ..Default::default()
    }
}

fn tool_call_name_delta(name: &str) -> ChatDelta {
    ChatDelta {
        delta_tool_call_name: Some(name.to_string()),
        ..Default::default()
    }
}

fn tool_call_args_delta(args: &str) -> ChatDelta {
    ChatDelta {
        delta_tool_call_arguments: Some(args.to_string()),
        ..Default::default()
    }
}

#[test]
fn b7_stream_accumulator_text_only() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&text_delta("Hello"));
    accum.apply_delta(&text_delta(" world"));
    accum.apply_delta(&usage_delta(100, 50));
    accum.apply_delta(&finish_delta("stop"));

    let outcome = accum.into_outcome();
    assert_eq!(outcome.full_content, "Hello world");
    assert_eq!(outcome.full_reasoning, "");
    assert_eq!(outcome.total_tokens, Some(150));
    assert_eq!(outcome.prompt_tokens, Some(100));
    assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
    assert!(outcome.has_effective_output);
    assert!(outcome.tool_calls.is_none());
}

#[test]
fn b7_stream_accumulator_with_reasoning() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&reasoning_delta("Let me think"));
    accum.apply_delta(&reasoning_delta("..."));
    accum.apply_delta(&text_delta("Answer"));
    accum.apply_delta(&finish_delta("stop"));

    let outcome = accum.into_outcome();
    assert_eq!(outcome.full_content, "Answer");
    assert_eq!(outcome.full_reasoning, "Let me think...");
    assert!(outcome.has_effective_output);
}

#[test]
fn b7_stream_accumulator_usage_overwrite() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&usage_delta(100, 20));
    accum.apply_delta(&usage_delta(100, 50));

    assert_eq!(accum.total_tokens(), Some(150));
    assert_eq!(accum.prompt_tokens(), Some(100));
}

#[test]
fn b7_stream_accumulator_empty_delta_no_effective_output() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&ChatDelta::default());
    assert!(!accum.has_effective_output());

    accum.apply_delta(&usage_delta(100, 50));
    assert!(!accum.has_effective_output());
}

#[test]
fn b8_tool_call_single() {
    let mut accum = ToolCallAccumulator::new();

    accum.apply_delta(&tool_call_id_delta("tc_1"));
    accum.apply_delta(&tool_call_name_delta("read_file"));
    accum.apply_delta(&tool_call_args_delta("{\"path\":"));
    accum.apply_delta(&tool_call_args_delta("\"/tmp/x\"}"));

    let calls = accum.finish().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "tc_1");
    assert_eq!(calls[0].name, "read_file");
    assert_eq!(calls[0].arguments, "{\"path\":\"/tmp/x\"}");
}

#[test]
fn b8_tool_call_multiple_finalizes_previous() {
    let mut accum = ToolCallAccumulator::new();

    let outcome1 = accum.apply_delta(&tool_call_id_delta("tc_1"));
    assert!(outcome1.finalized_previous.is_none());

    accum.apply_delta(&tool_call_name_delta("read_file"));
    accum.apply_delta(&tool_call_args_delta("{}"));

    let outcome2 = accum.apply_delta(&tool_call_id_delta("tc_2"));
    assert!(outcome2.finalized_previous.is_some());
    let finalized = outcome2.finalized_previous.unwrap();
    assert_eq!(finalized.id, "tc_1");
    assert_eq!(finalized.name, "read_file");
    assert_eq!(finalized.arguments, "{}");

    accum.apply_delta(&tool_call_name_delta("write_file"));
    accum.apply_delta(&tool_call_args_delta("{\"path\":\"a\"}"));

    let calls = accum.finish().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "tc_2");
    assert_eq!(calls[0].name, "write_file");
}

#[test]
fn b8_tool_call_invalid_json() {
    let mut accum = ToolCallAccumulator::new();

    accum.apply_delta(&tool_call_id_delta("tc_1"));
    accum.apply_delta(&tool_call_name_delta("bad_call"));
    accum.apply_delta(&tool_call_args_delta("not json"));

    let result = accum.finish();
    assert!(result.is_err());
}

#[test]
fn b8_tool_call_params_partial_signal() {
    let mut accum = ToolCallAccumulator::new();

    accum.apply_delta(&tool_call_id_delta("tc_1"));
    let outcome = accum.apply_delta(&tool_call_args_delta("{\"a\":1}"));

    assert!(outcome.params_partial.is_some());
    let chunk = outcome.params_partial.unwrap();
    assert_eq!(chunk.index, 0);
    assert_eq!(chunk.chunk, "{\"a\":1}");
}

#[test]
fn b7_accumulator_apply_delta_returns_outcome() {
    let mut accum = StreamAccumulator::new();

    let outcome: ApplyDeltaOutcome = accum.apply_delta(&text_delta("hi"));
    assert!(outcome.finalized_previous.is_none());
    assert!(outcome.params_partial.is_none());
}

#[test]
fn b7_accumulator_tool_call_sets_effective_output() {
    let mut accum = StreamAccumulator::new();

    accum.apply_delta(&tool_call_id_delta("tc_1"));
    assert!(accum.has_effective_output());
}

// ─── C8: consume_stream 等价性验证 ───

#[tokio::test]
async fn c8_consume_stream_normal_completion() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![
        Ok(text_delta("Hello")),
        Ok(text_delta(" world")),
        Ok(finish_delta("stop")),
    ];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();
    let on_delta = |_seq: u32, _delta: &ChatDelta| {};

    let result = chat_runtime::consume_stream(stream, cancel_token, on_delta, None).await;
    let outcome = result.unwrap();

    assert_eq!(outcome.full_content, "Hello world");
    assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
    assert!(outcome.has_effective_output);
    assert!(outcome.partial_recovery_reason.is_none());
}

#[tokio::test]
async fn c8_consume_stream_cancelled() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![Ok(text_delta("Partial"))];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let on_delta = |_seq: u32, _delta: &ChatDelta| {};
    let result = chat_runtime::consume_stream(stream, cancel_token, on_delta, None).await;

    match result {
        Err(StreamError::Cancelled(_partial)) => {}
        other => panic!("Expected Cancelled, got {:?}", other),
    }
}

#[tokio::test]
async fn c8_consume_stream_cancelled_after_partial_output() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> =
        vec![Ok(text_delta("Partial")), Ok(text_delta(" output"))];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();

    let token_clone = cancel_token.clone();
    let on_delta = move |seq: u32, _delta: &ChatDelta| {
        if seq == 1 {
            token_clone.cancel();
        }
    };

    let result = chat_runtime::consume_stream(stream, cancel_token, on_delta, None).await;

    match result {
        Err(StreamError::Cancelled(partial)) => assert!(partial.has_effective_output),
        other => panic!("Expected Cancelled with output, got {:?}", other),
    }
}

#[tokio::test]
async fn c8_consume_stream_timeout() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![Ok(text_delta("Data"))];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();
    let on_delta = |_seq: u32, _delta: &ChatDelta| {};

    let result = chat_runtime::consume_stream(
        stream,
        cancel_token,
        on_delta,
        Some(std::time::Duration::from_millis(1)),
    )
    .await;

    match result {
        Err(StreamError::Timeout(_partial)) => {}
        Ok(outcome) => {
            assert_eq!(outcome.full_content, "Data");
        }
        other => panic!("Expected Timeout or Ok with partial, got {:?}", other),
    }
}

#[tokio::test]
async fn c8_consume_stream_provider_error() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![
        Ok(text_delta("Before error")),
        Err(ProviderError::StreamError("test error".to_string())),
    ];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();
    let on_delta = |_seq: u32, _delta: &ChatDelta| {};

    let result = chat_runtime::consume_stream(stream, cancel_token, on_delta, None).await;

    match result {
        Err(StreamError::Provider(_, partial)) => assert!(partial.has_effective_output),
        other => panic!("Expected Provider error, got {:?}", other),
    }
}

#[tokio::test]
async fn c8_consume_stream_reasoning_accumulation() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![
        Ok(reasoning_delta("Thinking")),
        Ok(reasoning_delta(" hard")),
        Ok(text_delta("Answer")),
        Ok(finish_delta("stop")),
    ];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();
    let on_delta = |_seq: u32, _delta: &ChatDelta| {};

    let result = chat_runtime::consume_stream(stream, cancel_token, on_delta, None).await;
    let outcome = result.unwrap();

    assert_eq!(outcome.full_reasoning, "Thinking hard");
    assert_eq!(outcome.full_content, "Answer");
    assert!(outcome.has_effective_output);
}

#[tokio::test]
async fn c8_consume_stream_empty_stream() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![];
    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();
    let on_delta = |_seq: u32, _delta: &ChatDelta| {};

    let result = chat_runtime::consume_stream(stream, cancel_token, on_delta, None).await;
    let outcome = result.unwrap();

    assert_eq!(outcome.full_content, "");
    assert!(!outcome.has_effective_output);
}

// ─── C9: on_delta 回调等价性验证 ───

#[tokio::test]
async fn c9_on_delta_callback_sequence() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![
        Ok(text_delta("A")),
        Ok(text_delta("B")),
        Ok(reasoning_delta("R")),
        Ok(finish_delta("stop")),
    ];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();

    let recorded: Arc<Mutex<Vec<(u32, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let recorded_clone = recorded.clone();
    let on_delta = move |seq: u32, delta: &ChatDelta| {
        let text = delta.delta_text.clone().unwrap_or_default();
        let reasoning = delta.delta_reasoning.clone().unwrap_or_default();
        let finish = delta.finish_reason.clone().unwrap_or_default();
        let label = if !text.is_empty() {
            format!("text:{}", text)
        } else if !reasoning.is_empty() {
            format!("reasoning:{}", reasoning)
        } else if !finish.is_empty() {
            format!("finish:{}", finish)
        } else {
            "other".to_string()
        };
        recorded_clone.lock().unwrap().push((seq, label));
    };

    let _ = chat_runtime::consume_stream(stream, cancel_token, on_delta, None).await;

    let calls = recorded.lock().unwrap();
    assert_eq!(calls.len(), 4);
    assert_eq!(calls[0], (1, "text:A".to_string()));
    assert_eq!(calls[1], (2, "text:B".to_string()));
    assert_eq!(calls[2], (3, "reasoning:R".to_string()));
    assert_eq!(calls[3], (4, "finish:stop".to_string()));
}

// ─── D4: prepare_messages 等价性验证 ───

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
    }
}

#[test]
fn d4_prepare_messages_filters_compact_summary() {
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
fn d4_prepare_messages_system_prompt_insertion() {
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
fn d4_prepare_messages_empty_system_prompt_no_insert() {
    let detail = make_detail(vec![make_message_row("user", "hello")]);

    let result = prompt_builder::prepare_messages(&detail, "", None);

    assert_eq!(result.len(), 1);
    assert!(matches!(result[0].role, base::MessageRole::User));
}

#[test]
fn d4_prepare_messages_append_user_prompt_chat() {
    let detail = make_detail(vec![
        make_message_row("user", "hello"),
        make_message_row("assistant", "hi"),
    ]);

    let result = prompt_builder::prepare_messages(&detail, "System", None);

    assert_eq!(result.len(), 3);
    assert!(matches!(result[2].role, base::MessageRole::Assistant));
}

#[test]
fn d4_prepare_messages_append_user_prompt_compaction() {
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

// ─── D5: build_chat_options 等价性验证 ───

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
fn d5_build_chat_options_default_no_temperature() {
    let kind = AgentKind::Default;
    let mode = make_mode("default", None, None);

    let opts = prompt_builder::build_chat_options(&kind, &mode, 128000);

    assert_eq!(opts.temperature, 0.7);
    assert_eq!(opts.max_tokens, 4096);
    assert!(opts.stream);
    assert_eq!(opts.context_window_tokens, 128000);
}

#[test]
fn d5_build_chat_options_default_with_temperature() {
    let kind = AgentKind::Default;
    let mode = make_mode("default", Some(0.5), Some(2048));

    let opts = prompt_builder::build_chat_options(&kind, &mode, 128000);

    assert_eq!(opts.temperature, 0.5);
    assert_eq!(opts.max_tokens, 2048);
}

#[test]
fn d5_build_chat_options_quick() {
    let kind = AgentKind::Quick;
    let mode = make_mode("quick", Some(0.3), Some(1024));

    let opts = prompt_builder::build_chat_options(&kind, &mode, 128000);

    assert_eq!(opts.temperature, 0.3);
    assert_eq!(opts.max_tokens, 1024);
}

#[test]
fn d5_build_chat_options_deep_with_temperature() {
    let kind = AgentKind::Deep;
    let mode = make_mode("deep", Some(0.7), Some(8192));

    let opts = prompt_builder::build_chat_options(&kind, &mode, 128000);

    assert_eq!(opts.temperature, 0.7);
    assert_eq!(opts.max_tokens, 8192);
}

#[test]
fn d5_build_chat_options_deep_no_temperature_fallback() {
    let kind = AgentKind::Deep;
    let mode = make_mode("deep", None, None);

    let opts = prompt_builder::build_chat_options(&kind, &mode, 128000);

    assert_eq!(opts.temperature, 0.3);
    assert_eq!(opts.max_tokens, 4096);
}

#[test]
fn d5_build_chat_options_custom() {
    let kind = AgentKind::Custom;
    let mode = make_mode("custom", Some(0.8), Some(6000));

    let opts = prompt_builder::build_chat_options(&kind, &mode, 128000);

    assert_eq!(opts.temperature, 0.8);
    assert_eq!(opts.max_tokens, 6000);
}

#[test]
fn d5_build_chat_options_reasoning() {
    let kind = AgentKind::Deep;
    let mut mode = make_mode("deep", Some(0.7), Some(8192));
    mode.reasoning_enabled = Some(true);
    mode.reasoning_effort = Some("high".to_string());

    let opts = prompt_builder::build_chat_options(&kind, &mode, 128000);

    assert_eq!(opts.reasoning_enabled, Some(true));
    assert_eq!(opts.reasoning_effort.as_deref(), Some("high"));
}

// ─── D6: AgentKind 映射验证 ───

#[test]
fn d6_agent_kind_from_mode_kind() {
    assert_eq!(AgentKind::from_mode_kind("default"), AgentKind::Default);
    assert_eq!(AgentKind::from_mode_kind("quick"), AgentKind::Quick);
    assert_eq!(AgentKind::from_mode_kind("deep"), AgentKind::Deep);
    assert_eq!(AgentKind::from_mode_kind("custom"), AgentKind::Custom);
    assert_eq!(AgentKind::from_mode_kind("unknown"), AgentKind::Default);
    assert_eq!(AgentKind::from_mode_kind(""), AgentKind::Default);
}

#[test]
fn d6_agent_kind_supports_tool_call() {
    assert!(!AgentKind::Default.supports_tool_call());
    assert!(!AgentKind::Quick.supports_tool_call());
    assert!(!AgentKind::Deep.supports_tool_call());
    assert!(AgentKind::Custom.supports_tool_call());
}

#[test]
fn d6_agent_kind_persists_reasoning() {
    assert!(!AgentKind::Default.persists_reasoning());
    assert!(!AgentKind::Quick.persists_reasoning());
    assert!(AgentKind::Deep.persists_reasoning());
    assert!(!AgentKind::Custom.persists_reasoning());
}

// ─── G4: compaction on_delta 仅 emit delta_text + finish_reason ───

#[tokio::test]
async fn g4_compaction_on_delta_ignores_reasoning_and_tool_call() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![
        Ok(reasoning_delta("thinking")),
        Ok(tool_call_id_delta("tc_1")),
        Ok(tool_call_name_delta("search")),
        Ok(tool_call_args_delta("{}")),
        Ok(text_delta("Summary text")),
        Ok(finish_delta("stop")),
    ];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();

    let emitted_text: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let emitted_finish: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let et = emitted_text.clone();
    let ef = emitted_finish.clone();

    let on_delta = move |_seq: u32, delta: &ChatDelta| {
        if let Some(ref text) = delta.delta_text {
            if !text.is_empty() {
                et.lock().unwrap().push(text.clone());
            }
        }
        if let Some(ref reason) = delta.finish_reason {
            ef.lock().unwrap().push(reason.clone());
        }
    };

    let result = chat_runtime::consume_stream(
        stream,
        cancel_token,
        on_delta,
        Some(std::time::Duration::from_secs(30)),
    )
    .await;
    let outcome = result.unwrap();

    assert_eq!(outcome.full_content, "Summary text");
    assert_eq!(outcome.full_reasoning, "thinking");

    let texts = emitted_text.lock().unwrap();
    assert_eq!(*texts, vec!["Summary text".to_string()]);

    let finishes = emitted_finish.lock().unwrap();
    assert_eq!(*finishes, vec!["stop".to_string()]);
}

// ─── G7: compaction 取消路径 ───

#[tokio::test]
async fn g7_compaction_cancel_returns_partial_outcome() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![
        Ok(text_delta("Partial summary")),
        Ok(text_delta(" content")),
    ];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();

    let token_clone = cancel_token.clone();
    let on_delta = move |seq: u32, _delta: &ChatDelta| {
        if seq == 1 {
            token_clone.cancel();
        }
    };

    let result = chat_runtime::consume_stream(
        stream,
        cancel_token,
        on_delta,
        Some(std::time::Duration::from_secs(30)),
    )
    .await;

    match result {
        Err(StreamError::Cancelled(partial)) => {
            assert!(partial.has_effective_output);
            assert_eq!(partial.full_content, "Partial summary");
        }
        other => panic!("Expected Cancelled with partial, got {:?}", other),
    }
}

#[tokio::test]
async fn g7_compaction_timeout_returns_partial_outcome() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![Ok(text_delta("Partial"))];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();
    let on_delta = |_seq: u32, _delta: &ChatDelta| {};

    let result = chat_runtime::consume_stream(
        stream,
        cancel_token,
        on_delta,
        Some(std::time::Duration::from_millis(1)),
    )
    .await;

    match result {
        Err(StreamError::Timeout(partial)) => {
            assert!(partial.has_effective_output);
            assert_eq!(partial.full_content, "Partial");
        }
        Ok(outcome) => {
            assert_eq!(outcome.full_content, "Partial");
        }
        other => panic!("Expected Timeout or Ok, got {:?}", other),
    }
}

#[tokio::test]
async fn g7_compaction_provider_error_returns_partial_outcome() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![
        Ok(text_delta("Partial summary before error")),
        Err(ProviderError::StreamError("provider failed".to_string())),
    ];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();
    let on_delta = |_seq: u32, _delta: &ChatDelta| {};

    let result = chat_runtime::consume_stream(
        stream,
        cancel_token,
        on_delta,
        Some(std::time::Duration::from_secs(30)),
    )
    .await;

    match result {
        Err(StreamError::Provider(_, partial)) => {
            assert!(partial.has_effective_output);
            assert_eq!(partial.full_content, "Partial summary before error");
        }
        other => panic!("Expected Provider error with partial, got {:?}", other),
    }
}

// ─── G6: compaction 摘要从 outcome.full_content 取值 ───

#[tokio::test]
async fn g6_compaction_outcome_full_content_is_summary() {
    use futures::stream;

    let deltas: Vec<Result<ChatDelta, ProviderError>> = vec![
        Ok(text_delta("The user asked about ")),
        Ok(text_delta("Rust async patterns. ")),
        Ok(text_delta("Key points: tokio::select!")),
        Ok(finish_delta("stop")),
    ];

    let stream = stream::iter(deltas);
    let cancel_token = CancellationToken::new();
    let on_delta = |_seq: u32, _delta: &ChatDelta| {};

    let result = chat_runtime::consume_stream(
        stream,
        cancel_token,
        on_delta,
        Some(std::time::Duration::from_secs(30)),
    )
    .await;

    let outcome = result.unwrap();
    assert_eq!(
        outcome.full_content,
        "The user asked about Rust async patterns. Key points: tokio::select!"
    );
    assert!(outcome.has_effective_output);
    assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
}

// ─── ChatTasks attempt_id 验证 ───

#[test]
fn chat_tasks_start_attempt_id_format() {
    use assistant_agent::chat_tasks::ChatTasks;

    let tasks = ChatTasks::new();

    let (token1, attempt_id1) = tasks.start("conv-1".to_string());
    assert_eq!(attempt_id1, "conv-1:1");
    assert!(!token1.is_cancelled());

    let (token2, attempt_id2) = tasks.start("conv-1".to_string());
    assert_eq!(attempt_id2, "conv-1:2");
    assert!(token1.is_cancelled());
    assert!(!token2.is_cancelled());

    let (token3, attempt_id3) = tasks.start("conv-1".to_string());
    assert_eq!(attempt_id3, "conv-1:3");
    assert!(token2.is_cancelled());
    assert!(!token3.is_cancelled());
}

#[test]
fn chat_tasks_independent_conversations() {
    use assistant_agent::chat_tasks::ChatTasks;

    let tasks = ChatTasks::new();

    let (token_a1, attempt_a1) = tasks.start("conv-a".to_string());
    let (token_b1, attempt_b1) = tasks.start("conv-b".to_string());

    assert_eq!(attempt_a1, "conv-a:1");
    assert_eq!(attempt_b1, "conv-b:1");
    assert!(!token_a1.is_cancelled());
    assert!(!token_b1.is_cancelled());

    let (token_a2, attempt_a2) = tasks.start("conv-a".to_string());
    assert_eq!(attempt_a2, "conv-a:2");
    assert!(token_a1.is_cancelled());
    assert!(!token_b1.is_cancelled());
    drop(token_a2);
}

#[test]
fn chat_tasks_cancel_and_finish() {
    use assistant_agent::chat_tasks::ChatTasks;

    let tasks = ChatTasks::new();

    let (token, _) = tasks.start("conv-1".to_string());
    assert!(tasks.is_running("conv-1"));

    tasks.cancel("conv-1");
    assert!(token.is_cancelled());
    assert!(!tasks.is_running("conv-1"));

    let (token2, attempt_id2) = tasks.start("conv-1".to_string());
    assert_eq!(attempt_id2, "conv-1:1");
    assert!(!token2.is_cancelled());

    tasks.finish("conv-1");
    assert!(!tasks.is_running("conv-1"));
}
