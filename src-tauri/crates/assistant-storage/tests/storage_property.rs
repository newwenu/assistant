#![cfg(feature = "slow-tests")]

use assistant_storage::conversations::{self, ConversationRow, MessageRow};
use assistant_storage::tool_favorites;
use assistant_storage::db;
use proptest::prelude::*;

fn init_db() -> rusqlite::Connection {
    let dir = tempfile::tempdir().unwrap();
    db::init_db(&dir.path().join("test.db")).unwrap()
}

proptest! {
    #[test]
    fn toggle_double_is_noop(tool_id in "[a-z]{1,10}") {
        let conn = init_db();
        let r1 = tool_favorites::toggle_favorite(&conn, &tool_id).unwrap();
        let r2 = tool_favorites::toggle_favorite(&conn, &tool_id).unwrap();
        assert_ne!(r1, r2, "consecutive toggles must return opposite values");
        assert!(tool_favorites::get_favorites(&conn).unwrap().is_empty());
    }

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