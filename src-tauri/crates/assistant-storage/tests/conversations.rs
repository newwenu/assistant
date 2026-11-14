mod common;

use assistant_storage::conversations;
use common::{conv, init_db, msg, msg_with_parent, provider};

#[test]
fn delete_conversation_removes_all_messages() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "user", "a", 100)).unwrap();
    conversations::insert_message(&conn, &msg("m2", "c1", "assistant", "b", 200)).unwrap();

    conversations::delete_conversation(&conn, "c1").unwrap();
    assert_eq!(conversations::get_message_count(&conn, "c1").unwrap(), 0);
    assert!(conversations::get_conversation_by_id(&conn, "c1")
        .unwrap()
        .is_none());
}

#[test]
fn tool_result_rows_do_not_create_branch_siblings() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "", 200, Some("u1"), 0),
    )
    .unwrap();
    conversations::update_message_tool_calls(
        &conn,
        "a1",
        r#"[{"id":"call_1","name":"fs_read","arguments":"{}"}]"#,
    )
    .unwrap();

    for (id, ts) in [("t1", 300), ("t2", 400)] {
        let mut row = msg_with_parent(id, "c1", "tool", "result", ts, Some("a1"), 0);
        row.tool_call_id = Some("call_1".to_string());
        conversations::insert_message(&conn, &row).unwrap();
    }

    assert_eq!(
        conversations::get_sibling_count(&conn, "c1", Some("u1")).unwrap(),
        1,
        "tool rows must not add siblings under the user message"
    );
    let rows = conversations::get_messages(&conn, "c1").unwrap();
    let tools: Vec<_> = rows.iter().filter(|row| row.role == "tool").collect();
    assert_eq!(tools.len(), 2);
    assert!(
        tools.iter().all(|row| row.sibling_index == 0),
        "tool rows always use sibling_index 0"
    );
    assert!(
        tools
            .iter()
            .all(|row| row.tool_call_id.as_deref() == Some("call_1")),
        "tool rows keep tool_call_id"
    );
}

#[test]
fn tool_history_roundtrip_preserves_order_and_fields() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "", 200, Some("u1"), 0),
    )
    .unwrap();
    conversations::update_message_tool_calls(
        &conn,
        "a1",
        r#"[{"id":"call_1","name":"fs_read","arguments":"{\"path\":\"a\"}"},{"id":"call_2","name":"fs_list","arguments":"{}"}]"#,
    )
    .unwrap();

    for (id, call_id, content, is_error, ts) in [
        ("t1", "call_1", "ok", false, 300),
        ("t2", "call_2", "boom", true, 400),
    ] {
        let mut row = msg_with_parent(id, "c1", "tool", content, ts, Some("a1"), 0);
        row.tool_call_id = Some(call_id.to_string());
        row.is_error = is_error;
        conversations::insert_message(&conn, &row).unwrap();
    }

    let rows = conversations::get_messages(&conn, "c1").unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].role, "user");
    assert_eq!(rows[1].role, "assistant");
    let tool_calls_json = rows[1]
        .tool_calls_json
        .as_deref()
        .expect("assistant row keeps tool_calls_json");
    assert!(tool_calls_json.contains("call_1"), "{tool_calls_json}");
    assert!(tool_calls_json.contains("call_2"), "{tool_calls_json}");
    assert_eq!(rows[2].role, "tool");
    assert_eq!(rows[2].tool_call_id.as_deref(), Some("call_1"));
    assert!(!rows[2].is_error);
    assert_eq!(rows[3].role, "tool");
    assert_eq!(rows[3].tool_call_id.as_deref(), Some("call_2"));
    assert!(rows[3].is_error);

    let results = conversations::get_tool_results(&conn, "c1").unwrap();
    assert_eq!(
        results,
        vec![
            ("call_1".to_string(), "ok".to_string(), false),
            ("call_2".to_string(), "boom".to_string(), true),
        ]
    );
}

#[test]
fn get_tool_results_isolates_conversations() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_conversation(&conn, &conv("c2")).unwrap();
    for (conv_id, id, call_id) in [("c1", "t1", "call_1"), ("c2", "t2", "call_2")] {
        let mut row = msg(id, conv_id, "tool", "result", 100);
        row.tool_call_id = Some(call_id.to_string());
        conversations::insert_message(&conn, &row).unwrap();
    }

    let results = conversations::get_tool_results(&conn, "c1").unwrap();
    assert_eq!(
        results,
        vec![("call_1".to_string(), "result".to_string(), false)]
    );
}

#[test]
fn toggle_pin_is_involution_and_sorts() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();

    let pinned = conversations::toggle_pin(&conn, "c1").unwrap();
    assert!(pinned);
    let c = conversations::get_conversation_by_id(&conn, "c1")
        .unwrap()
        .unwrap();
    assert!(c.is_pinned);
    assert!(
        c.sort_order > 0,
        "pinned conversation should have positive sort_order"
    );

    let unpinned = conversations::toggle_pin(&conn, "c1").unwrap();
    assert!(!unpinned);
    let c = conversations::get_conversation_by_id(&conn, "c1")
        .unwrap()
        .unwrap();
    assert!(!c.is_pinned);
    assert_eq!(
        c.sort_order, 0,
        "unpinned conversation should reset sort_order"
    );
}

#[test]
fn pinned_conversations_appear_first() {
    let conn = init_db();
    let mut c1 = conv("c1");
    c1.updated_at = 3000;
    let mut c2 = conv("c2");
    c2.updated_at = 1000;
    conversations::insert_conversation(&conn, &c1).unwrap();
    conversations::insert_conversation(&conn, &c2).unwrap();

    conversations::toggle_pin(&conn, "c2").unwrap();

    let all = conversations::get_conversations(&conn, None).unwrap();
    assert_eq!(
        all[0].id, "c2",
        "pinned conversation should appear first even with older updated_at"
    );
    assert_eq!(all[1].id, "c1");
}

#[test]
fn search_returns_distinct_results() {
    let conn = init_db();
    let mut c = conv("c1");
    c.title = "Rust guide".to_string();
    conversations::insert_conversation(&conn, &c).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "user", "Learn Rust today", 100))
        .unwrap();

    let results = conversations::search_conversations(&conn, "Rust").unwrap();
    assert_eq!(
        results.len(),
        1,
        "conversation matching both title and content should appear once"
    );
}

#[test]
fn search_with_special_characters() {
    let conn = init_db();
    let mut c = conv("c1");
    c.title = "C++ & Rust".to_string();
    conversations::insert_conversation(&conn, &c).unwrap();

    let results = conversations::search_conversations(&conn, "C++").unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn search_empty_query_returns_all() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_conversation(&conn, &conv("c2")).unwrap();

    let results = conversations::search_conversations(&conn, "").unwrap();
    assert_eq!(
        results.len(),
        2,
        "empty query matches everything via LIKE '%%'"
    );
}

#[test]
fn duplicate_creates_independent_copy() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "user", "hello", 100)).unwrap();

    let new_id = conversations::duplicate_conversation(&conn, "c1").unwrap();

    conversations::delete_conversation(&conn, "c1").unwrap();
    let detail = conversations::get_conversation_detail(&conn, &new_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.messages.len(),
        1,
        "copy should survive original deletion"
    );
    assert_eq!(
        detail.conversation.title, "test",
        "title stays untouched; the copy suffix is rendered by the UI"
    );
    assert_eq!(
        detail.conversation.title_source,
        conversations::COPY_TITLE_SOURCE
    );
}

#[test]
fn pagination_covers_all_messages_without_overlap_or_gap() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    // 20 轮，每轮 user + assistant 两行；游标按 turn 走，页边界不得切开 turn。
    for t in 1..=20i64 {
        for (offset, role) in [(0i64, "user"), (1i64, "assistant")] {
            let mut row = msg(
                &format!("t{t}-{role}"),
                "c1",
                role,
                &format!("msg {t} {role}"),
                t * 100 + offset,
            );
            row.turn_index = Some(t);
            row.iteration = offset;
            conversations::insert_message(&conn, &row).unwrap();
        }
    }

    let page1 = conversations::get_messages_paginated(&conn, "c1", 50, None).unwrap();
    assert_eq!(
        page1.len(),
        (conversations::TURNS_PER_PAGE as usize) * 2,
        "first page covers TURNS_PER_PAGE turns, both rows each"
    );
    assert_eq!(
        page1.first().unwrap().turn_index,
        Some(20 - (conversations::TURNS_PER_PAGE - 1)),
        "first page is the newest turn window, ascending"
    );

    let cursor = u32::try_from(page1.first().unwrap().turn_index.unwrap()).unwrap();
    let page2 = conversations::get_messages_paginated(&conn, "c1", 50, Some(cursor)).unwrap();
    assert_eq!(
        page2.len(),
        (20 - conversations::TURNS_PER_PAGE as usize) * 2,
        "remaining turns, whole"
    );
    assert_eq!(
        page2.last().unwrap().turn_index,
        Some(20 - conversations::TURNS_PER_PAGE),
        "page boundary never splits a turn"
    );

    let all_ids: Vec<&str> = page1
        .iter()
        .chain(page2.iter())
        .map(|m| m.id.as_str())
        .collect();
    let unique: std::collections::HashSet<&str> = all_ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        40,
        "pages should cover all messages without overlap"
    );
}

#[test]
fn limit_guardrail_never_splits_a_turn() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    // 每轮 5 行，护栏 limit=7 落在第 2 轮中间，必须停在轮边界而不是切轮。
    for t in 1..=3i64 {
        for iteration in 0..5i64 {
            let mut row = msg(
                &format!("t{t}-{iteration}"),
                "c1",
                if iteration == 0 { "user" } else { "assistant" },
                "x",
                t * 100 + iteration,
            );
            row.turn_index = Some(t);
            row.iteration = iteration;
            conversations::insert_message(&conn, &row).unwrap();
        }
    }

    let page = conversations::get_messages_paginated(&conn, "c1", 7, None).unwrap();
    assert_eq!(page.len(), 5, "护栏截断点落在第 1 轮末尾，不切第 2 轮");
    assert!(
        page.iter().all(|m| m.turn_index == Some(3)),
        "最新一页从最大 turn 起，且只含整轮"
    );

    // 单轮自身超过护栏：整轮返回，宁可超出也不切开。
    let oversized = conversations::get_messages_paginated(&conn, "c1", 3, None).unwrap();
    assert_eq!(oversized.len(), 5, "最新一轮自身超限时整轮返回");
    assert!(oversized.iter().all(|m| m.turn_index == Some(3)));
}

#[test]
fn pagination_keeps_unresolved_segment_out_of_turn_pages() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    let mut legacy = msg("legacy", "c1", "user", "old", 10);
    legacy.turn_index = None;
    conversations::insert_message(&conn, &legacy).unwrap();
    let mut row = msg("t1", "c1", "user", "new", 100);
    row.turn_index = Some(1);
    row.iteration = 0;
    conversations::insert_message(&conn, &row).unwrap();

    let first = conversations::get_messages_paginated(&conn, "c1", 50, None).unwrap();
    assert_eq!(first.len(), 2, "首屏含最新 turn 与存量未解析段");
    assert_eq!(first.first().unwrap().id, "t1", "有坐标的行在前，升序输出");
    assert_eq!(
        first.last().unwrap().id,
        "legacy",
        "存量未解析段按排序规则垫底"
    );

    let second = conversations::get_messages_paginated(&conn, "c1", 50, Some(1)).unwrap();
    assert!(second.is_empty(), "turn 游标耗尽后不再重复返回");
}

#[test]
fn allocate_turn_index_is_monotonic_and_persisted() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    let t1 = conversations::allocate_turn_index(&conn, "c1").unwrap();
    let t2 = conversations::allocate_turn_index(&conn, "c1").unwrap();
    assert_eq!((t1, t2), (1, 2), "首轮 1，逐轮递增");

    let mut row = msg("m1", "c1", "user", "hi", 100);
    row.turn_index = Some(i64::from(t1));
    row.iteration = 0;
    conversations::insert_message(&conn, &row).unwrap();
    let t3 = conversations::allocate_turn_index(&conn, "c1").unwrap();
    assert_eq!(t3, 3, "计数器落库，不依赖 MAX(turn_index) 重算");
}

#[test]
fn allocate_iteration_counts_within_turn_only() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    for (turn, iteration) in [(1i64, 0i64), (1, 1), (1, 2), (2, 0)] {
        let mut row = msg(
            &format!("t{turn}-{iteration}"),
            "c1",
            "assistant",
            "x",
            turn * 100 + iteration,
        );
        row.turn_index = Some(turn);
        row.iteration = iteration;
        conversations::insert_message(&conn, &row).unwrap();
    }

    assert_eq!(
        conversations::allocate_iteration(&conn, "c1", 1).unwrap(),
        3,
        "同 turn 内递增"
    );
    assert_eq!(
        conversations::allocate_iteration(&conn, "c1", 2).unwrap(),
        1,
        "跨 turn 不累加"
    );
    assert_eq!(
        conversations::allocate_iteration(&conn, "c1", 3).unwrap(),
        1,
        "空 turn 从 1 起（0 保留给 user 行）"
    );
}

#[test]
fn increment_generation_monotonic() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    let g1 = conversations::increment_generation(&conn, "c1").unwrap();
    let g2 = conversations::increment_generation(&conn, "c1").unwrap();
    let g3 = conversations::increment_generation(&conn, "c1").unwrap();
    assert!(g1 < g2 && g2 < g3, "generation must be strictly monotonic");
}

#[test]
fn provider_filter_isolates_correct_subset() {
    let conn = init_db();
    let mut c1 = conv("c1");
    c1.provider_id = Some("p1".to_string());
    let mut c2 = conv("c2");
    c2.provider_id = Some("p2".to_string());
    let mut c3 = conv("c3");
    c3.provider_id = None;
    conversations::insert_conversation(&conn, &c1).unwrap();
    conversations::insert_conversation(&conn, &c2).unwrap();
    conversations::insert_conversation(&conn, &c3).unwrap();

    let p1 = conversations::get_conversations(&conn, Some("p1")).unwrap();
    assert_eq!(p1.len(), 1);
    assert_eq!(p1[0].id, "c1");

    let unfiltered = conversations::get_conversations(&conn, None).unwrap();
    assert_eq!(unfiltered.len(), 3);
}

#[test]
fn export_json_roundtrip() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "user", "hello 世界", 100)).unwrap();

    let json = conversations::export_conversation_json(&conn, "c1").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["messages"][0]["content"], "hello 世界");
}

#[test]
fn delete_provider_nullifies_conversation_provider_id() {
    let conn = init_db();
    assistant_storage::providers::insert_provider(&conn, &provider("p1", 0)).unwrap();
    let mut c = conv("c1");
    c.provider_id = Some("p1".to_string());
    conversations::insert_conversation(&conn, &c).unwrap();

    assistant_storage::providers::delete_provider(&conn, "p1").unwrap();
    let fetched = conversations::get_conversation_by_id(&conn, "c1")
        .unwrap()
        .unwrap();
    assert!(
        fetched.provider_id.is_none(),
        "referential integrity: provider_id must be NULL after provider deleted"
    );
}

#[cfg(feature = "slow-tests")]
mod property {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn message_count_never_negative(n_msgs in 0usize..20) {
            let conn = init_db();
            let mut c = conv("c1");
            c.created_at = 0;
            c.updated_at = 0;
            conversations::insert_conversation(&conn, &c).unwrap();
            for i in 0..n_msgs {
                let mut m = msg(&format!("m{}", i), "c1", "user", "x", i as i64);
                conversations::insert_message(&conn, &m).unwrap();
            }
            let count = conversations::get_message_count(&conn, "c1").unwrap();
            assert_eq!(count, n_msgs as i64);
        }

        #[test]
        fn search_result_always_subset_of_all(
            title in "[a-z]{1,5}",
            content in "[a-z]{1,5}",
            query in "[a-z]{1,3}",
        ) {
            let conn = init_db();
            let mut c = conv("c1");
            c.title = title.clone();
            c.created_at = 0;
            c.updated_at = 0;
            conversations::insert_conversation(&conn, &c).unwrap();
            let m = msg("m1", "c1", "user", &content, 0);
            conversations::insert_message(&conn, &m).unwrap();

            let all = conversations::get_conversations(&conn, None).unwrap();
            let found = conversations::search_conversations(&conn, &query).unwrap();
            assert!(found.len() <= all.len());
        }
    }
}

#[test]
fn create_branch_appends_sibling_without_deleting() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "world", 200, Some("u1"), 0),
    )
    .unwrap();

    let new_msg =
        conversations::create_branch(&conn, "c1", "a1", "assistant", "regenerated").unwrap();

    assert_eq!(new_msg.role, "assistant");
    assert_eq!(new_msg.content, "regenerated");
    assert_eq!(new_msg.sibling_index, 1);
    assert_eq!(new_msg.parent_id.as_deref(), Some("u1"));

    let all = conversations::get_messages(&conn, "c1").unwrap();
    assert_eq!(all.len(), 3, "original a1 must still exist");

    let detail = conversations::get_conversation_detail(&conn, "c1")
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.messages.len(),
        2,
        "active branch should have 2 messages"
    );
    assert_eq!(
        detail.messages[1].id, new_msg.id,
        "active branch should point to new sibling"
    );
}

#[test]
fn create_branch_increments_sibling_index() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "first", 200, Some("u1"), 0),
    )
    .unwrap();

    let branch2 = conversations::create_branch(&conn, "c1", "a1", "assistant", "second").unwrap();
    assert_eq!(branch2.sibling_index, 1);

    let branch3 = conversations::create_branch(&conn, "c1", "a1", "assistant", "third").unwrap();
    assert_eq!(branch3.sibling_index, 2);
}

#[test]
fn create_branch_max_siblings_enforced() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a0", "c1", "assistant", "orig", 200, Some("u1"), 0),
    )
    .unwrap();

    for i in 1..20 {
        let result =
            conversations::create_branch(&conn, "c1", "a0", "assistant", &format!("branch{}", i));
        assert!(result.is_ok(), "branch {} should succeed", i);
    }

    let result = conversations::create_branch(&conn, "c1", "a0", "assistant", "overflow");
    assert!(result.is_err(), "21st sibling should be rejected");
    assert!(result.unwrap_err().contains("MaxSiblingsExceeded"));
}

#[test]
fn create_branch_updates_active_branch_id() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "world", 200, Some("u1"), 0),
    )
    .unwrap();

    let new_msg = conversations::create_branch(&conn, "c1", "a1", "assistant", "new").unwrap();

    let conv = conversations::get_conversation_by_id(&conn, "c1")
        .unwrap()
        .unwrap();
    assert_eq!(conv.active_branch_id.as_deref(), Some(new_msg.id.as_str()));
}

#[test]
fn switch_branch_changes_active_messages() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "first", 200, Some("u1"), 0),
    )
    .unwrap();

    let a2 = conversations::create_branch(&conn, "c1", "a1", "assistant", "second").unwrap();

    let detail_before = conversations::get_conversation_detail(&conn, "c1")
        .unwrap()
        .unwrap();
    assert_eq!(detail_before.messages[1].id, a2.id);

    conversations::switch_branch(&conn, "c1", "a1").unwrap();

    let detail_after = conversations::get_conversation_detail(&conn, "c1")
        .unwrap()
        .unwrap();
    assert_eq!(detail_after.messages[1].id, "a1");

    let conv = conversations::get_conversation_by_id(&conn, "c1")
        .unwrap()
        .unwrap();
    assert_eq!(conv.active_branch_id.as_deref(), Some("a1"));
}

#[test]
fn switch_branch_rejects_nonexistent_message() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();

    let result = conversations::switch_branch(&conn, "c1", "ghost");
    assert!(result.is_err());
}

#[test]
fn get_message_siblings_returns_ordered_list() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "first", 200, Some("u1"), 0),
    )
    .unwrap();

    let a2 = conversations::create_branch(&conn, "c1", "a1", "assistant", "second").unwrap();

    let siblings = conversations::get_message_siblings(&conn, &a2.id).unwrap();
    assert_eq!(siblings.len(), 2);
    assert_eq!(siblings[0].id, "a1");
    assert_eq!(siblings[0].sibling_index, 0);
    assert_eq!(siblings[1].id, a2.id);
    assert_eq!(siblings[1].sibling_index, 1);
}

#[test]
fn advance_active_branch_tracks_multi_iteration_chain() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "first", 200, Some("u1"), 0),
    )
    .unwrap();
    conversations::update_active_branch_id(&conn, "c1", "a1").unwrap();

    let mut tool = msg_with_parent("t1", "c1", "tool", "result", 300, Some("a1"), 0);
    tool.tool_call_id = Some("call_1".to_string());
    conversations::insert_message(&conn, &tool).unwrap();
    conversations::advance_active_branch(&conn, "c1", Some("a1"), "t1").unwrap();

    conversations::insert_message(
        &conn,
        &msg_with_parent("a2", "c1", "assistant", "second", 400, Some("t1"), 0),
    )
    .unwrap();
    conversations::advance_active_branch(&conn, "c1", Some("t1"), "a2").unwrap();

    let detail = conversations::get_conversation_detail(&conn, "c1")
        .unwrap()
        .unwrap();
    let ids: Vec<_> = detail.messages.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["u1", "a1", "t1", "a2"]);
    assert_eq!(detail.conversation.active_branch_id.as_deref(), Some("a2"));
}

#[test]
fn multi_tool_batch_chain_advances_leaf_to_terminal_row() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();

    // 与 executor 真实产生的 parent 序列一致：U1 → A1 → T1 → T2 → T3 → A2。
    // 缺陷形态（全部 tool 行挂 A1）会让叶子停在第 1 条 tool 行，这里按单链推进。
    let steps: [(&str, &str, &str); 5] = [
        ("a1", "assistant", "u1"),
        ("t1", "tool", "a1"),
        ("t2", "tool", "t1"),
        ("t3", "tool", "t2"),
        ("a2", "assistant", "t3"),
    ];
    for (index, (id, role, parent)) in steps.iter().enumerate() {
        let mut row = msg_with_parent(
            id,
            "c1",
            role,
            if *role == "tool" { "result" } else { "" },
            200 + index as i64 * 10,
            Some(parent),
            0,
        );
        if *role == "tool" {
            row.tool_call_id = Some(format!("call_{index}"));
        }
        conversations::insert_message(&conn, &row).unwrap();
        conversations::advance_active_branch(&conn, "c1", Some(parent), id).unwrap();
    }

    let detail = conversations::get_conversation_detail(&conn, "c1")
        .unwrap()
        .unwrap();
    let ids: Vec<_> = detail.messages.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["u1", "a1", "t1", "t2", "t3", "a2"]);
    assert_eq!(
        detail.conversation.active_branch_id.as_deref(),
        Some("a2"),
        "叶子必须推进到终态 assistant 行，而不是停在第 1 条 tool 行"
    );
    assert_eq!(
        conversations::get_sibling_count(&conn, "c1", Some("u1")).unwrap(),
        1,
        "tool 行链式追加，不得在 U1 下产生假分支"
    );
    assert_eq!(
        conversations::get_sibling_count(&conn, "c1", Some("a1")).unwrap(),
        1,
        "A1 下只有 1 个孩子（T1），tool 行彼此不成兄弟"
    );
}

#[test]
fn long_single_chain_detail_still_opens() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m0", "c1", "user", "q", 100)).unwrap();

    // 放宽前上限为 1000，这里构造 1200 行合法单链，验证详情不再被深度上限截断。
    let depth = 1200;
    let mut parent = "m0".to_string();
    for index in 1..=depth {
        let id = format!("m{index}");
        let row = msg_with_parent(
            &id,
            "c1",
            "assistant",
            "x",
            100 + index as i64,
            Some(&parent),
            0,
        );
        conversations::insert_message(&conn, &row).unwrap();
        parent = id;
    }
    conversations::update_active_branch_id(&conn, "c1", &parent).unwrap();

    let detail = conversations::get_conversation_detail(&conn, "c1")
        .unwrap()
        .unwrap();
    assert_eq!(detail.messages.len(), depth as usize + 1, "长链全部可见");
    assert_eq!(
        detail.conversation.active_branch_id.as_deref(),
        Some(parent.as_str())
    );
}

#[test]
fn advance_active_branch_keeps_user_branch_selection() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "picked", 200, Some("u1"), 0),
    )
    .unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a2", "c1", "assistant", "other", 300, Some("u1"), 1),
    )
    .unwrap();
    conversations::update_active_branch_id(&conn, "c1", "a1").unwrap();

    conversations::advance_active_branch(&conn, "c1", Some("u1"), "a2").unwrap();

    let detail = conversations::get_conversation_detail(&conn, "c1")
        .unwrap()
        .unwrap();
    assert_eq!(
        detail.conversation.active_branch_id.as_deref(),
        Some("a1"),
        "advancing must not steal the user's branch selection"
    );
    let ids: Vec<_> = detail.messages.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["u1", "a1"]);
}

#[test]
fn trace_active_branch_follows_parent_chain() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "world", 200, Some("u1"), 0),
    )
    .unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("u2", "c1", "user", "next", 300, Some("a1"), 0),
    )
    .unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a2", "c1", "assistant", "reply", 400, Some("u2"), 0),
    )
    .unwrap();

    let path = conversations::trace_active_branch(&conn, "c1", "a2").unwrap();
    assert_eq!(path.len(), 4);
    assert_eq!(path[0].id, "u1");
    assert_eq!(path[1].id, "a1");
    assert_eq!(path[2].id, "u2");
    assert_eq!(path[3].id, "a2");
}

#[test]
fn fork_to_new_conversation_copies_active_branch() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "world", 200, Some("u1"), 0),
    )
    .unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("u2", "c1", "user", "next", 300, Some("a1"), 0),
    )
    .unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a2", "c1", "assistant", "reply", 400, Some("u2"), 0),
    )
    .unwrap();

    let new_id = conversations::fork_to_new_conversation(&conn, "c1", "a2").unwrap();

    let new_detail = conversations::get_conversation_detail(&conn, &new_id)
        .unwrap()
        .unwrap();
    assert_eq!(new_detail.messages.len(), 4);
    assert_eq!(new_detail.messages[0].content, "hello");
    assert_eq!(new_detail.messages[3].content, "reply");
    assert_eq!(new_detail.conversation.title, "test");
    assert_eq!(
        new_detail.conversation.title_source,
        conversations::COPY_TITLE_SOURCE
    );

    let orig_detail = conversations::get_conversation_detail(&conn, "c1")
        .unwrap()
        .unwrap();
    assert_eq!(
        orig_detail.messages.len(),
        4,
        "original conversation must be untouched"
    );
}

#[test]
fn inserted_messages_default_to_completed_status() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "user", "hi", 100)).unwrap();

    assert_eq!(
        conversations::get_message_status(&conn, "m1").unwrap(),
        Some("completed".to_string())
    );
}

#[test]
fn update_message_status_roundtrips_all_values() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "assistant", "", 100)).unwrap();

    for status in ["streaming", "interrupted", "failed", "completed"] {
        conversations::update_message_status(&conn, "m1", status).unwrap();
        assert_eq!(
            conversations::get_message_status(&conn, "m1").unwrap(),
            Some(status.to_string())
        );
    }
}

#[test]
fn update_message_status_unknown_id_is_noop() {
    let conn = init_db();
    conversations::update_message_status(&conn, "missing", "failed").unwrap();
    assert_eq!(
        conversations::get_message_status(&conn, "missing").unwrap(),
        None
    );
}

#[test]
fn duplicate_and_fork_do_not_carry_streaming_status() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    let mut streaming = msg("m1", "c1", "assistant", "partial", 100);
    streaming.status = "streaming".to_string();
    conversations::insert_message(&conn, &streaming).unwrap();
    let mut interrupted = msg_with_parent("m2", "c1", "assistant", "cut", 200, Some("m1"), 0);
    interrupted.status = "interrupted".to_string();
    conversations::insert_message(&conn, &interrupted).unwrap();

    let copy_id = conversations::duplicate_conversation(&conn, "c1").unwrap();
    let copy = conversations::get_conversation_detail(&conn, &copy_id)
        .unwrap()
        .unwrap();
    let copy_statuses: Vec<&str> = copy.messages.iter().map(|m| m.status.as_str()).collect();
    assert_eq!(
        copy_statuses,
        vec!["completed", "interrupted"],
        "streaming 是运行期瞬时态，拷贝后必须落为 completed；终态原样承接"
    );

    let fork_id = conversations::fork_to_new_conversation(&conn, "c1", "m2").unwrap();
    let fork = conversations::get_conversation_detail(&conn, &fork_id)
        .unwrap()
        .unwrap();
    let fork_statuses: Vec<&str> = fork.messages.iter().map(|m| m.status.as_str()).collect();
    assert_eq!(
        fork_statuses,
        vec!["completed", "interrupted"],
        "分叉同样不得承接 streaming"
    );
}

#[test]
fn fork_preserves_parent_chain_in_new_conversation() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "world", 200, Some("u1"), 0),
    )
    .unwrap();

    let new_id = conversations::fork_to_new_conversation(&conn, "c1", "a1").unwrap();

    let new_msgs = conversations::get_messages(&conn, &new_id).unwrap();
    assert_eq!(new_msgs.len(), 2);
    let new_u1 = &new_msgs[0];
    let new_a1 = &new_msgs[1];
    assert_eq!(
        new_a1.parent_id.as_deref(),
        Some(new_u1.id.as_str()),
        "forked messages must have remapped parentId"
    );
}

#[test]
fn get_sibling_counts_batch_efficient() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "hello", 100)).unwrap();
    conversations::insert_message(
        &conn,
        &msg_with_parent("a1", "c1", "assistant", "first", 200, Some("u1"), 0),
    )
    .unwrap();
    conversations::create_branch(&conn, "c1", "a1", "assistant", "second").unwrap();

    let counts =
        conversations::get_sibling_counts_batch(&conn, "c1", &["u1".to_string(), "a1".to_string()])
            .unwrap();
    assert_eq!(
        counts.get("u1").copied(),
        Some(1),
        "u1 has no siblings at root level"
    );
    assert_eq!(
        counts.get("a1").copied(),
        Some(2),
        "a1 has 2 siblings under u1"
    );
}

#[test]
fn message_mode_id_roundtrips_through_read_paths() {
    let conn = init_db();
    let mut conversation = conv("c1");
    conversation.mode_id = Some("builtin-agent".to_string());
    conversations::insert_conversation(&conn, &conversation).unwrap();

    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    let mut assistant = msg_with_parent("a1", "c1", "assistant", "answer", 200, Some("u1"), 0);
    assistant.mode_id = Some("builtin-agent".to_string());
    conversations::insert_message(&conn, &assistant).unwrap();

    let rows = conversations::get_messages(&conn, "c1").unwrap();
    assert_eq!(rows[0].mode_id, None, "A6: user 行 mode_id 为 NULL");
    assert_eq!(
        rows[1].mode_id.as_deref(),
        Some("builtin-agent"),
        "A3: assistant 行往返保留 mode_id"
    );

    let paginated = conversations::get_messages_paginated(&conn, "c1", 10, None).unwrap();
    let paginated_assistant = paginated
        .iter()
        .find(|m| m.id == "a1")
        .expect("分页结果必须包含 assistant 行");
    assert_eq!(
        paginated_assistant.mode_id.as_deref(),
        Some("builtin-agent")
    );

    let detail = conversations::get_conversation_detail(&conn, "c1")
        .unwrap()
        .unwrap();
    let detail_assistant = detail
        .messages
        .iter()
        .find(|m| m.id == "a1")
        .expect("详情结果必须包含 assistant 行");
    assert_eq!(detail_assistant.mode_id.as_deref(), Some("builtin-agent"));
}

#[test]
fn copy_paths_keep_message_mode_id() {
    let conn = init_db();
    let mut conversation = conv("c1");
    conversation.mode_id = Some("builtin-agent".to_string());
    conversations::insert_conversation(&conn, &conversation).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    let mut assistant = msg_with_parent("a1", "c1", "assistant", "answer", 200, Some("u1"), 0);
    assistant.mode_id = Some("builtin-agent".to_string());
    conversations::insert_message(&conn, &assistant).unwrap();

    let copy_id = conversations::duplicate_conversation(&conn, "c1").unwrap();
    let copy = conversations::get_conversation_detail(&conn, &copy_id)
        .unwrap()
        .unwrap();
    let copied_assistant = copy
        .messages
        .iter()
        .find(|m| m.role == "assistant")
        .expect("副本必须包含 assistant 行");
    assert_eq!(
        copied_assistant.mode_id.as_deref(),
        Some("builtin-agent"),
        "复制会话复制源行的 mode_id，不重置"
    );
    assert!(
        copy.messages
            .iter()
            .all(|m| m.role != "user" || m.mode_id.is_none()),
        "复制后的 user 行仍为 NULL"
    );

    let branch =
        conversations::create_branch(&conn, "c1", "a1", "assistant", "alt answer").unwrap();
    assert_eq!(
        branch.mode_id.as_deref(),
        Some("builtin-agent"),
        "分支 assistant 行继承 conversations.mode_id"
    );
    let user_branch = conversations::create_branch(&conn, "c1", "u1", "user", "alt q").unwrap();
    assert_eq!(user_branch.mode_id, None, "分支 user 行写 NULL");
}

#[test]
fn duplicate_and_fork_close_in_flight_placeholders() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    assistant_with_calls(&conn, "a1", "c1", "u1", 200);
    conversations::insert_tool_placeholders(
        &conn,
        "c1",
        "a1",
        &[
            ("p1".to_string(), "call_1".to_string()),
            ("p2".to_string(), "call_2".to_string()),
        ],
        Some(7),
        2,
        200,
    )
    .unwrap();
    conversations::mark_tool_rows_running(&conn, "c1", &["call_1".to_string()]).unwrap();
    conversations::update_active_branch_id(&conn, "c1", "p2").unwrap();

    let copy_id = conversations::duplicate_conversation(&conn, "c1").unwrap();
    let copy = conversations::get_conversation_detail(&conn, &copy_id)
        .unwrap()
        .unwrap();
    let copied = |call_id: &str| {
        copy.messages
            .iter()
            .find(|m| m.tool_call_id.as_deref() == Some(call_id))
            .expect("占位行必须被复制")
    };
    assert_eq!(copied("call_1").status, "interrupted");
    assert_eq!(
        copied("call_1").content,
        conversations::INTERRUPTED_TOOL_RESULT,
        "副本没有执行队列可推进，已进队列的只能报结果未知"
    );
    assert!(copied("call_1").is_error);
    assert_eq!(copied("call_2").status, "interrupted");
    assert_eq!(
        copied("call_2").content,
        conversations::TOOL_NOT_STARTED_RESULT,
        "未进队列的副本行可断言未开始"
    );

    let fork_id = conversations::fork_to_new_conversation(&conn, "c1", "p2").unwrap();
    let fork = conversations::get_conversation_detail(&conn, &fork_id)
        .unwrap()
        .unwrap();
    assert_eq!(fork.messages.len(), 4, "分叉同样保留占位行，不丢调用");
    assert!(fork
        .messages
        .iter()
        .filter(|m| m.role == "tool")
        .all(|m| m.status == "interrupted" && !m.content.is_empty()));
}

#[test]
fn interrupted_result_text_does_not_claim_not_executed() {
    let text = conversations::INTERRUPTED_TOOL_RESULT;
    assert!(
        !text.contains("未执行"),
        "中断时无法判断工具是否已产生副作用，断言「未执行」会诱导模型重复执行非幂等操作"
    );
    assert!(text.contains("未知"), "必须明确告知执行结果未知");
}

fn assistant_with_calls(conn: &rusqlite::Connection, id: &str, conv: &str, parent: &str, ts: i64) {
    let mut row = msg_with_parent(id, conv, "assistant", "", ts, Some(parent), 0);
    row.turn_index = Some(7);
    row.iteration = 2;
    conversations::insert_message(conn, &row).unwrap();
    conversations::update_message_tool_calls(
        conn,
        id,
        r#"[{"id":"call_1","name":"fs_read","arguments":"{}"},{"id":"call_2","name":"fs_write","arguments":"{}"}]"#,
    )
    .unwrap();
}

#[test]
fn placeholders_are_declared_under_owner_in_order() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    assistant_with_calls(&conn, "a1", "c1", "u1", 200);

    conversations::insert_tool_placeholders(
        &conn,
        "c1",
        "a1",
        &[
            ("p1".to_string(), "call_1".to_string()),
            ("p2".to_string(), "call_2".to_string()),
        ],
        Some(7),
        2,
        200,
    )
    .unwrap();

    let path = conversations::trace_active_branch(&conn, "c1", "p2").unwrap();
    let ids: Vec<&str> = path.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["u1", "a1", "p1", "p2"],
        "占位行按声明顺序紧挂在 owner 之后"
    );
    for row in &path[2..] {
        assert_eq!(
            row.status,
            conversations::TOOL_ROW_PENDING,
            "声明即 pending"
        );
        assert!(row.content.is_empty(), "占位行正文为空");
        assert_eq!(row.turn_index, Some(7), "坐标与声明行一致");
        assert_eq!(row.iteration, 2, "坐标与声明行一致");
    }
    let active = conversations::get_conversation_by_id(&conn, "c1")
        .unwrap()
        .unwrap()
        .active_branch_id;
    assert_eq!(active.as_deref(), Some("p2"), "声明即把活跃叶子推到批尾");
}

#[test]
fn running_placeholder_never_rolls_back() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    assistant_with_calls(&conn, "a1", "c1", "u1", 200);
    conversations::insert_tool_placeholders(
        &conn,
        "c1",
        "a1",
        &[
            ("p1".to_string(), "call_1".to_string()),
            ("p2".to_string(), "call_2".to_string()),
        ],
        Some(7),
        2,
        200,
    )
    .unwrap();

    conversations::mark_tool_rows_running(&conn, "c1", &["call_1".to_string()]).unwrap();
    let path = conversations::trace_active_branch(&conn, "c1", "p2").unwrap();
    assert_eq!(path[2].status, conversations::TOOL_ROW_RUNNING);
    assert_eq!(
        path[3].status,
        conversations::TOOL_ROW_PENDING,
        "未执行的保持 pending"
    );

    conversations::mark_tool_rows_running(&conn, "c1", &["call_1".to_string()]).unwrap();
    let path = conversations::trace_active_branch(&conn, "c1", "p2").unwrap();
    assert_eq!(
        path[2].status,
        conversations::TOOL_ROW_RUNNING,
        "不回退到 pending"
    );
}

#[test]
fn fill_keeps_topology_and_closes_placeholder() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("u1", "c1", "user", "q", 100)).unwrap();
    assistant_with_calls(&conn, "a1", "c1", "u1", 200);
    conversations::insert_tool_placeholders(
        &conn,
        "c1",
        "a1",
        &[
            ("p1".to_string(), "call_1".to_string()),
            ("p2".to_string(), "call_2".to_string()),
        ],
        Some(7),
        2,
        200,
    )
    .unwrap();
    conversations::mark_tool_rows_running(&conn, "c1", &["call_1".to_string()]).unwrap();

    conversations::fill_tool_row(&conn, "p1", "ok", false).unwrap();

    let path = conversations::trace_active_branch(&conn, "c1", "p2").unwrap();
    let ids: Vec<&str> = path.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, vec!["u1", "a1", "p1", "p2"], "回填不插行、不改拓扑");
    assert_eq!(path[2].status, "completed");
    assert_eq!(path[2].content, "ok");
    assert_eq!(path[3].status, conversations::TOOL_ROW_PENDING);

    assert_eq!(
        conversations::find_tool_placeholder(&conn, "c1", "call_1").unwrap(),
        None,
        "已回填的调用不再是在途占位"
    );
    assert_eq!(
        conversations::find_tool_placeholder(&conn, "c1", "call_2").unwrap(),
        Some("p2".to_string())
    );
}
