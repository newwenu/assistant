use tauri::{command, AppHandle, Manager};

use crate::cmd::{get_db_conn, translate_storage_error};
use crate::protocol::MessageRecordDto;
use assistant_storage::conversations;

/// JSONL 组装落在 app 层（I51 §5.4）：`assistant-storage` 是下层 crate，看不到
/// `MessageRecordDto`，在 `ops.rs` 里另造镜像结构会与 DTO 漂移。
/// 首行是 schema 元信息（DTO 本身不含版本字段），其后每行一条落库原文，可直接 `jq` 过滤。
fn export_conversation_jsonl(conn: &rusqlite::Connection, id: &str) -> Result<String, String> {
    let detail = conversations::get_conversation_detail(conn, id)?
        .ok_or_else(|| assistant_i18n::t("conversation.not_found"))?;
    let conversation_id = detail.conversation.id.clone();
    let message_count = detail.messages.len();

    let mut out = String::new();
    let header = serde_json::json!({
        "schema": conversations::CONVERSATION_RAW_SCHEMA,
        "conversationId": conversation_id,
        "messageCount": message_count,
    });
    out.push_str(&serde_json::to_string(&header).map_err(|e| e.to_string())?);
    out.push('\n');
    for row in detail.messages {
        let line =
            serde_json::to_string(&MessageRecordDto::from(row)).map_err(|e| e.to_string())?;
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

#[command]
pub fn export_conversation(
    app: AppHandle,
    id: String,
    format: String,
    path: Option<String>,
) -> Result<Option<String>, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;
    let content = match format.as_str() {
        "json" => {
            conversations::export_conversation_json(&conn, &id).map_err(translate_storage_error)?
        }
        "jsonl" => export_conversation_jsonl(&conn, &id)?,
        "markdown" | "md" => conversations::export_conversation_markdown(&conn, &id)
            .map_err(translate_storage_error)?,
        _ => conversations::export_conversation_markdown(&conn, &id)
            .map_err(translate_storage_error)?,
    };
    match path {
        Some(p) => {
            std::fs::write(&p, &content).map_err(|e| e.to_string())?;
            Ok(None)
        }
        None => Ok(Some(content)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_storage::conversations::{ConversationRow, MessageRow};
    use assistant_storage::db;

    fn conv_row(id: &str) -> ConversationRow {
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

    fn msg_row(id: &str, conv_id: &str, role: &str, content: &str, ts: i64) -> MessageRow {
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

    #[test]
    fn jsonl_header_then_one_self_contained_line_per_message() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::init_db(&dir.path().join("t.db")).unwrap();
        conversations::insert_conversation(&conn, &conv_row("c1")).unwrap();
        conversations::insert_message(&conn, &msg_row("m1", "c1", "user", "hi\n世界", 100))
            .unwrap();
        conversations::insert_message(&conn, &msg_row("m2", "c1", "assistant", "ok", 200)).unwrap();

        let out = export_conversation_jsonl(&conn, "c1").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3, "schema header + one line per message");

        let header: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(header["schema"], conversations::CONVERSATION_RAW_SCHEMA);
        assert_eq!(header["conversationId"], "c1");
        assert_eq!(header["messageCount"], 2);

        for line in &lines[1..] {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(
                row["conversationId"], "c1",
                "every line must be jq-filterable"
            );
            assert!(
                row.get("toolCallsJson").is_some(),
                "raw json fields keep their keys (null when absent)"
            );
        }
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(lines[1]).unwrap()["content"],
            "hi\n世界",
            "newlines must be escaped so one message stays one line"
        );
    }
}
