use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use assistant_providers::base::{
    BaseProviderAdapter, ChatDelta, ChatMessage, ChatOptions, ChatStream, ModelConfig,
    ProviderConfig, ProviderError, ProviderKind,
};
use assistant_providers::types::ToolSpec;
use assistant_storage::agent_modes::AgentModeRow;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::conversations::{ConversationDetail, ConversationRow, MessageRow};
use assistant_storage::providers::ProviderRow;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::agent::round_executor::{
    ToolErrorClass, ToolExecCtx, ToolExecResult, ToolExecutionPort, ToolStage,
};
use crate::agent::tool_call::FinalizedToolCall;

use super::{
    AppendToolMessageReq, Clock, CreateAssistantReq, EventSink, InputError, MessageStatus,
    MessageStore, ProviderCredential, StoreError, TurnEvent, TurnInput, TurnInputSource,
    UsageRecord,
};

#[derive(Default)]
pub struct RecordingSink {
    events: Mutex<Vec<TurnEvent>>,
    /// 达到该条数后模拟写事件失败（只记日志，不再入列）。
    fail_after: Option<usize>,
    /// 被模拟失败丢弃的事件条数（V4：失败必须可断言，不得静默）。
    dropped: AtomicUsize,
}

impl RecordingSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failing_after(count: usize) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            fail_after: Some(count),
            dropped: AtomicUsize::new(0),
        }
    }

    pub fn events(&self) -> Vec<TurnEvent> {
        self.lock().clone()
    }

    /// 模拟发射失败后丢弃的事件条数。
    pub fn dropped_count(&self) -> usize {
        self.dropped.load(Ordering::SeqCst)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<TurnEvent>> {
        match self.events.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl EventSink for RecordingSink {
    fn emit(&self, event: TurnEvent) {
        let mut events = self.lock();
        if let Some(limit) = self.fail_after {
            if events.len() >= limit {
                self.dropped.fetch_add(1, Ordering::SeqCst);
                log::error!("recording sink simulated emit failure");
                return;
            }
        }
        events.push(event);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreCall {
    Create(String),
    Save(String),
    Finalize(String, MessageStatus),
    Usage(usize),
    SaveToolCalls(String, String),
    AppendTool(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub parent_id: Option<String>,
    pub tool_calls_json: Option<String>,
    pub tool_call_id: Option<String>,
    pub is_error: bool,
    pub timestamp: i64,
}

#[derive(Default)]
pub struct InMemoryStore {
    calls: Mutex<Vec<StoreCall>>,
    messages: Mutex<Vec<ReplayMessage>>,
    /// 注入后所有写操作立即失败。
    fail: bool,
    next_id: Mutex<u32>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }

    pub fn calls(&self) -> Vec<StoreCall> {
        self.lock_calls().clone()
    }

    /// 已写入的行，按写入顺序；用于「写入 → 读回 → 重建 wire 形状」断言。
    pub fn replay_messages(&self) -> Vec<ReplayMessage> {
        self.lock_messages().clone()
    }

    /// 预置既有行：模拟 `TurnRunner` 在进入循环前创建的 assistant 占位行。
    /// 占位行 id 由调用方给定，不消耗 `next_id`，因此不影响后续行编号。
    pub fn seed_assistant_row(&self, id: &str, parent_id: Option<&str>) {
        self.lock_messages().push(ReplayMessage {
            id: id.to_string(),
            role: "assistant".to_string(),
            content: String::new(),
            parent_id: parent_id.map(str::to_string),
            tool_calls_json: None,
            tool_call_id: None,
            is_error: false,
            timestamp: 0,
        });
    }

    fn lock_calls(&self) -> std::sync::MutexGuard<'_, Vec<StoreCall>> {
        match self.calls.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn lock_messages(&self) -> std::sync::MutexGuard<'_, Vec<ReplayMessage>> {
        match self.messages.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn next_id(&self) -> u32 {
        let mut next = match self.next_id.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *next += 1;
        *next
    }

    fn push(&self, call: StoreCall) -> Result<(), StoreError> {
        // 失败也记录调用序列，便于断言「失败后仍尝试写终态」。
        self.lock_calls().push(call);
        if self.fail {
            return Err(StoreError::Failed("injected failure".to_string()));
        }
        Ok(())
    }
}

#[async_trait]
impl MessageStore for InMemoryStore {
    async fn create_assistant_message(
        &self,
        req: CreateAssistantReq,
    ) -> Result<String, StoreError> {
        let id = format!("{}:assistant:{}", req.conversation_id, self.next_id());
        self.push(StoreCall::Create(id.clone()))?;
        self.lock_messages().push(ReplayMessage {
            id: id.clone(),
            role: "assistant".to_string(),
            content: String::new(),
            parent_id: req.parent_id.clone(),
            tool_calls_json: None,
            tool_call_id: None,
            is_error: false,
            timestamp: 0,
        });
        Ok(id)
    }

    async fn save_content(
        &self,
        msg_id: &str,
        content: &str,
        _tokens: Option<u32>,
        _reasoning: Option<&str>,
    ) -> Result<(), StoreError> {
        self.push(StoreCall::Save(msg_id.to_string()))?;
        if let Some(msg) = self.lock_messages().iter_mut().find(|msg| msg.id == msg_id) {
            msg.content = content.to_string();
        }
        Ok(())
    }

    async fn save_tool_calls(&self, msg_id: &str, tool_calls_json: &str) -> Result<(), StoreError> {
        self.push(StoreCall::SaveToolCalls(
            msg_id.to_string(),
            tool_calls_json.to_string(),
        ))?;
        if let Some(msg) = self.lock_messages().iter_mut().find(|msg| msg.id == msg_id) {
            msg.tool_calls_json = Some(tool_calls_json.to_string());
        }
        Ok(())
    }

    async fn append_tool_message(&self, req: AppendToolMessageReq) -> Result<String, StoreError> {
        let id = format!("{}:tool:{}", req.conversation_id, self.next_id());
        self.push(StoreCall::AppendTool(id.clone()))?;
        self.lock_messages().push(ReplayMessage {
            id: id.clone(),
            role: "tool".to_string(),
            content: req.content,
            parent_id: Some(req.parent_id),
            tool_calls_json: None,
            tool_call_id: Some(req.tool_call_id),
            is_error: req.is_error,
            timestamp: req.timestamp,
        });
        Ok(id)
    }

    async fn finalize(&self, msg_id: &str, status: MessageStatus) -> Result<(), StoreError> {
        self.push(StoreCall::Finalize(msg_id.to_string(), status))
    }

    async fn record_usage(&self, records: &[UsageRecord]) -> Result<(), StoreError> {
        self.push(StoreCall::Usage(records.len()))
    }
}

pub struct ScriptedToolExec {
    results: Mutex<Vec<ToolExecResult>>,
    calls: Mutex<Vec<FinalizedToolCall>>,
    deny_all: bool,
}

impl ScriptedToolExec {
    pub fn new(results: Vec<ToolExecResult>) -> Self {
        Self {
            results: Mutex::new(results),
            calls: Mutex::new(Vec::new()),
            deny_all: false,
        }
    }

    pub fn denied() -> Self {
        Self {
            deny_all: true,
            ..Self::new(Vec::new())
        }
    }

    pub fn recorded_calls(&self) -> Vec<FinalizedToolCall> {
        match self.calls.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[async_trait]
impl ToolExecutionPort for ScriptedToolExec {
    async fn execute(
        &self,
        calls: Vec<FinalizedToolCall>,
        _ctx: &ToolExecCtx<'_>,
    ) -> Vec<ToolExecResult> {
        match self.calls.lock() {
            Ok(mut guard) => guard.extend(calls.iter().cloned()),
            Err(poisoned) => poisoned.into_inner().extend(calls.iter().cloned()),
        }
        let mut scripted = match self.results.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        calls
            .into_iter()
            .map(|call| {
                if !scripted.is_empty() {
                    return scripted.remove(0);
                }
                if self.deny_all {
                    return ToolExecResult {
                        call_id: call.tool_id,
                        tool_name: call.tool_name,
                        content: "denied".to_string(),
                        is_error: true,
                        stage: ToolStage::Gateway,
                        error_kind: Some(ToolErrorClass::Denied),
                        confirmation_wait_ms: None,
                    };
                }
                ToolExecResult {
                    call_id: call.tool_id,
                    tool_name: call.tool_name,
                    content: String::new(),
                    is_error: false,
                    stage: ToolStage::Execute,
                    error_kind: None,
                    confirmation_wait_ms: None,
                }
            })
            .collect()
    }
}

pub struct FixtureSource {
    detail: ConversationDetail,
    provider: ProviderCredential,
    attachments: Arc<AttachmentStore>,
    adapter: Arc<dyn BaseProviderAdapter>,
    mode: Option<AgentModeRow>,
}

impl FixtureSource {
    pub fn new(
        detail: ConversationDetail,
        provider: ProviderCredential,
        attachment_dir: &str,
        adapter: Arc<dyn BaseProviderAdapter>,
    ) -> Self {
        Self {
            detail,
            provider,
            attachments: Arc::new(AttachmentStore::new(attachment_dir.into())),
            adapter,
            mode: None,
        }
    }

    /// 注入 agent mode：`AgentKind::Custom` 才注入 tools，工具循环用例必须走这里。
    pub fn with_mode(mut self, mode: AgentModeRow) -> Self {
        self.mode = Some(mode);
        self
    }

    pub fn simple(conversation_id: &str) -> Self {
        let now = 0;
        let conversation = ConversationRow {
            id: conversation_id.to_string(),
            title: "fixture".to_string(),
            title_source: "user".to_string(),
            provider_id: Some("fixture-provider".to_string()),
            model_id: "fixture-model".to_string(),
            system_prompt: None,
            is_pinned: false,
            sort_order: 0,
            generation: 0,
            created_at: now,
            updated_at: now,
            is_full: false,
            active_branch_id: None,
        };
        let messages: Vec<MessageRow> = Vec::new();
        let provider = ProviderCredential {
            row: ProviderRow {
                id: "fixture-provider".to_string(),
                name: "fixture".to_string(),
                type_: "openai".to_string(),
                base_url: "https://example.invalid".to_string(),
                api_key_encrypted: String::new(),
                models_json: "[]".to_string(),
                default_model: "fixture-model".to_string(),
                updated_at: now,
            },
            api_key: "fixture-key".to_string(),
        };
        Self::new(
            ConversationDetail {
                conversation,
                messages,
            },
            provider,
            "fixture-attachments",
            Arc::new(ScriptedAdapter::new(Vec::new())),
        )
    }
}

#[async_trait]
impl TurnInputSource for FixtureSource {
    async fn conversation_detail(&self, conversation_id: &str) -> Result<TurnInput, InputError> {
        if self.detail.conversation.id != conversation_id {
            return Err(InputError::NotFound(conversation_id.to_string()));
        }
        Ok(TurnInput {
            detail: self.detail.clone(),
            mode: self.mode.clone(),
            reasoning: crate::chat_options::ReasoningOverride { level: None },
            provider: self.provider.clone(),
            attachments: self.attachments.clone(),
            adapter: self.adapter.clone(),
        })
    }

    async fn provider(&self, provider_id: &str) -> Result<ProviderCredential, InputError> {
        if self.provider.row.id != provider_id {
            return Err(InputError::NotFound(provider_id.to_string()));
        }
        Ok(self.provider.clone())
    }

    fn attachments(&self) -> Arc<AttachmentStore> {
        self.attachments.clone()
    }
}

pub struct FixedClock {
    base: std::time::Instant,
}

impl Default for FixedClock {
    fn default() -> Self {
        Self {
            base: std::time::Instant::now(),
        }
    }
}

impl FixedClock {
    /// 固定到指定时刻，用于让 turn 的墙钟预算立即超时。
    pub fn at(base: std::time::Instant) -> Self {
        Self { base }
    }
}

impl Clock for FixedClock {
    fn now(&self) -> std::time::Instant {
        self.base
    }

    /// 测试替身不做真实计时；需要固定落库时间戳的用例自行断言。
    fn now_millis(&self) -> i64 {
        0
    }
}

/// 可推进的测试时钟：单调时刻与墙钟毫秒同步推进，超时用例不再依赖真实 `sleep`。
pub struct ManualClock {
    base: std::time::Instant,
    base_millis: i64,
    offset: Mutex<std::time::Duration>,
}

impl ManualClock {
    pub fn new(base: std::time::Instant, base_millis: i64) -> Self {
        Self {
            base,
            base_millis,
            offset: Mutex::new(std::time::Duration::ZERO),
        }
    }

    /// 以当前真实时刻为基准；用例只关心推进量，不关心绝对起点（H4）。
    pub fn starting_now() -> Self {
        Self::new(std::time::Instant::now(), 0)
    }

    pub fn advance(&self, delta: std::time::Duration) {
        let mut offset = self.lock_offset();
        *offset += delta;
    }

    fn lock_offset(&self) -> std::sync::MutexGuard<'_, std::time::Duration> {
        match self.offset.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl Clock for ManualClock {
    fn now(&self) -> std::time::Instant {
        self.base + *self.lock_offset()
    }

    fn now_millis(&self) -> i64 {
        self.base_millis + self.lock_offset().as_millis() as i64
    }
}

/// 每次读取推进固定步长：首个读取即 turn 起点，后续读取自然超预算，
/// 用于在无真实 `sleep` 的前提下触发墙钟超时。
pub struct StepClock {
    base: std::time::Instant,
    step: std::time::Duration,
    reads: std::sync::atomic::AtomicU64,
}

impl StepClock {
    pub fn new(base: std::time::Instant, step: std::time::Duration) -> Self {
        Self {
            base,
            step,
            reads: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// 以当前真实时刻为基准；用例只关心推进步长，不关心绝对起点（H4）。
    pub fn starting_now(step: std::time::Duration) -> Self {
        Self::new(std::time::Instant::now(), step)
    }
}

impl Clock for StepClock {
    fn now(&self) -> std::time::Instant {
        let read = self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.base + self.step * read as u32
    }

    fn now_millis(&self) -> i64 {
        0
    }
}

/// 单次模型调用的脚本。
pub enum ScriptedResponse {
    /// 依次产出的 delta；末条应带 `finish_reason` 才能正常收尾。
    Deltas(Vec<ChatDelta>),
    /// 先等待指定时长再产出（配合暂停时钟，无真实等待）。
    Delayed(std::time::Duration, Vec<ChatDelta>),
    /// 立即返回 provider 错误。
    Error(ProviderError),
    /// 产出若干 delta 后挂起，用于超时 / 取消用例。
    Stall(Vec<ChatDelta>),
}

impl ScriptedResponse {
    /// 文本增量 + 正常结束。
    pub fn text(chunks: &[&str]) -> Self {
        let mut deltas: Vec<ChatDelta> = chunks
            .iter()
            .map(|chunk| ChatDelta {
                delta_text: Some((*chunk).to_string()),
                ..Default::default()
            })
            .collect();
        deltas.push(ChatDelta::finish("stop"));
        Self::Deltas(deltas)
    }

    /// 无任何增量的空响应。
    pub fn empty() -> Self {
        Self::Deltas(Vec::new())
    }
}

/// 一次模型调用的请求快照：用于断言 nudge 注入、收尾撤 tools 与 wire 配对。
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Option<Vec<ToolSpec>>,
}

/// 脚本化 provider 适配器：按调用顺序返回预设流，L1 用例不再起监听端口（H1 / H8）。
pub struct ScriptedAdapter {
    config: ProviderConfig,
    scripts: Mutex<std::collections::VecDeque<ScriptedResponse>>,
    calls: std::sync::atomic::AtomicU32,
    requests: Mutex<Vec<RecordedRequest>>,
}

impl ScriptedAdapter {
    pub fn new(scripts: Vec<ScriptedResponse>) -> Self {
        Self {
            config: ProviderConfig {
                kind: ProviderKind::OpenAI,
                base_url: Some("scripted://local".to_string()),
                default_model: "fixture-model".to_string(),
                max_tokens: 4096,
                temperature: 0.7,
            },
            scripts: Mutex::new(scripts.into()),
            calls: std::sync::atomic::AtomicU32::new(0),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// 已发生的模型调用次数。
    pub fn calls(&self) -> u32 {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 已发出的请求快照，按调用顺序；用于断言本轮实际发送了什么。
    pub fn requests(&self) -> Vec<RecordedRequest> {
        match self.requests.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn next_script(&self) -> ScriptedResponse {
        let mut scripts = match self.scripts.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        scripts.pop_front().unwrap_or(ScriptedResponse::empty())
    }
}

impl BaseProviderAdapter for ScriptedAdapter {
    fn kind(&self) -> &ProviderKind {
        &self.config.kind
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn chat_completion_stream(
        &self,
        _api_key: &str,
        messages: &[ChatMessage],
        _model: &str,
        options: &ChatOptions,
        _cancel_token: CancellationToken,
    ) -> Result<ChatStream, ProviderError> {
        use futures::StreamExt;

        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match self.requests.lock() {
            Ok(mut guard) => guard.push(RecordedRequest {
                messages: messages.to_vec(),
                tools: options.tools.clone(),
            }),
            Err(poisoned) => poisoned.into_inner().push(RecordedRequest {
                messages: messages.to_vec(),
                tools: options.tools.clone(),
            }),
        }
        match self.next_script() {
            ScriptedResponse::Deltas(deltas) => {
                Ok(Box::pin(futures::stream::iter(deltas.into_iter().map(Ok))))
            }
            ScriptedResponse::Delayed(delay, deltas) => {
                let items: Vec<Result<ChatDelta, ProviderError>> =
                    deltas.into_iter().map(Ok).collect();
                Ok(Box::pin(
                    futures::stream::once(async move {
                        tokio::time::sleep(delay).await;
                        items
                    })
                    .flat_map(futures::stream::iter),
                ))
            }
            ScriptedResponse::Error(error) => Err(error),
            ScriptedResponse::Stall(deltas) => Ok(Box::pin(
                futures::stream::iter(deltas.into_iter().map(Ok)).chain(futures::stream::pending()),
            )),
        }
    }

    fn list_models(&self, _api_key: &str) -> Result<Vec<ModelConfig>, ProviderError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{TurnPhase, TurnStatus};

    #[tokio::test]
    async fn recording_sink_collects_events_in_order() {
        let sink = RecordingSink::new();
        sink.emit(TurnEvent::StreamStart {
            turn_id: "conv:1".to_string(),
            phase: TurnPhase::Thinking,
        });
        sink.emit(TurnEvent::Terminated {
            turn_id: "conv:1".to_string(),
            status: TurnStatus::Completed,
            last_seq: 0,
            error: None,
        });

        let events = sink.events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], TurnEvent::StreamStart { .. }));
        assert!(matches!(
            events[1],
            TurnEvent::Terminated {
                status: TurnStatus::Completed,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn recording_sink_swallows_simulated_failure() {
        let sink = RecordingSink::failing_after(1);
        sink.emit(TurnEvent::StreamStart {
            turn_id: "conv:1".to_string(),
            phase: TurnPhase::Thinking,
        });
        sink.emit(TurnEvent::PhaseChanged {
            turn_id: "conv:1".to_string(),
            phase: TurnPhase::Streaming,
        });

        assert_eq!(
            sink.events().len(),
            1,
            "failing emit must not panic nor abort"
        );
    }

    #[tokio::test]
    async fn in_memory_store_records_call_sequence() {
        let store = InMemoryStore::new();
        let id = store
            .create_assistant_message(CreateAssistantReq {
                conversation_id: "conv".to_string(),
                parent_id: None,
                provider_id: Some("p".to_string()),
                model_id: Some("m".to_string()),
            })
            .await
            .expect("create");
        assert_eq!(id, "conv:assistant:1");

        store
            .save_content(&id, "hello", Some(1), None)
            .await
            .expect("save");
        store
            .finalize(&id, MessageStatus::Completed)
            .await
            .expect("finalize");
        store.record_usage(&[]).await.expect("usage");

        assert_eq!(
            store.calls(),
            vec![
                StoreCall::Create(id.clone()),
                StoreCall::Save(id.clone()),
                StoreCall::Finalize(id.clone(), MessageStatus::Completed),
                StoreCall::Usage(0),
            ]
        );
    }

    #[tokio::test]
    async fn in_memory_store_failure_propagates_and_finalize_still_callable() {
        let store = InMemoryStore::failing();
        let created = store
            .create_assistant_message(CreateAssistantReq {
                conversation_id: "conv".to_string(),
                parent_id: None,
                provider_id: None,
                model_id: None,
            })
            .await;
        assert!(created.is_err(), "injected failure must surface");

        let finalized = store
            .finalize("conv:assistant:1", MessageStatus::Failed)
            .await;
        assert!(finalized.is_err(), "store is fully failing in this fixture");
        assert_eq!(
            store.calls(),
            vec![
                StoreCall::Create("conv:assistant:1".to_string()),
                StoreCall::Finalize("conv:assistant:1".to_string(), MessageStatus::Failed),
            ],
            "failed attempts are still recorded so callers can assert write order"
        );
    }

    #[tokio::test]
    async fn fixture_source_returns_fixtures_and_errors_on_unknown_ids() {
        let source = FixtureSource::simple("conv-1");
        let input = source.conversation_detail("conv-1").await.expect("detail");
        assert_eq!(input.detail.conversation.id, "conv-1");
        assert!(source.conversation_detail("missing").await.is_err());

        let credential = source.provider("fixture-provider").await.expect("provider");
        assert_eq!(credential.api_key, "fixture-key");
        assert!(source.provider("missing").await.is_err());

        assert!(Arc::strong_count(&source.attachments()) >= 2);
    }

    #[test]
    fn fixed_clock_is_monotonic_and_stable() {
        let clock = FixedClock::default();
        let first = clock.now();
        let second = clock.now();
        assert_eq!(first, second);
    }

    #[test]
    fn message_status_maps_to_db_values() {
        assert_eq!(MessageStatus::Streaming.as_db_str(), "streaming");
        assert_eq!(MessageStatus::Completed.as_db_str(), "completed");
        assert_eq!(MessageStatus::Interrupted.as_db_str(), "interrupted");
        assert_eq!(MessageStatus::Failed.as_db_str(), "failed");
    }
}
