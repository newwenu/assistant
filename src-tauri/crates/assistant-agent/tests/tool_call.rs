mod common;

use assistant_agent::agent::tool_call::repair_truncated_json;
use assistant_agent::agent::tool_call::ToolCallAccumulator;
use assistant_agent::agent::tool_call::ToolCallBoundary;
use assistant_agent::agent::tool_call::ToolCallStreamKey;
use assistant_providers::base::ChatDelta;
use assistant_providers::types::unified::UnifiedResponse;
use assistant_providers::types::unified::UnifiedToolCall;
use common::chat::delta_tool_call as delta;

#[test]
fn accumulates_fragments_across_chunks() {
    let mut acc = ToolCallAccumulator::new();

    acc.apply_delta(
        ToolCallStreamKey::Indexed(0),
        Some("call_1".to_string()),
        Some("search".to_string()),
        Some("{\"q\":".to_string()),
        false,
    );
    acc.apply_delta(
        ToolCallStreamKey::Indexed(0),
        None,
        None,
        Some("\"test\"}".to_string()),
        false,
    );

    let finalized = acc.finalize_all(ToolCallBoundary::FinishReason);
    assert_eq!(finalized.len(), 1);
    assert_eq!(finalized[0].tool_id, "call_1");
    assert_eq!(finalized[0].tool_name, "search");
    assert_eq!(finalized[0].arguments, serde_json::json!({"q": "test"}));
    assert!(!finalized[0].is_error);
}

#[test]
fn chat_delta_fragments_feed_accumulator() {
    let mut acc = ToolCallAccumulator::new();

    acc.apply_chat_delta(&delta(Some("call_1"), Some("read"), None));
    acc.apply_chat_delta(&delta(None, None, Some("{\"path\":")));
    acc.apply_chat_delta(&delta(None, None, Some("\"a.txt\"}")));

    let finalized = acc.finalize_all(ToolCallBoundary::FinishReason);
    assert_eq!(finalized.len(), 1);
    assert_eq!(finalized[0].raw_arguments, "{\"path\":\"a.txt\"}");
}

#[test]
fn snapshot_without_id_starts_pending_call() {
    let mut acc = ToolCallAccumulator::new();

    acc.apply_chat_delta(&ChatDelta {
        delta_tool_call_name: Some("read".to_string()),
        delta_tool_call_arguments: Some("{\"path\":\"a.txt\"}".to_string()),
        delta_tool_call_index: Some(1),
        delta_tool_call_arguments_is_snapshot: true,
        ..Default::default()
    });

    let finalized = acc.finalize_all(ToolCallBoundary::StreamEnd);
    assert_eq!(finalized.len(), 1);
    assert_eq!(finalized[0].tool_id, "call_1");
    assert_eq!(finalized[0].tool_name, "read");
    assert_eq!(finalized[0].arguments, serde_json::json!({"path": "a.txt"}));
}

#[test]
fn snapshot_replaces_accumulated_arguments() {
    let mut acc = ToolCallAccumulator::new();

    acc.apply_delta(
        ToolCallStreamKey::Indexed(0),
        Some("call_1".to_string()),
        Some("tool_a".to_string()),
        Some("{\"partial\"".to_string()),
        false,
    );
    acc.apply_delta(
        ToolCallStreamKey::Indexed(0),
        None,
        None,
        Some("{\"full\":true}".to_string()),
        true,
    );

    let finalized = acc.finalize_all(ToolCallBoundary::FinishReason);
    assert_eq!(finalized[0].raw_arguments, "{\"full\":true}");
}

#[test]
fn invalid_json_becomes_error() {
    let mut acc = ToolCallAccumulator::new();

    acc.apply_delta(
        ToolCallStreamKey::Indexed(0),
        Some("call_1".to_string()),
        Some("tool_a".to_string()),
        Some("{\"a\":".to_string()),
        false,
    );

    let finalized = acc.finalize_all(ToolCallBoundary::StreamEnd);
    assert_eq!(finalized.len(), 1);
    assert!(finalized[0].is_error);
    assert_eq!(finalized[0].raw_arguments, "{\"a\":");
}

#[test]
fn new_tool_finalizes_previous() {
    let mut acc = ToolCallAccumulator::new();

    acc.apply_delta(
        ToolCallStreamKey::Indexed(0),
        Some("call_1".to_string()),
        Some("tool_a".to_string()),
        Some("{\"x\":1}".to_string()),
        false,
    );

    let outcome = acc.apply_delta(
        ToolCallStreamKey::Indexed(0),
        Some("call_2".to_string()),
        Some("tool_b".to_string()),
        None,
        false,
    );

    let finalized = outcome.finalized_previous.unwrap();
    assert_eq!(finalized.tool_id, "call_1");
    assert_eq!(finalized.tool_name, "tool_a");
}

#[test]
fn apply_unified_response() {
    let mut acc = ToolCallAccumulator::new();

    let response = UnifiedResponse {
        tool_call: Some(UnifiedToolCall {
            tool_call_index: Some(0),
            id: Some("tc_1".to_string()),
            name: Some("search".to_string()),
            arguments: Some("{\"q\":\"hello\"}".to_string()),
            arguments_is_snapshot: false,
        }),
        ..Default::default()
    };

    acc.apply_unified_response(&response);

    let finalized = acc.finalize_all(ToolCallBoundary::FinishReason);
    assert_eq!(finalized.len(), 1);
    assert_eq!(finalized[0].tool_id, "tc_1");
}

#[test]
fn three_dialects_produce_equivalent_finalized_call() {
    let openai = {
        let mut acc = ToolCallAccumulator::new();
        acc.apply_chat_delta(&ChatDelta {
            delta_tool_call_id: Some("call_1".to_string()),
            delta_tool_call_name: Some("read_file".to_string()),
            delta_tool_call_arguments: Some("{\"path\":".to_string()),
            delta_tool_call_index: Some(0),
            delta_tool_call_arguments_is_snapshot: false,
            ..Default::default()
        });
        acc.apply_chat_delta(&ChatDelta {
            delta_tool_call_arguments: Some("\"a.txt\"}".to_string()),
            delta_tool_call_index: Some(0),
            delta_tool_call_arguments_is_snapshot: false,
            ..Default::default()
        });
        acc.finalize_all(ToolCallBoundary::StreamEnd)
    };

    let anthropic = {
        let mut acc = ToolCallAccumulator::new();
        acc.apply_chat_delta(&ChatDelta {
            delta_tool_call_id: Some("call_1".to_string()),
            delta_tool_call_name: Some("read_file".to_string()),
            delta_tool_call_arguments: Some("{\"path\":".to_string()),
            delta_tool_call_index: Some(0),
            delta_tool_call_arguments_is_snapshot: false,
            ..Default::default()
        });
        acc.apply_chat_delta(&ChatDelta {
            delta_tool_call_arguments: Some("\"a.txt\"}".to_string()),
            delta_tool_call_index: Some(0),
            delta_tool_call_arguments_is_snapshot: false,
            ..Default::default()
        });
        acc.finalize_all(ToolCallBoundary::FinishReason)
    };

    let gemini = {
        let mut acc = ToolCallAccumulator::new();
        acc.apply_chat_delta(&ChatDelta {
            delta_tool_call_name: Some("read_file".to_string()),
            delta_tool_call_arguments: Some("{\"path\":\"a.txt\"}".to_string()),
            delta_tool_call_index: Some(0),
            delta_tool_call_arguments_is_snapshot: true,
            ..Default::default()
        });
        acc.finalize_all(ToolCallBoundary::StreamEnd)
    };

    let arguments = serde_json::json!({ "path": "a.txt" });
    for finalized in [&openai, &anthropic, &gemini] {
        assert_eq!(finalized.len(), 1);
        assert_eq!(finalized[0].tool_name, "read_file");
        assert_eq!(finalized[0].arguments, arguments);
        assert!(!finalized[0].is_error);
    }
}

#[test]
fn repair_truncated_json_cases() {
    assert_eq!(
        repair_truncated_json("{\"a\":1"),
        Some("{\"a\":1}".to_string())
    );
    assert_eq!(
        repair_truncated_json("{\"a\":[1,2"),
        Some("{\"a\":[1,2]}".to_string())
    );
    assert_eq!(
        repair_truncated_json("{\"a\":\"hello"),
        Some("{\"a\":\"hello\"}".to_string())
    );
    assert_eq!(repair_truncated_json("{\"a\":"), None);
}
