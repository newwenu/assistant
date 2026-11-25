use chrono::Utc;
use tauri::{command, AppHandle, Manager};

use super::to_dto;
use crate::cmd::{get_db_conn, translate_storage_error};
use crate::protocol::events::{emit_broadcast, event_name};
use crate::protocol::{
    ChatMessagesChangedPayloadDto, ConversationListChangeKindDto, ConversationPatchDto,
    MessageChangeKindDto,
};
use assistant_storage::conversations;
use assistant_tools::storage::scratch_root_for;

use super::emit_list_changed;

/// 会话结束清理临时区（I73 §5.4 P4.6）：整棵树删除，`NotFound` 静默。
///
/// 只删「该会话命名空间」这一个子目录，不碰基根下其它会话与 `shared` 段。
/// 失败只记日志、不影响命令结果：数据库事务已提交，残留目录由存储管理页展示占用。
fn remove_session_scratch(dirs: &crate::app_dirs::AppDirs, id: &str) {
    let root = scratch_root_for(&dirs.tool_scratch(), Some(id));
    if let Err(err) = std::fs::remove_dir_all(&root) {
        if err.kind() != std::io::ErrorKind::NotFound {
            log::warn!("remove tool scratch failed at {}: {}", root.display(), err);
        }
    }
}

#[command]
pub fn delete_conversation(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    conversations::delete_conversation(&conn, &id)?;
    if let Ok(dirs) = crate::app_dirs::AppDirs::resolve() {
        remove_session_scratch(&dirs, &id);
    }

    emit_list_changed(&app, ConversationListChangeKindDto::Deleted, Some(id), None);

    Ok(())
}

/// 同步命令在主线程执行，而这里既取 DB 连接锁、又对工作区路径做 `canonicalize()`
/// 磁盘 IO，网络盘/已拔出盘上会直接把窗口卡死；整体下放到阻塞线程池（I38h §9.4）。
#[command]
pub async fn patch_conversation(
    app: AppHandle,
    id: String,
    patch: ConversationPatchDto,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || patch_conversation_blocking(&app, id, &patch))
        .await
        .map_err(|e| e.to_string())?
}

fn patch_conversation_blocking(
    app: &AppHandle,
    id: String,
    patch: &ConversationPatchDto,
) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;

    let mut changed_kinds: Vec<ConversationListChangeKindDto> = Vec::new();

    if let Some(ref title) = patch.title {
        let source = patch.title_source.as_deref().unwrap_or("user");
        conversations::rename_conversation(&conn, &id, title, source)?;
        changed_kinds.push(ConversationListChangeKindDto::Renamed);
    }

    if patch.model_id.is_some() || patch.provider_id.is_some() {
        let now = Utc::now().timestamp_millis();
        let mut conv = conversations::get_conversation_by_id(&conn, &id)?
            .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;

        if let Some(ref model_id) = patch.model_id {
            conv.model_id = model_id.clone();
            changed_kinds.push(ConversationListChangeKindDto::ModelUpdated);
        }
        if let Some(ref provider_id) = patch.provider_id {
            conv.provider_id = Some(provider_id.clone());
            changed_kinds.push(ConversationListChangeKindDto::ProviderUpdated);
        }
        conv.updated_at = now;
        conversations::update_conversation(&conn, &conv)?;
    }

    if let Some(ref mode_id) = patch.mode_id {
        let tasks = app
            .state::<std::sync::Arc<assistant_agent::kernel::TurnTasks>>()
            .inner()
            .clone();
        if tasks.is_busy(&id) {
            return Err(assistant_i18n::t("error.conversation_busy"));
        }
        let now = Utc::now().timestamp_millis();
        let mut conv = conversations::get_conversation_by_id(&conn, &id)?
            .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
        conv.mode_id = Some(mode_id.clone());
        conv.updated_at = now;
        conversations::update_conversation(&conn, &conv)?;
        changed_kinds.push(ConversationListChangeKindDto::ModeUpdated);
    }

    if let Some(ref tool_config) = patch.tool_config {
        // 规则在落库前校验（I70 §6.2 / A11）：失败即拒绝保存，不留半份写入。
        crate::adapters::validate_tool_config_rules(tool_config)?;
        let now = Utc::now().timestamp_millis();
        let mut conv = conversations::get_conversation_by_id(&conn, &id)?
            .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
        conv.tool_config_json =
            Some(serde_json::to_string(tool_config).map_err(|e| e.to_string())?);
        conv.updated_at = now;
        conversations::update_conversation(&conn, &conv)?;
        changed_kinds.push(ConversationListChangeKindDto::ToolConfigUpdated);
    }

    // 工作区三态（I38h §8.2）：缺省/`null` 不改；有值校验后写入；空串 `Err`；
    // `clearWorkspaceRoot: true` 写回空串。写入与授权清理必须同事务（§6.2）。
    let workspace_request: Option<Option<String>> = match (
        patch.clear_workspace_root.unwrap_or(false),
        patch.workspace_root.as_deref(),
    ) {
        (true, Some(_)) => return Err(assistant_i18n::t("error.workspace_patch_conflict")),
        (true, None) => Some(None),
        (false, Some(raw)) => {
            if raw.trim().is_empty() {
                return Err(assistant_i18n::t("error.workspace_empty"));
            }
            let canonical = crate::tool_setup::validate_workspace_root(raw)
                .ok_or_else(|| assistant_i18n::t("error.workspace_invalid"))?;
            Some(Some(canonical))
        }
        (false, None) => None,
    };

    if let Some(target) = workspace_request {
        let tasks = app
            .state::<std::sync::Arc<assistant_agent::kernel::TurnTasks>>()
            .inner()
            .clone();
        if tasks.is_busy(&id) {
            return Err(assistant_i18n::t("error.conversation_busy"));
        }
        conversations::get_conversation_by_id(&conn, &id)?
            .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;

        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        let value = target.as_deref().unwrap_or("");
        assistant_storage::workspaces::set_conversation_workspace(&tx, &id, value)?;
        assistant_storage::workspaces::clear_session_grants(&tx, &id)?;
        // 审计：`tool = "workspace"`，清除绑定记 `resource = "none"`（不落空串）。
        // 路径必须过 `redact_resource`：主库是明文 SQLite，工作区绝对路径不落原文，
        // 与网关决策行、授权管理行保持同一摘要口径。
        // 写失败只 `warn`：审计不得回滚工作区切换（§3.6 与 §4 的边界）。
        let resource = match target.as_deref() {
            Some(path) => assistant_tools::redact_resource(path),
            None => "none".to_string(),
        };
        if let Err(err) = tx.execute(
            "INSERT INTO permission_audit (tool, action, resource, outcome, at_millis)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "workspace",
                "switch",
                resource,
                "allowed",
                Utc::now().timestamp_millis()
            ],
        ) {
            log::warn!("workspace switch audit write failed: {err}");
        }
        tx.commit().map_err(|e| e.to_string())?;
    }

    for kind in changed_kinds {
        emit_list_changed(app, kind, Some(id.clone()), None);
    }

    Ok(())
}

#[command]
pub fn get_message_subtree_size(app: AppHandle, message_id: String) -> Result<i64, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    conversations::get_subtree_size(&conn, &message_id)
}

#[command]
pub fn delete_message_persist(app: AppHandle, message_id: String) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let conv_id = conversations::get_conversation_id_by_message(&conn, &message_id)?;
    conversations::delete_subtree(&conn, &message_id)?;

    if let Some(ref cid) = conv_id {
        if let Some(detail) = conversations::get_conversation_detail(&conn, cid)? {
            emit_broadcast(
                &app,
                event_name::CHAT_MESSAGES_CHANGED,
                ChatMessagesChangedPayloadDto {
                    conversation_id: cid.clone(),
                    change_kind: MessageChangeKindDto::MessageDeleted,
                    messages: to_dto::messages_with_sibling_counts(&conn, cid, &detail.messages)?,
                    active_branch_id: detail.conversation.active_branch_id.clone(),
                    seq: None,
                },
            );
        }
    }
    Ok(())
}

#[command]
pub fn toggle_conversation_pin(app: AppHandle, id: String) -> Result<bool, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let result = conversations::toggle_pin(&conn, &id)?;

    emit_list_changed(
        &app,
        ConversationListChangeKindDto::PinToggled,
        Some(id),
        None,
    );

    Ok(result)
}

#[command]
pub fn duplicate_conversation(app: AppHandle, id: String) -> Result<String, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let new_id =
        conversations::duplicate_conversation(&conn, &id).map_err(translate_storage_error)?;

    emit_list_changed(
        &app,
        ConversationListChangeKindDto::Duplicated,
        Some(new_id.clone()),
        None,
    );

    Ok(new_id)
}

#[command]
pub fn batch_delete_conversations(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    let affected = ids.clone();
    conversations::batch_delete(&conn, &ids)?;
    if let Ok(dirs) = crate::app_dirs::AppDirs::resolve() {
        for id in &ids {
            remove_session_scratch(&dirs, id);
        }
    }

    emit_list_changed(
        &app,
        ConversationListChangeKindDto::BatchDeleted,
        None,
        Some(affected),
    );

    Ok(())
}

#[command]
pub fn purge_conversations(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = get_db_conn(&state)?;
    // 清空会删掉全部会话，故清理临时区前先取全量 id（删表后无从枚举）。
    let ids: Vec<String> = conversations::get_conversations(&conn, None)?
        .into_iter()
        .map(|row| row.id)
        .collect();
    conversations::purge_all(&conn)?;
    if let Ok(dirs) = crate::app_dirs::AppDirs::resolve() {
        for id in &ids {
            remove_session_scratch(&dirs, id);
        }
    }

    emit_list_changed(&app, ConversationListChangeKindDto::Purged, None, None);

    Ok(())
}
