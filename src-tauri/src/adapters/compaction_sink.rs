use assistant_agent::kernel::CompactionSink;
use assistant_storage::{conversations, providers};
use async_trait::async_trait;
use tauri::Manager;

/// 终止式压缩的宿主侧执行者：内核只判定「该压缩」，实际压缩在宿主异步跑。
pub struct TauriCompactionSink {
    app: tauri::AppHandle,
}

impl TauriCompactionSink {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }

    fn load(
        &self,
        conversation_id: &str,
    ) -> Result<Option<(conversations::ConversationDetail, providers::ProviderRow)>, String> {
        let state = self.app.state::<crate::setup::DbState>();
        let conn = state.0.lock().map_err(|e| e.to_string())?;
        let Some(detail) = conversations::get_conversation_detail(&conn, conversation_id)? else {
            return Ok(None);
        };
        let Some(provider_id) = detail.conversation.provider_id.clone() else {
            return Ok(None);
        };
        let Some(provider_row) = providers::get_provider_by_id(&conn, &provider_id)? else {
            return Ok(None);
        };
        Ok(Some((detail, provider_row)))
    }
}

#[async_trait]
impl CompactionSink for TauriCompactionSink {
    async fn terminate(&self, conversation_id: &str) {
        let loaded = match self.load(conversation_id) {
            Ok(Some(pair)) => pair,
            Ok(None) => {
                log::warn!(
                    "compaction skipped: conversation={} has no provider/detail",
                    conversation_id
                );
                return;
            }
            Err(err) => {
                log::error!(
                    "compaction skipped: conversation={} load failed: {err}",
                    conversation_id
                );
                return;
            }
        };
        let (detail, provider_row) = loaded;
        let app = self.app.clone();
        let conv_id = conversation_id.to_string();
        tauri::async_runtime::spawn(async move {
            if let Err(err) =
                crate::cmd::execute_compaction(&app, &conv_id, &detail, &provider_row).await
            {
                log::error!("Auto compaction failed for {}: {}", conv_id, err);
            }
        });
    }
}
