use assistant_agent::kernel::{
    AppendToolMessageReq, CreateAssistantReq, MessageStatus, MessageStore, StoreError, UsageRecord,
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

    async fn append_tool_message(&self, req: AppendToolMessageReq) -> Result<String, StoreError> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
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
                    parent_id: Some(req.parent_id.clone()),
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
                },
            )?;
            conversations::update_message_status(&tx, &id, MessageStatus::Completed.as_db_str())?;
            conversations::advance_active_branch(
                &tx,
                &req.conversation_id,
                Some(req.parent_id.as_str()),
                &id,
            )?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(id)
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