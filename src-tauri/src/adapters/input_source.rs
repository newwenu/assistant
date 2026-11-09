use std::sync::Arc;

use assistant_agent::chat_options::ReasoningOverride;
use assistant_agent::kernel::{InputError, ProviderCredential, TurnInput, TurnInputSource};
use assistant_agent::prompt_builder::PromptFacts;
use assistant_storage::agent_modes;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::{conversations, providers};
use async_trait::async_trait;
use tauri::Manager;

/// 环境事实的唯一取值点：与工具执行、占用估算、策略加载共用
/// [`crate::tool_setup::resolve_workspace`]，保证「提示词说的」与「工具用的」同源
/// （I38h §4 #2）。
fn prompt_facts(conn: &rusqlite::Connection, conversation_id: &str) -> PromptFacts {
    PromptFacts {
        working_directory: crate::tool_setup::resolve_workspace(conn, conversation_id),
    }
}

/// 一次 turn 的发送参数：mode 选择与推理强度由宿主命令给出，内核不感知来源。
pub struct TauriInputSource {
    app: tauri::AppHandle,
    attachments: Arc<AttachmentStore>,
    mode_id: Option<String>,
    reasoning: ReasoningOverride,
}

impl TauriInputSource {
    pub fn new(
        app: tauri::AppHandle,
        mode_id: Option<String>,
        reasoning: ReasoningOverride,
    ) -> Self {
        let attachments = app.state::<AttachmentStore>().inner().clone();
        Self {
            app,
            attachments: Arc::new(attachments),
            mode_id,
            reasoning,
        }
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
    ) -> Result<T, InputError> {
        let state = self.app.state::<crate::setup::DbState>();
        let conn = state
            .0
            .lock()
            .map_err(|e| InputError::Failed(e.to_string()))?;
        f(&conn).map_err(InputError::Failed)
    }

    fn decrypt_key(
        &self,
        row: &assistant_storage::providers::ProviderRow,
    ) -> Result<String, InputError> {
        let machine_id = assistant_crypto::api_key::get_machine_id()
            .map_err(|e| InputError::Failed(e.to_string()))?;
        assistant_crypto::api_key::decrypt_api_key_b64(&row.api_key_encrypted, &machine_id)
            .map_err(|e| InputError::Failed(e.to_string()))
    }
}

#[async_trait]
impl TurnInputSource for TauriInputSource {
    async fn conversation_detail(&self, conversation_id: &str) -> Result<TurnInput, InputError> {
        let detail = self
            .with_conn(|conn| conversations::get_conversation_detail(conn, conversation_id))?
            .ok_or_else(|| InputError::NotFound(conversation_id.to_string()))?;

        let mode = match self.mode_id.as_deref() {
            Some(mode_id) => self.with_conn(|conn| agent_modes::get_by_id(conn, mode_id))?,
            None => None,
        };

        let provider_id = detail
            .conversation
            .provider_id
            .clone()
            .ok_or_else(|| InputError::NotFound("provider".to_string()))?;
        let row = self
            .with_conn(|conn| providers::get_provider_by_id(conn, &provider_id))?
            .ok_or_else(|| InputError::NotFound(provider_id.clone()))?;
        let api_key = self.decrypt_key(&row)?;
        let prompt_facts = self.with_conn(|conn| Ok(prompt_facts(conn, conversation_id)))?;
        // 适配器构造上移到宿主侧：内核只消费 `TurnInput.adapter`（H1）。
        let adapter: Arc<dyn assistant_providers::base::BaseProviderAdapter> =
            Arc::from(assistant_providers::base::create_provider_adapter(
                assistant_agent::provider_config::build_provider_config(&row),
            ));

        Ok(TurnInput {
            detail,
            mode,
            reasoning: self.reasoning.clone(),
            provider: ProviderCredential { row, api_key },
            attachments: self.attachments.clone(),
            prompt_facts,
            adapter,
        })
    }

    async fn provider(&self, provider_id: &str) -> Result<ProviderCredential, InputError> {
        let row = self
            .with_conn(|conn| providers::get_provider_by_id(conn, provider_id))?
            .ok_or_else(|| InputError::NotFound(provider_id.to_string()))?;
        let api_key = self.decrypt_key(&row)?;
        Ok(ProviderCredential { row, api_key })
    }

    fn attachments(&self) -> Arc<AttachmentStore> {
        self.attachments.clone()
    }
}
