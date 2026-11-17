use chrono::Utc;
use uuid::Uuid;

use assistant_storage::{
    attachment_store::{self},
    conversations::{self, MessageRow},
    providers::{self as provider_store},
};

use crate::protocol::{CommandErrorCodeDto, CommandErrorDto};

pub(crate) struct InsertedMessages {
    pub assistant_msg_id: String,
    pub user_msg_id: Option<String>,
    /// 本轮 turn 序号（I46 §2.4）。`None` = 分支点位于存量行，该轮无坐标。
    pub turn_index: Option<u32>,
    /// assistant 占位行的迭代号（I46 §2.6）：分支重生成时为 `> 1`，事件必须如实下发。
    pub iteration: u32,
}

pub(crate) struct InsertChatInput<'a> {
    pub conversation_id: &'a str,
    pub content: &'a str,
    pub branch_point_message_id: Option<&'a str>,
    pub provider_id: &'a str,
    pub model_id: &'a str,
    /// 本次发送所选模式（I43 §3.4）；空串视为未选，回退 `conversations.mode_id`。
    pub mode_id: Option<&'a str>,
}

/// 写入 `assistant` 占位行的身份字段（I43 §3.4）：仅 assistant 行携带，`user` 行恒为 `NULL`。
struct MessageIdentity<'a> {
    provider_id: Option<&'a str>,
    model_id: Option<&'a str>,
    mode_id: Option<&'a str>,
}

/// 本轮坐标（I46 §2.4 / §2.6）：`turn_index` 与 `iteration` 恒成对出现，
/// 收成一个入参，写行函数签名不随坐标字段增长。
struct TurnCoords {
    turn_index: Option<u32>,
    iteration: u32,
}

pub(crate) fn insert_chat_messages(
    conn: &rusqlite::Connection,
    input: InsertChatInput<'_>,
) -> Result<InsertedMessages, CommandErrorDto> {
    let InsertChatInput {
        conversation_id,
        content,
        branch_point_message_id,
        provider_id,
        model_id,
        mode_id,
    } = input;

    if conversations::is_conversation_full(conn, conversation_id)
        .map_err(super::super::command_error_from_storage)?
    {
        return Err(CommandErrorDto::new(
            CommandErrorCodeDto::Validation,
            assistant_i18n::t("chat.conversation_full"),
        ));
    }

    let now = Utc::now().timestamp_millis();

    let detail_before = conversations::get_conversation_detail(conn, conversation_id)
        .map_err(CommandErrorDto::internal)?
        .ok_or_else(|| {
            CommandErrorDto::new(
                CommandErrorCodeDto::ConversationNotFound,
                assistant_i18n::t("error.conversation_not_found"),
            )
        })?;

    provider_store::get_provider_by_id(conn, provider_id)
        .map_err(CommandErrorDto::internal)?
        .ok_or_else(|| {
            CommandErrorDto::new(
                CommandErrorCodeDto::ProviderNotFound,
                assistant_i18n::t("error.provider_not_found"),
            )
        })?;

    // 取号、写行、递增计数器必须同事务（I46 §2.4）：任一失败整体回滚，
    // 计数器不会因写行失败而空转。
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| CommandErrorDto::internal(e.to_string()))?;

    {
        let mut conv = conversations::get_conversation_by_id(&tx, conversation_id)
            .map_err(CommandErrorDto::internal)?
            .ok_or_else(|| {
                CommandErrorDto::new(
                    CommandErrorCodeDto::ConversationNotFound,
                    assistant_i18n::t("error.conversation_not_found"),
                )
            })?;
        conv.provider_id = Some(provider_id.to_string());
        conv.model_id = model_id.to_string();
        conv.updated_at = now;
        conversations::update_conversation(&tx, &conv).map_err(CommandErrorDto::internal)?;
    }

    // 模式来源统一：本次所选（非空串）优先，否则回退会话现值（I43 §3.4）。
    let msg_mode_id: Option<String> = match mode_id {
        Some(raw) if !raw.is_empty() => Some(raw.to_string()),
        _ => detail_before.conversation.mode_id.clone(),
    };

    let identity = MessageIdentity {
        provider_id: Some(provider_id),
        model_id: Some(model_id),
        mode_id: msg_mode_id.as_deref(),
    };

    let (assistant_msg_id, user_msg_id, turn_index, iteration) =
        if let Some(bp_id) = branch_point_message_id {
            insert_on_branch(&tx, conversation_id, content, bp_id, now, &identity)?
        } else {
            insert_normal_append(
                &tx,
                conversation_id,
                content,
                &detail_before,
                now,
                &identity,
            )?
        };

    conversations::update_active_branch_id(&tx, conversation_id, &assistant_msg_id)
        .map_err(CommandErrorDto::internal)?;

    tx.commit()
        .map_err(|e| CommandErrorDto::internal(e.to_string()))?;

    Ok(InsertedMessages {
        assistant_msg_id,
        user_msg_id,
        turn_index,
        iteration,
    })
}

/// 分支发送的坐标归属（I46 §2.4 路径 2 / 3）：
///
/// - 分支点是 `user`：等于「在该 user 之后新开一轮」，取新号，`(t, 0)` + `(t, 1)`；
/// - 分支点是 `assistant`：属于同一轮的重新生成，沿用分支点的 `turn_index`，
///   迭代号在该轮内递增，**不递增会话计数器**（ADR D2 约束 4 例外）；
///   分支点无坐标（存量行）时整轮无坐标，落 `unresolved`。
fn insert_on_branch(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    content: &str,
    bp_id: &str,
    now: i64,
    identity: &MessageIdentity<'_>,
) -> Result<(String, Option<String>, Option<u32>, u32), CommandErrorDto> {
    let branch_point = conversations::get_message_by_id(conn, bp_id)
        .map_err(CommandErrorDto::internal)?
        .ok_or_else(|| {
            CommandErrorDto::new(
                CommandErrorCodeDto::ConversationNotFound,
                format!("Branch point message {} not found", bp_id),
            )
        })?;

    let bp_parent_id = branch_point.parent_id.clone();

    if branch_point.role == "user" {
        let user_parent_id = bp_parent_id.clone();
        let user_sibling_index =
            conversations::get_sibling_count(conn, conversation_id, user_parent_id.as_deref())
                .map_err(CommandErrorDto::internal)?
                .max(0);

        let turn_index = conversations::allocate_turn_index(conn, conversation_id)
            .map_err(CommandErrorDto::internal)?;

        let uid = insert_user_message_with_attachments(
            conn,
            conversation_id,
            content,
            user_parent_id,
            user_sibling_index,
            now,
            TurnCoords {
                turn_index: Some(turn_index),
                iteration: 0,
            },
        )?;

        let aid = insert_placeholder_assistant_message(
            conn,
            conversation_id,
            Some(uid.clone()),
            0,
            now + 1,
            identity,
            TurnCoords {
                turn_index: Some(turn_index),
                iteration: 1,
            },
        )?;

        Ok((aid, Some(uid), Some(turn_index), 1))
    } else {
        let assistant_parent_id = bp_parent_id.clone();
        let assistant_sibling_index =
            conversations::get_sibling_count(conn, conversation_id, assistant_parent_id.as_deref())
                .map_err(CommandErrorDto::internal)?
                .max(0);

        let (turn_index, iteration) = match branch_point.turn_index {
            Some(raw) => {
                let t = u32::try_from(raw)
                    .map_err(|_| CommandErrorDto::internal(format!("invalid turn_index {raw}")))?;
                let it = conversations::allocate_iteration(conn, conversation_id, t)
                    .map_err(CommandErrorDto::internal)?;
                (Some(t), it)
            }
            None => (None, 0),
        };

        let aid = insert_placeholder_assistant_message(
            conn,
            conversation_id,
            assistant_parent_id,
            assistant_sibling_index,
            now,
            identity,
            TurnCoords {
                turn_index,
                iteration,
            },
        )?;

        Ok((aid, None, turn_index, iteration))
    }
}

fn insert_normal_append(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    content: &str,
    detail_before: &conversations::ConversationDetail,
    now: i64,
    identity: &MessageIdentity<'_>,
) -> Result<(String, Option<String>, Option<u32>, u32), CommandErrorDto> {
    let user_parent_id = detail_before.messages.last().map(|m| m.id.clone());
    let user_sibling_index =
        conversations::get_sibling_count(conn, conversation_id, user_parent_id.as_deref())
            .map_err(CommandErrorDto::internal)?
            .max(0);

    let turn_index = conversations::allocate_turn_index(conn, conversation_id)
        .map_err(CommandErrorDto::internal)?;

    let uid = insert_user_message_with_attachments(
        conn,
        conversation_id,
        content,
        user_parent_id,
        user_sibling_index,
        now,
        TurnCoords {
            turn_index: Some(turn_index),
            iteration: 0,
        },
    )?;

    let aid = insert_placeholder_assistant_message(
        conn,
        conversation_id,
        Some(uid.clone()),
        0,
        now + 1,
        identity,
        TurnCoords {
            turn_index: Some(turn_index),
            iteration: 1,
        },
    )?;

    Ok((aid, Some(uid), Some(turn_index), 1))
}

fn insert_user_message_with_attachments(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    content: &str,
    parent_id: Option<String>,
    sibling_index: i64,
    now: i64,
    coords: TurnCoords,
) -> Result<String, CommandErrorDto> {
    let uid = Uuid::new_v4().to_string();
    conversations::insert_message(
        conn,
        &MessageRow {
            id: uid.clone(),
            conversation_id: conversation_id.to_string(),
            role: "user".to_string(),
            content: content.to_string(),
            tokens: None,
            timestamp: now,
            parent_id,
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
            turn_index: coords.turn_index.map(i64::from),
            iteration: i64::from(coords.iteration),
        },
    )
    .map_err(CommandErrorDto::internal)?;

    {
        let attachment_ids = extract_attachment_ids(content);
        if !attachment_ids.is_empty() {
            attachment_store::insert_attachment_refs(conn, &attachment_ids, conversation_id, &uid)
                .map_err(CommandErrorDto::internal)?;
        }
    }

    Ok(uid)
}

fn insert_placeholder_assistant_message(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    parent_id: Option<String>,
    sibling_index: i64,
    timestamp: i64,
    identity: &MessageIdentity<'_>,
    coords: TurnCoords,
) -> Result<String, CommandErrorDto> {
    let aid = Uuid::new_v4().to_string();
    conversations::insert_message(
        conn,
        &MessageRow {
            id: aid.clone(),
            conversation_id: conversation_id.to_string(),
            role: "assistant".to_string(),
            content: String::new(),
            tokens: None,
            timestamp,
            parent_id,
            is_pinned: false,
            reasoning: None,
            tool_calls_json: None,
            sibling_index,
            provider_id: identity.provider_id.map(|s| s.to_string()),
            model_id: identity.model_id.map(|s| s.to_string()),
            tool_call_id: None,
            is_error: false,
            status: "streaming".to_string(),
            mode_id: identity.mode_id.map(|s| s.to_string()),
            turn_index: coords.turn_index.map(i64::from),
            iteration: i64::from(coords.iteration),
        },
    )
    .map_err(CommandErrorDto::internal)?;
    conversations::update_message_status(conn, &aid, "streaming")
        .map_err(CommandErrorDto::internal)?;
    Ok(aid)
}

pub(crate) fn is_content_empty(content: &str) -> bool {
    assistant_storage::content_codec::is_content_empty(content)
}

pub(crate) fn extract_attachment_ids(content: &str) -> Vec<String> {
    let Some(blocks) = assistant_providers::types::parse_content(content) else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter_map(|block| match block {
            assistant_providers::types::ContentBlock::Image { attachment, .. } => {
                if attachment.attachment_id.is_empty() {
                    None
                } else {
                    Some(attachment.attachment_id.clone())
                }
            }
            assistant_providers::types::ContentBlock::File { attachment, .. } => {
                if attachment.attachment_id.is_empty() {
                    None
                } else {
                    Some(attachment.attachment_id.clone())
                }
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_storage::conversations::ConversationRow;
    use assistant_storage::providers::ProviderRow;

    fn setup(conversation_mode: Option<&str>) -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = assistant_storage::db::init_db(&dir.path().join("chat.db")).unwrap();
        provider_store::insert_provider(
            &conn,
            &ProviderRow {
                id: "p1".to_string(),
                name: "p1".to_string(),
                type_: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key_encrypted: String::new(),
                models_json: "[]".to_string(),
                default_model: "gpt-4".to_string(),
                updated_at: 1000,
            },
        )
        .unwrap();
        conversations::insert_conversation(
            &conn,
            &ConversationRow {
                id: "c1".to_string(),
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
                mode_id: conversation_mode.map(str::to_string),
                tool_config_json: None,
                workspace_root: String::new(),
            },
        )
        .unwrap();
        (dir, conn)
    }

    fn send(
        conn: &rusqlite::Connection,
        mode_id: Option<&str>,
        bp: Option<&str>,
    ) -> InsertedMessages {
        insert_chat_messages(
            conn,
            InsertChatInput {
                conversation_id: "c1",
                content: "hello",
                branch_point_message_id: bp,
                provider_id: "p1",
                model_id: "gpt-4",
                mode_id,
            },
        )
        .unwrap()
    }

    fn mode_of(conn: &rusqlite::Connection, id: &str) -> Option<String> {
        conversations::get_message_by_id(conn, id)
            .unwrap()
            .expect("message exists")
            .mode_id
    }

    #[test]
    fn chat_insert_writes_mode_only_to_assistant_row() {
        let (_dir, conn) = setup(None);
        let inserted = send(&conn, Some("builtin-agent"), None);

        assert_eq!(
            mode_of(&conn, &inserted.assistant_msg_id).as_deref(),
            Some("builtin-agent"),
            "A6: assistant 占位行写本次发送模式"
        );
        let user_id = inserted.user_msg_id.expect("普通追加必须建 user 行");
        assert_eq!(
            mode_of(&conn, &user_id),
            None,
            "A6: user 行 mode_id 为 NULL"
        );
    }

    #[test]
    fn empty_mode_falls_back_to_conversation_mode() {
        let (_dir, conn) = setup(Some("builtin-translate"));
        let inserted = send(&conn, Some(""), None);

        assert_eq!(
            mode_of(&conn, &inserted.assistant_msg_id).as_deref(),
            Some("builtin-translate"),
            "空串视为未选，回退 conversations.mode_id"
        );
    }

    #[test]
    fn missing_mode_falls_back_to_conversation_mode() {
        let (_dir, conn) = setup(Some("builtin-agent"));
        let inserted = send(&conn, None, None);

        assert_eq!(
            mode_of(&conn, &inserted.assistant_msg_id).as_deref(),
            Some("builtin-agent"),
            "缺省同样回退 conversations.mode_id"
        );
    }

    #[test]
    fn branch_insert_on_assistant_point_keeps_mode() {
        let (_dir, conn) = setup(Some("builtin-agent"));
        let first = send(&conn, Some("builtin-agent"), None);

        let second = send(
            &conn,
            Some("builtin-translate"),
            Some(&first.assistant_msg_id),
        );

        assert_eq!(
            mode_of(&conn, &second.assistant_msg_id).as_deref(),
            Some("builtin-translate"),
            "分支插入的 assistant 行取本次发送模式"
        );
        assert!(
            second.user_msg_id.is_none(),
            "以 assistant 为分支点时不新建 user 行"
        );
    }

    fn coords(conn: &rusqlite::Connection, id: &str) -> (Option<i64>, i64) {
        let row = conversations::get_message_by_id(conn, id)
            .unwrap()
            .expect("message exists");
        (row.turn_index, row.iteration)
    }

    fn next_turn_index(conn: &rusqlite::Connection) -> i64 {
        conn.query_row(
            "SELECT next_turn_index FROM conversations WHERE id = 'c1'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn write_failure_rolls_back_counter_and_rows() {
        let (_dir, conn) = setup(None);
        conn.execute_batch(
            "CREATE TRIGGER fail_message_insert BEFORE INSERT ON messages \
             WHEN NEW.content = 'boom' BEGIN SELECT RAISE(ABORT, 'boom'); END;",
        )
        .unwrap();

        let before = next_turn_index(&conn);
        let result = insert_chat_messages(
            &conn,
            InsertChatInput {
                conversation_id: "c1",
                content: "boom",
                branch_point_message_id: None,
                provider_id: "p1",
                model_id: "gpt-4",
                mode_id: None,
            },
        );
        assert!(result.is_err(), "注入写入失败必须整体报错");

        assert_eq!(
            next_turn_index(&conn),
            before,
            "门禁 19：写行失败时计数器不递增（取号与写行同事务）"
        );
        assert_eq!(
            conversations::get_messages(&conn, "c1").unwrap().len(),
            0,
            "回滚后不留半成品行"
        );

        conn.execute_batch("DROP TRIGGER fail_message_insert;")
            .unwrap();
        let ok = send(&conn, None, None);
        assert_eq!(ok.turn_index, Some(1), "失败轮不占用号，下一轮仍从 1 起");
    }

    #[test]
    fn normal_append_allocates_monotonic_turn_coords() {
        let (_dir, conn) = setup(None);
        let first = send(&conn, None, None);
        assert_eq!(first.turn_index, Some(1), "首轮从 1 起");

        let uid = first.user_msg_id.clone().expect("user 行");
        assert_eq!(coords(&conn, &uid), (Some(1), 0), "user 行 (t, 0)");
        assert_eq!(
            coords(&conn, &first.assistant_msg_id),
            (Some(1), 1),
            "assistant 行 (t, 1)"
        );

        let second = send(&conn, None, None);
        assert_eq!(second.turn_index, Some(2), "下一轮递增");
        let uid2 = second.user_msg_id.clone().expect("user 行");
        assert_eq!(coords(&conn, &uid2), (Some(2), 0));
        assert_eq!(coords(&conn, &second.assistant_msg_id), (Some(2), 1));
    }

    #[test]
    fn branch_on_user_point_opens_new_turn() {
        let (_dir, conn) = setup(None);
        let first = send(&conn, None, None);
        let uid = first.user_msg_id.clone().expect("user 行");

        let branched = send(&conn, None, Some(&uid));
        assert_eq!(
            branched.turn_index,
            Some(2),
            "以 user 为分支点 = 在其后新开一轮"
        );
        let new_uid = branched.user_msg_id.clone().expect("user 行");
        assert_eq!(coords(&conn, &new_uid), (Some(2), 0));
        assert_eq!(coords(&conn, &branched.assistant_msg_id), (Some(2), 1));
    }

    #[test]
    fn branch_on_assistant_point_reuses_turn_and_bumps_iteration() {
        let (_dir, conn) = setup(None);
        let first = send(&conn, None, None);
        assert_eq!(coords(&conn, &first.assistant_msg_id), (Some(1), 1));

        let regenerated = send(&conn, None, Some(&first.assistant_msg_id));
        assert_eq!(
            regenerated.turn_index,
            Some(1),
            "重生成沿用分支点 turn，不新开轮"
        );
        assert_eq!(
            coords(&conn, &regenerated.assistant_msg_id),
            (Some(1), 2),
            "迭代号在该 turn 内递增"
        );

        let again = send(&conn, None, Some(&regenerated.assistant_msg_id));
        assert_eq!(coords(&conn, &again.assistant_msg_id), (Some(1), 3));
    }

    #[test]
    fn new_rows_never_carry_null_turn_index() {
        let (_dir, conn) = setup(None);
        let first = send(&conn, None, None);
        let second = send(&conn, None, Some(&first.assistant_msg_id));

        let rows = conversations::get_messages(&conn, "c1").unwrap();
        assert_eq!(rows.len(), 3, "两轮共 3 行（第二轮为分支重生成）");
        for row in &rows {
            assert!(
                row.turn_index.is_some(),
                "门禁 18：chat 写路径不得产生 NULL 坐标行，违规行 {}",
                row.id
            );
        }
        assert_eq!(second.turn_index, Some(1));
    }
}
