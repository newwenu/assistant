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

#[test]
fn args_delta_opens_once_and_emits_appended_chunks() {
    let mut acc = ToolCallAccumulator::new();

    let first = acc.apply_chat_delta(&delta(Some("call_1"), Some("read"), Some("{\"a\":1")));
    let opened = first.opened.expect("首个片段必须产出身份声明");
    assert_eq!(opened.tool_id, "call_1");
    assert_eq!(opened.tool_name, "read");
    assert_eq!(opened.params_chunk, "");
    assert_eq!(
        first.params_partial.expect("首个片段应带参数").params_chunk,
        "{\"a\":1"
    );

    let second = acc.apply_chat_delta(&delta(None, None, Some("}")));
    assert!(second.opened.is_none(), "身份声明只发一次");
    assert_eq!(second.params_partial.unwrap().params_chunk, "}");
}

#[test]
fn snapshot_chunks_emit_only_suffix() {
    let mut acc = ToolCallAccumulator::new();

    let first = acc.apply_chat_delta(&ChatDelta {
        delta_tool_call_name: Some("read".to_string()),
        delta_tool_call_arguments: Some("{\"a\":1".to_string()),
        delta_tool_call_index: Some(0),
        delta_tool_call_arguments_is_snapshot: true,
        ..Default::default()
    });
    assert!(first.opened.is_some());
    assert_eq!(first.params_partial.unwrap().params_chunk, "{\"a\":1");

    let second = acc.apply_chat_delta(&ChatDelta {
        delta_tool_call_arguments: Some("{\"a\":1,\"b\":2}".to_string()),
        delta_tool_call_index: Some(0),
        delta_tool_call_arguments_is_snapshot: true,
        ..Default::default()
    });
    assert!(second.opened.is_none());
    assert_eq!(
        second.params_partial.unwrap().params_chunk,
        ",\"b\":2}",
        "快照型只发后缀"
    );

    let finalized = acc.finalize_all(ToolCallBoundary::StreamEnd);
    assert_eq!(finalized[0].raw_arguments, "{\"a\":1,\"b\":2}");
}

#[test]
fn non_prefix_snapshot_stops_emitting() {
    let mut acc = ToolCallAccumulator::new();

    acc.apply_chat_delta(&ChatDelta {
        delta_tool_call_name: Some("read".to_string()),
        delta_tool_call_arguments: Some("{\"a\":1}".to_string()),
        delta_tool_call_index: Some(0),
        delta_tool_call_arguments_is_snapshot: true,
        ..Default::default()
    });

    let non_prefix = acc.apply_chat_delta(&ChatDelta {
        delta_tool_call_arguments: Some("{\"b\":2}".to_string()),
        delta_tool_call_index: Some(0),
        delta_tool_call_arguments_is_snapshot: true,
        ..Default::default()
    });
    assert!(
        non_prefix.params_partial.is_none(),
        "非前缀快照不可续，停发"
    );

    let afterwards = acc.apply_chat_delta(&ChatDelta {
        delta_tool_call_arguments: Some("{\"b\":2,\"c\":3}".to_string()),
        delta_tool_call_index: Some(0),
        delta_tool_call_arguments_is_snapshot: true,
        ..Default::default()
    });
    assert!(afterwards.params_partial.is_none(), "关闭后不再恢复发射");

    let finalized = acc.finalize_all(ToolCallBoundary::StreamEnd);
    assert_eq!(finalized[0].raw_arguments, "{\"b\":2,\"c\":3}");
}

#[test]
fn args_delta_caps_at_4096_bytes() {
    let mut acc = ToolCallAccumulator::new();

    let first = acc.apply_chat_delta(&delta(
        Some("call_1"),
        Some("write"),
        Some(&"a".repeat(3000)),
    ));
    let chunk1 = first.params_partial.unwrap().params_chunk;
    assert_eq!(chunk1.len(), 3000);

    let second = acc.apply_chat_delta(&delta(None, None, Some(&"b".repeat(3000))));
    let chunk2 = second.params_partial.unwrap().params_chunk;
    assert_eq!(chunk2.len(), 1096, "截断到余量");
    assert_eq!(chunk1.len() + chunk2.len(), 4096);

    let third = acc.apply_chat_delta(&delta(None, None, Some("c")));
    assert!(third.params_partial.is_none(), "触顶后停发");
}

#[test]
fn discard_pending_reports_announced_calls() {
    let mut acc = ToolCallAccumulator::new();

    let first = acc.apply_chat_delta(&delta(Some("call_1"), Some("write"), Some("{\"a\":")));
    assert!(
        first.opened.is_some(),
        "前置条件：身份已通过 ArgsDelta 声明"
    );

    let discarded = acc.discard_pending();
    assert_eq!(discarded.len(), 1, "被声明的调用必须交回补发终态");
    assert_eq!(discarded[0].tool_id, "call_1");
    assert_eq!(discarded[0].tool_name, "write");

    assert!(!acc.has_pending(), "丢弃后不再有 pending");
    assert!(
        acc.finalize_all(ToolCallBoundary::StreamEnd).is_empty(),
        "丢弃的调用不得进入执行"
    );
}

#[test]
fn discard_pending_on_empty_accumulator_is_noop() {
    let mut acc = ToolCallAccumulator::new();
    assert!(acc.discard_pending().is_empty(), "空响应重试时无事可补发");
}

#[test]
fn args_delta_truncates_on_char_boundary() {
    let mut acc = ToolCallAccumulator::new();

    acc.apply_chat_delta(&delta(
        Some("call_1"),
        Some("write"),
        Some(&"中".repeat(1000)),
    ));
    let second = acc.apply_chat_delta(&delta(None, None, Some(&"文".repeat(1000))));
    let chunk = second.params_partial.unwrap().params_chunk;
    assert_eq!(
        chunk.len(),
        1095,
        "4096 - 3000 = 1096 非 3 的倍数，退到字符边界"
    );
    assert!(chunk.chars().all(|c| c == '文'));
}
