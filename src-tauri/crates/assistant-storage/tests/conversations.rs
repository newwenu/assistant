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
fn delete_message_and_after_removes_target_and_all_later() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "user", "keep", 100)).unwrap();
    conversations::insert_message(&conn, &msg("m2", "c1", "assistant", "target", 200)).unwrap();
    conversations::insert_message(&conn, &msg("m3", "c1", "user", "later", 300)).unwrap();

    conversations::delete_message_and_after(&conn, "m2").unwrap();
    let remaining = conversations::get_messages(&conn, "c1").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, "m1");
}

#[test]
fn delete_message_and_after_preserves_same_timestamp_siblings() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "user", "keep", 100)).unwrap();
    conversations::insert_message(&conn, &msg("m2", "c1", "assistant", "target", 200)).unwrap();
    conversations::insert_message(&conn, &msg("m3", "c1", "user", "same-ts-sibling", 200)).unwrap();
    conversations::insert_message(&conn, &msg("m4", "c1", "assistant", "later", 300)).unwrap();

    conversations::delete_message_and_after(&conn, "m2").unwrap();
    let remaining = conversations::get_messages(&conn, "c1").unwrap();
    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0].id, "m1");
    assert_eq!(
        remaining[1].id, "m3",
        "same-timestamp sibling should survive"
    );
}

#[test]
fn edit_message_removes_later_and_moves_timestamp() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "user", "original", 100)).unwrap();
    conversations::insert_message(&conn, &msg("m2", "c1", "assistant", "response", 200)).unwrap();
    conversations::insert_message(&conn, &msg("m3", "c1", "user", "followup", 300)).unwrap();

    conversations::edit_message(&conn, "m1", "edited").unwrap();
    let remaining = conversations::get_messages(&conn, "c1").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].content, "edited");
    assert!(
        remaining[0].timestamp > 100,
        "edited message should get a new timestamp"
    );
}

#[test]
fn edit_message_removes_same_timestamp_siblings() {
    let conn = init_db();
    conversations::insert_conversation(&conn, &conv("c1")).unwrap();
    conversations::insert_message(&conn, &msg("m1", "c1", "user", "edit-me", 200)).unwrap();
    conversations::insert_message(&conn, &msg("m2", "c1", "assistant", "same-ts-sibling", 200))
        .unwrap();
    conversations::insert_message(&conn, &msg("m3", "c1", "user", "later", 300)).unwrap();

    conversations::edit_message(&conn, "m1", "edited").unwrap();
    let remaining = conversations::get_messages(&conn, "c1").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].content, "edited");
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
            }).unwrap();

            let all = conversations::get_conversations(&conn, None).unwrap();
            let found = conversations::search_conversations(&conn, &query).unwrap();
            assert!(found.len() <= all.len());
        }
    }
}
