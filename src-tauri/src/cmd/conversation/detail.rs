use tauri::{command, AppHandle, Manager};

use super::to_dto;
use crate::cmd::get_db_conn;
use crate::protocol::{
    ConversationDetailDto, ConversationQueryOptsDto, TokenUsageEstimatedSourceDto,
    TokenUsageSummaryDto,
};
use assistant_agent::kernel::request;
use assistant_storage::{
    attachment_store::AttachmentStore, conversations, providers as provider_store,
};

use super::detail_dto::build_detail_dto;

#[command]
pub fn get_conversation(
    app: AppHandle,
    id: String,
    opts: Option<ConversationQueryOptsDto>,
) -> Result<Option<ConversationDetailDto>, String> {
    let state = app.state::<crate::db_state::DbState>();
    let conn = get_db_conn(&state)?;

    let mode = opts.as_ref().map(|o| o.mode.as_str()).unwrap_or("detail");
    let include_records = opts.as_ref().map(|o| o.include_records).unwrap_or(false);

    match mode {
        "all" => get_conversation_all_impl(&conn, &id, include_records),
        "paginated" => {
            let o = opts.as_ref();
            let limit = o.map(|o| o.limit).unwrap_or(50);
            let before_turn = o.and_then(|o| o.before_turn_index);
            get_conversation_paginated_impl(&conn, &id, limit, before_turn, include_records)
        }
        _ => get_conversation_detail_impl(&conn, &id, include_records),
    }
}

fn get_conversation_detail_impl(
    conn: &rusqlite::Connection,
    id: &str,
    include_records: bool,
) -> Result<Option<ConversationDetailDto>, String> {
    let detail = conversations::get_conversation_detail(conn, id)?;
    match detail {
        Some(d) => {
            let total_count = d.messages.len() as i64;
            Ok(Some(build_detail_dto(
                conn,
                id,
                d.conversation,
                d.messages,
                total_count,
                include_records,
            )?))
        }
        None => Ok(None),
    }
}

fn get_conversation_paginated_impl(
    conn: &rusqlite::Connection,
    id: &str,
    limit: i64,
    before_turn_index: Option<u32>,
    include_records: bool,
) -> Result<Option<ConversationDetailDto>, String> {
    match conversations::get_conversation_by_id(conn, id)? {
        Some(conv) => {
            let (messages, total_count) = if let Some(ref leaf_id) = conv.active_branch_id {
                let branch = conversations::trace_active_branch(conn, id, leaf_id)?;
                let count = conversations::get_message_count(conn, id)?;
                (branch, count)
            } else {
                let msgs =
                    conversations::get_messages_paginated(conn, id, limit, before_turn_index)?;
                let count = conversations::get_message_count(conn, id)?;
                (msgs, count)
            };
            Ok(Some(build_detail_dto(
                conn,
                id,
                conv,
                messages,
                total_count,
                include_records,
            )?))
        }
        None => Ok(None),
    }
}

fn get_conversation_all_impl(
    conn: &rusqlite::Connection,
    id: &str,
    include_records: bool,
) -> Result<Option<ConversationDetailDto>, String> {
    match conversations::get_conversation_by_id(conn, id)? {
        Some(conv) => {
            let messages = conversations::get_messages(conn, id)?;
            let total_count = messages.len() as i64;
            Ok(Some(build_detail_dto(
                conn,
                id,
                conv,
                messages,
                total_count,
                include_records,
            )?))
        }
        None => Ok(None),
    }
}

#[command]
pub async fn get_conversation_token_usage(
    app: AppHandle,
    conversation_id: String,
    context_window_tokens: Option<u32>,
) -> Result<TokenUsageSummaryDto, String> {
    let (summary, detail, provider_row, window, mode_prompt, workspace_root) = {
        let state = app.state::<crate::db_state::DbState>();
        let conn = get_db_conn(&state)?;
        let window = context_window_tokens
            .filter(|&c| c > 0)
            .or_else(|| to_dto::resolve_context_window_for_conversation(&conn, &conversation_id));
        let conversation = conversations::get_conversation_by_id(&conn, &conversation_id)?
            .ok_or_else(|| assistant_i18n::t("error.conversation_not_found"))?;
        let summary = assistant_storage::token_usage::get_summary(
            &conn,
            &conversation_id,
            conversation.active_branch_id.as_deref(),
            window,
        )?;
        let detail = conversations::get_conversation_detail(&conn, &conversation_id)?;
        let provider_row = conversation
            .provider_id
            .as_deref()
            .and_then(|id| provider_store::get_provider_by_id(&conn, id).ok().flatten());
        // 占用估算的系统提示词必须与发送路径同源：`mode.prompt`，不是会话遗留列。
        let mode_prompt = conversation
            .mode_id
            .as_deref()
            .and_then(|id| {
                assistant_storage::agent_modes::get_by_id(&conn, id)
                    .ok()
                    .flatten()
            })
            .map(|m| m.prompt)
            .unwrap_or_default();
        // 估算与真实提示词同源（I38h §4 #3）：同一个会话、同一个解析点。
        let workspace_root = crate::tool_setup::resolve_workspace(&conn, &conversation_id);
        (
            summary,
            detail,
            provider_row,
            window,
            mode_prompt,
            workspace_root,
        )
    };

    let mut dto: TokenUsageSummaryDto = summary.into();

    // 记录表在该分支零记录时，按分支出站投影估算兜底；只读，不写记录表、不写 messages.tokens
    if dto.occupancy.used_tokens.is_none() {
        if let (Some(detail), Some(provider_row)) = (detail, provider_row) {
            if !detail.messages.is_empty() {
                let attachment_store = app.state::<AttachmentStore>();
                let facts = assistant_agent::prompt_builder::PromptFacts {
                    working_directory: workspace_root.clone(),
                };
                let estimated = request::estimate_branch_tokens(
                    &detail,
                    &mode_prompt,
                    &facts,
                    &provider_row,
                    &attachment_store,
                )
                .await;
                if estimated > 0 {
                    let estimated = estimated as i64;
                    let window = window.filter(|&w| w > 0);
                    dto.occupancy.used_tokens = Some(estimated);
                    dto.occupancy.estimated = true;
                    dto.occupancy.estimated_source = Some(TokenUsageEstimatedSourceDto::Projection);
                    dto.occupancy.context_window_tokens = window;
                    dto.occupancy.percentage =
                        window.map(|w| (estimated as f32 / w as f32).clamp(0.0, 1.0));
                }
            }
        }
    }

    Ok(dto)
}
