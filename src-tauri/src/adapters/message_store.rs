use assistant_agent::kernel::{
    AppendToolMessageReq, CreateAssistantReq, DeclareToolCallsReq, MessageStatus, MessageStore,
    StoreError, UsageRecord,
};
use assistant_storage::conversations::{self, MessageRow};
use assistant_storage::token_usage;
use async_trait::async_trait;
use tauri::Manager;

pub struct DbMessageStore {
    app: tauri::AppHandle,
}

impl DbMessageStore {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
    ) -> Result<T, StoreError> {
        let state = self.app.state::<crate::setup::DbState>();
        let conn = state
            .0
            .lock()
            .map_err(|e| StoreError::Failed(e.to_string()))?;
        f(&conn).map_err(StoreError::Failed)
    }
}

#[async_trait]
impl MessageStore for DbMessageStore {
    async fn create_assistant_message(
        &self,
        req: CreateAssistantReq,
    ) -> Result<String, StoreError> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
            let sibling_index = conversations::get_sibling_count(
                &tx,
                &req.conversation_id,
                req.parent_id.as_deref(),
            )?;
            let id = uuid::Uuid::new_v4().to_string();
            conversations::insert_message(
                &tx,
                &MessageRow {
                    id: id.clone(),
                    conversation_id: req.conversation_id.clone(),
                    role: "assistant".to_string(),
                    content: String::new(),
                    tokens: None,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    parent_id: req.parent_id.clone(),
                    is_pinned: false,
                    reasoning: None,
                    tool_calls_json: None,
                    sibling_index,
                    provider_id: req.provider_id.clone(),
                    model_id: req.model_id.clone(),
                    tool_call_id: None,
                    is_error: false,
                    status: "streaming".to_string(),
                    mode_id: req.mode_id.clone(),
                    turn_index: req.turn_index.map(i64::from),
                    iteration: i64::from(req.iteration),
                },
            )?;
            conversations::update_message_status(&tx, &id, MessageStatus::Streaming.as_db_str())?;
            conversations::advance_active_branch(
                &tx,
                &req.conversation_id,
                req.parent_id.as_deref(),
                &id,
            )?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(id)
        })
    }

    async fn save_content(
        &self,
        msg_id: &str,
        content: &str,
        tokens: Option<u32>,
        reasoning: Option<&str>,
    ) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conversations::update_message_content(
                conn,
                msg_id,
                content,
                tokens.map(i64::from),
                reasoning,
            )
        })
    }

    async fn save_tool_calls(&self, msg_id: &str, tool_calls_json: &str) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conversations::update_message_tool_calls(conn, msg_id, tool_calls_json)
        })
    }

    async fn declare_tool_calls(
        &self,
        req: DeclareToolCallsReq,
    ) -> Result<Vec<String>, StoreError> {
        self.with_conn(|conn| {
            // 占位行 id 由宿主生成（storage 不持 uuid），与结果行同一批挂在 owner 之下。
            // 整批插入的原子性由 storage 层单事务保证，这里不再叠一层事务。
            let rows: Vec<(String, String)> = req
                .call_ids
                .iter()
                .map(|call_id| (uuid::Uuid::new_v4().to_string(), call_id.clone()))
                .collect();
            conversations::insert_tool_placeholders(
                conn,
                &req.conversation_id,
                &req.parent_id,
                &rows,
                req.turn_index.map(i64::from),
                i64::from(req.iteration),
                chrono::Utc::now().timestamp_millis(),
            )?;
            Ok(rows.into_iter().map(|(id, _)| id).collect())
        })
    }

    async fn mark_tool_calls_running(
        &self,
        conversation_id: &str,
        call_ids: &[String],
    ) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            match conversations::mark_tool_rows_running(conn, conversation_id, call_ids) {
                Ok(()) => Ok(()),
                Err(err) => {
                    // 状态推进失败时调用方仍会执行工具，`pending` 已不能再断言「未开始」：
                    // 先把保守文案写定，避免投影期退化成可重试提示诱导重复副作用操作。
                    // 真正的结果到达时会整行覆盖；这次补救失败也不阻断执行。
                    if let Err(fallback) =
                        conversations::mark_tool_rows_unknown(conn, conversation_id, call_ids)
                    {
                        log::warn!(
                            "会话 {conversation_id} 占位行状态推进失败且保守文案回填失败：{fallback}"
                        );
                    }
                    Err(err)
                }
            }
        })
    }

    async fn fill_tool_messages(&self, reqs: Vec<AppendToolMessageReq>) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
            for req in &reqs {
                let Some(id) = conversations::find_tool_placeholder(
                    &tx,
                    &req.conversation_id,
                    &req.tool_call_id,
                )?
                else {
                    // 存量会话没有占位行：已离开写入窗口，不补行，留给投影期合成。
                    continue;
                };
                conversations::fill_tool_row(&tx, &id, &req.content, req.is_error)?;
            }
            tx.commit().map_err(|e| e.to_string())?;
            Ok(())
        })
    }

    async fn append_tool_messages(
        &self,
        reqs: Vec<AppendToolMessageReq>,
    ) -> Result<Vec<String>, StoreError> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
            let mut ids = Vec::with_capacity(reqs.len());
            let mut parent_id = reqs.first().map(|req| req.parent_id.clone());
            for req in &reqs {
                // 有占位行则原地回填（拓扑不动），没有才插新行（存量会话）。
                if let Some(id) = conversations::find_tool_placeholder(
                    &tx,
                    &req.conversation_id,
                    &req.tool_call_id,
                )? {
                    conversations::fill_tool_row(&tx, &id, &req.content, req.is_error)?;
                    ids.push(id);
                    continue;
                }
                let id = uuid::Uuid::new_v4().to_string();
                conversations::insert_message(
                    &tx,
                    &MessageRow {
                        id: id.clone(),
                        conversation_id: req.conversation_id.clone(),
                        role: "tool".to_string(),
                        content: req.content.clone(),
                        tokens: None,
                        timestamp: req.timestamp,
                        parent_id: parent_id.clone(),
                        is_pinned: false,
                        reasoning: None,
                        tool_calls_json: None,
                        sibling_index: 0,
                        provider_id: None,
                        model_id: None,
                        tool_call_id: Some(req.tool_call_id.clone()),
                        is_error: req.is_error,
                        status: "completed".to_string(),
                        mode_id: None,
                        turn_index: req.turn_index.map(i64::from),
                        iteration: i64::from(req.iteration),
                    },
                )?;
                conversations::advance_active_branch(
                    &tx,
                    &req.conversation_id,
                    parent_id.as_deref(),
                    &id,
                )?;
                parent_id = Some(id.clone());
                ids.push(id);
            }
            tx.commit().map_err(|e| e.to_string())?;
            Ok(ids)
        })
    }

    async fn finalize(&self, msg_id: &str, status: MessageStatus) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            conversations::update_message_status(conn, msg_id, status.as_db_str())
        })
    }

    async fn record_usage(&self, records: &[UsageRecord]) -> Result<(), StoreError> {
        self.with_conn(|conn| {
            for record in records {
                token_usage::insert_record(conn, record)?;
            }
            Ok(())
        })
    }
}
