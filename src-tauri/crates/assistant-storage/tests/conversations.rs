use assistant_storage::conversations::{self, ConversationRow, MessageRow};
use assistant_storage::db;

fn init_db() -> rusqlite::Connection {
    let dir = tempfile::tempdir().unwrap();
    db::init_db(&dir.path().join("test.db")).unwrap()
}

fn conv(id: &str) -> ConversationRow {
    ConversationRow {
        id: id.to_string(),
        title: "test".to_string(),
        title_source: "placeholder".to_string(),
        provider_id: None,
        model_id: "gpt-4".to_string(),
        system_prompt: None,
        is_pinned: false,
        sort_order: 0,
        generation: 0,
        created_at: 1000,
        updated_at: 1000,
        is_full: false,
        active_branch_id: None,
    }
}

fn msg(id: &str, conv_id: &str, role: &str, content: &str, ts: i64) -> MessageRow {
    MessageRow {
        id: id.to_string(),
        conversation_id: conv_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        tokens: None,
        timestamp: ts,
        parent_id: None,
        is_pinned: false,
        reasoning: None,
        tool_calls_json: None,
        sibling_index: 0,
    }
}

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
    assert!(detail.conversation.title.contains("副本"));
}

#[test]
fn pagination_covers_all_messages_without_overlap_or_gap() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    for i in 0..5 {
        conversations::insert_message(
            &conn,
            &msg(
                &format!("m{}", i),
                "c1",
                "user",
                &format!("msg{}", i),
                (i as i64) * 100,
            ),
        )
        .unwrap();
    }

    let page1 = conversations::get_messages_paginated(&conn, "c1", 3, None).unwrap();
    assert_eq!(page1.len(), 3);

    let oldest_ts = page1.first().unwrap().timestamp;
    let page2 = conversations::get_messages_paginated(&conn, "c1", 3, Some(oldest_ts)).unwrap();
    assert_eq!(
        page2.len(),
        2,
        "remaining 2 messages before oldest of page1"
    );

    let all_ids: Vec<&str> = page1
        .iter()
        .chain(page2.iter())
        .map(|m| m.id.as_str())
        .collect();
    let unique: std::collections::HashSet<&str> = all_ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        5,
        "pages should cover all messages without overlap"
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
    assistant_storage::providers::insert_provider(
        &conn,
        &assistant_storage::providers::ProviderRow {
            id: "p1".to_string(),
            name: "test".to_string(),
            type_: "openai".to_string(),
            base_url: "http://x".to_string(),
            api_key_encrypted: String::new(),
            models_json: "[]".to_string(),
            default_model: "gpt-4".to_string(),
            updated_at: 0,
        },
    )
    .unwrap();
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
            let conv = ConversationRow {
                id: "c1".to_string(),
                title: "t".to_string(),
                title_source: "placeholder".to_string(),
                provider_id: None,
                model_id: "m".to_string(),
                system_prompt: None,
                is_pinned: false,
                sort_order: 0,
                generation: 0,
                created_at: 0,
                updated_at: 0,
                is_full: false,
                active_branch_id: None,
            };
            conversations::insert_conversation(&conn, &conv).unwrap();
            for i in 0..n_msgs {
                conversations::insert_message(&conn, &MessageRow {
                    id: format!("m{}", i),
                    conversation_id: "c1".to_string(),
                    role: "user".to_string(),
                    content: "x".to_string(),
                    tokens: None,
                    timestamp: i as i64,
                    parent_id: None,
                    is_pinned: false,
                    reasoning: None,
                    tool_calls_json: None,
                    sibling_index: 0,
                }).unwrap();
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
            let conv = ConversationRow {
                id: "c1".to_string(),
                title: title.clone(),
                title_source: "placeholder".to_string(),
                provider_id: None,
                model_id: "m".to_string(),
                system_prompt: None,
                is_pinned: false,
                sort_order: 0,
                generation: 0,
                created_at: 0,
                updated_at: 0,
                is_full: false,
                active_branch_id: None,
            };
            conversations::insert_conversation(&conn, &conv).unwrap();
            conversations::insert_message(&conn, &MessageRow {
                id: "m1".to_string(),
                conversation_id: "c1".to_string(),
                role: "user".to_string(),
                content: content.clone(),
                tokens: None,
                timestamp: 0,
                parent_id: None,
                is_pinned: false,
                reasoning: None,
                tool_calls_json: None,
                sibling_index: 0,
            }).unwrap();

            let all = conversations::get_conversations(&conn, None).unwrap();
            let found = conversations::search_conversations(&conn, &query).unwrap();
            assert!(found.len() <= all.len());
        }
    }
}

fn msg_with_parent(
    id: &str,
    conv_id: &str,
    role: &str,
    content: &str,
    ts: i64,
    parent_id: Option<&str>,
    sibling_index: i64,
) -> MessageRow {
    MessageRow {
        id: id.to_string(),
        conversation_id: conv_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        tokens: None,
        timestamp: ts,
        parent_id: parent_id.map(|s| s.to_string()),
        is_pinned: false,
        reasoning: None,
        tool_calls_json: None,
        sibling_index,
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
