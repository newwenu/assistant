use assistant_agent::agent::ladder::{
    budget_of, hard_trim, microcompact, restore_tool_bodies, split_input_allowance,
    CLEARED_PLACEHOLDER_PREFIX,
};
use assistant_providers::types::{ChatMessage, MessageRole, ToolCall};

fn text_msg(role: MessageRole, content: &str) -> ChatMessage {
    ChatMessage {
        role,
        content: content.to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        is_error: false,
        thinking_signature: None,
        content_blocks: None,
    }
}

fn tool_msg(name: &str, id: &str, body: &str) -> ChatMessage {
    let mut msg = text_msg(MessageRole::Tool, body);
    msg.name = Some(name.to_string());
    msg.tool_call_id = Some(id.to_string());
    msg
}

fn assistant_call(id: &str, name: &str) -> ChatMessage {
    let mut msg = text_msg(MessageRole::Assistant, "");
    msg.tool_calls = Some(vec![ToolCall {
        id: id.to_string(),
        name: name.to_string(),
        arguments: "{}".to_string(),
    }]);
    msg
}

#[test]
fn budget_deducts_output_reserve_and_safety() {
    assert_eq!(budget_of(Some(10_000), 1_000), Some(8_000));
    assert_eq!(budget_of(Some(0), 1_000), None);
    assert_eq!(budget_of(None, 1_000), None);
    assert_eq!(budget_of(Some(500), 1_000), Some(0));
}

#[test]
fn microcompact_skips_when_under_budget() {
    let body = "a".repeat(4_000);
    let mut messages = vec![
        text_msg(MessageRole::System, "sys"),
        tool_msg("read", "c1", &body),
    ];
    let cleared = microcompact(&mut messages, Some(100_000), 6);
    assert!(cleared.is_empty());
    assert_eq!(messages[1].content, body);
}

#[test]
fn microcompact_blanks_only_older_than_keep_recent() {
    let body = "b".repeat(4_000);
    let mut messages = vec![
        text_msg(MessageRole::System, "sys"),
        tool_msg("read", "c1", &body),
        tool_msg("read", "c2", &body),
        tool_msg("grep", "c3", &body),
    ];
    let cleared = microcompact(&mut messages, Some(1_500), 1);
    assert_eq!(cleared.len(), 2);
    assert_eq!(cleared[0].index, 1);
    assert_eq!(cleared[1].index, 2);
    assert!(messages[1].content.starts_with(CLEARED_PLACEHOLDER_PREFIX));
    assert!(messages[1].content.contains("read"));
    assert_eq!(messages[3].content, body);
}

#[test]
fn microcompact_disabled_when_keep_recent_is_zero() {
    let mut messages = vec![tool_msg("read", "c1", &"b".repeat(4_000))];
    let cleared = microcompact(&mut messages, Some(100), 0);
    assert!(cleared.is_empty());
    assert!(!messages[0].content.starts_with(CLEARED_PLACEHOLDER_PREFIX));
}

#[test]
fn microcompact_noop_when_window_unknown() {
    let mut messages = vec![tool_msg("read", "c1", &"b".repeat(4_000))];
    let cleared = microcompact(&mut messages, None, 6);
    assert!(cleared.is_empty());
}

#[test]
fn restore_respects_allowance_and_prefers_recent() {
    let mut messages = vec![
        tool_msg("read", "c1", &"x".repeat(4_000)),
        tool_msg("read", "c2", &"y".repeat(4_000)),
        tool_msg("read", "c3", "latest"),
    ];
    let cleared = microcompact(&mut messages, Some(100), 1);
    assert_eq!(cleared.len(), 2);
    assert!(messages[0].content.starts_with(CLEARED_PLACEHOLDER_PREFIX));
    assert!(messages[1].content.starts_with(CLEARED_PLACEHOLDER_PREFIX));

    let cost = cleared[1].original_tokens - cleared[1].placeholder_tokens;
    let restored = restore_tool_bodies(&mut messages, &cleared, cost);
    assert_eq!(restored, 1);
    assert!(messages[0].content.starts_with(CLEARED_PLACEHOLDER_PREFIX));
    assert_eq!(messages[1].content, "y".repeat(4_000));
}

#[test]
fn restore_with_zero_allowance_keeps_placeholders() {
    let mut messages = vec![
        tool_msg("read", "c1", &"x".repeat(4_000)),
        tool_msg("read", "c2", &"y".repeat(4_000)),
        tool_msg("read", "c3", "latest"),
    ];
    let cleared = microcompact(&mut messages, Some(100), 1);
    assert_eq!(cleared.len(), 2);
    assert_eq!(restore_tool_bodies(&mut messages, &cleared, 0), 0);
    assert!(messages[0].content.starts_with(CLEARED_PLACEHOLDER_PREFIX));
    assert!(messages[1].content.starts_with(CLEARED_PLACEHOLDER_PREFIX));
}

#[test]
fn allowance_split_is_three_to_one() {
    assert_eq!(split_input_allowance(10_000, 6_000), (3_000, 1_000));
    assert_eq!(split_input_allowance(1_000, 2_000), (0, 0));
    assert_eq!(split_input_allowance(0, 0), (0, 0));
}

#[test]
fn hard_trim_keeps_system_and_last_user() {
    let big = "x".repeat(4_000);
    let mut messages = vec![
        text_msg(MessageRole::System, "sys"),
        text_msg(MessageRole::User, &big),
        text_msg(MessageRole::Assistant, &big),
        text_msg(MessageRole::User, "current"),
    ];
    let evicted = hard_trim(&mut messages, 2_000);
    assert_eq!(evicted, 1);
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].role, MessageRole::System);
    assert_eq!(messages[2].content, "current");
}

#[test]
fn hard_trim_removes_paired_tool_rows_together() {
    let big = "x".repeat(4_000);
    let mut messages = vec![
        text_msg(MessageRole::System, "sys"),
        text_msg(MessageRole::User, "do it"),
        assistant_call("c1", "read"),
        tool_msg("read", "c1", &big),
        text_msg(MessageRole::User, "current"),
    ];
    let evicted = hard_trim(&mut messages, 1_000);
    assert_eq!(evicted, 3);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, MessageRole::System);
    assert_eq!(messages[1].content, "current");
}

#[test]
fn hard_trim_stops_when_nothing_evictable() {
    let mut messages = vec![text_msg(MessageRole::System, &"s".repeat(4_000))];
    let evicted = hard_trim(&mut messages, 1_000);
    assert_eq!(evicted, 0);
    assert_eq!(messages.len(), 1);
}
