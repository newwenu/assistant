#![allow(dead_code)]

use assistant_storage::conversations::{ConversationRow, MessageRow};
use assistant_storage::db;
use assistant_storage::providers::ProviderRow;
use rusqlite::Connection;
use std::path::Path;

/// 测试数据库句柄：`TempDir` 必须与连接同生命周期。
///
/// 字段按声明顺序析构，`conn` 在前保证先关闭 SQLite 连接，`_dir` 之后才能删除目录；
/// 否则 Windows 上打开中的 `test.db` 会让 `remove_dir_all` 静默失败并泄漏到 `%TEMP%`。
pub struct TestDb {
    conn: Connection,
    _dir: tempfile::TempDir,
}

impl std::ops::Deref for TestDb {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        &self.conn
    }
}

impl std::ops::DerefMut for TestDb {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

pub fn init_db() -> TestDb {
    let dir = tempfile::tempdir().unwrap();
    let conn = db::init_db(&dir.path().join("test.db")).unwrap();
    TestDb { conn, _dir: dir }
}

pub fn init_db_at(path: &Path) -> Connection {
    db::init_db(path).unwrap()
}

pub fn conv(id: &str) -> ConversationRow {
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
        mode_id: None,
        tool_config_json: None,
        workspace_root: String::new(),
    }
}

pub fn msg(id: &str, conv_id: &str, role: &str, content: &str, ts: i64) -> MessageRow {
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
        provider_id: None,
        model_id: None,
        tool_call_id: None,
        is_error: false,
        status: "completed".to_string(),
        mode_id: None,
        turn_index: None,
        iteration: 0,
    }
}

pub fn msg_with_parent(
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
        provider_id: None,
        model_id: None,
        tool_call_id: None,
        is_error: false,
        status: "completed".to_string(),
        mode_id: None,
        turn_index: None,
        iteration: 0,
    }
}

pub fn provider(id: &str, ts: i64) -> ProviderRow {
    ProviderRow {
        id: id.to_string(),
        name: format!("provider-{}", id),
        type_: "openai".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        api_key_encrypted: String::new(),
        models_json: "[]".to_string(),
        default_model: "gpt-4".to_string(),
        updated_at: ts,
    }
}
