use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use crate::compact::{args_preview, build_compact, ToolResultCompact};
use crate::context::ToolUseContext;
use crate::exec::{self, DEFAULT_TOOL_TIMEOUT};
use crate::gateway::{GatewayDecision, ToolGateway};
use crate::policy::ResolvedPermissionPolicy;
use crate::registry::ToolRegistry;
use crate::result::{ToolErrorKind, ToolResult};
use crate::storage::{
    persisted_reference, select_persist_targets, PersistToolResult, PersistedRef, ToolResultStorage,
};

/// 一次工具调用请求。内部类型，不承载 UI 状态，映射到 wire DTO 由适配层完成。
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
}

impl ToolCall {
    pub fn new(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments: Value,
    ) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            arguments,
        }
    }
}

/// 结果最终停在哪个阶段。用于区分「未找到工具」「校验失败」「网关拒绝」等同为
/// `ok: false` 但成因完全不同的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Find,
    Validate,
    Gateway,
    Execute,
    Persist,
}

#[derive(Debug, Clone)]
pub struct ToolOutcome {
    pub tool_call_id: String,
    pub tool_name: String,
    pub stage: PipelineStage,
    /// 校验阶段的「不可改写绕过」标记，供上层决定是否允许模型改输入重试。
    pub blocks_input_rewrite: bool,
    pub result: ToolResult,
    /// 回填模型的文本：落盘后为引用 + 预览，否则为原始回填文本。
    pub assistant_text: String,
    pub persisted: Option<PersistedRef>,
    /// 原始参数：`Started` 的参数摘要与信封 head 的 subject 都从这里取。
    pub arguments: Value,
    /// 紧凑信封：展示与历史重载的唯一事实源（I46 compact §3.4）。
    pub compact: Option<ToolResultCompact>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineEvent {
    Started {
        tool_call_id: String,
        tool_name: String,
        args_preview: Option<String>,
    },
    Finished {
        tool_call_id: String,
        tool_name: String,
        assistant_text: String,
        compact: Option<ToolResultCompact>,
    },
    Failed {
        tool_call_id: String,
        tool_name: String,
        error: String,
        error_kind: Option<ToolErrorKind>,
        compact: Option<ToolResultCompact>,
    },
}

/// 事件端口：P2 只定义 trait 与内存实现，Tauri 事件发射在适配层。
#[async_trait]
pub trait PipelineEventSink: Send + Sync {
    async fn emit(&self, event: PipelineEvent);
}

#[derive(Debug, Default)]
pub struct InMemoryEventSink {
    events: Mutex<Vec<PipelineEvent>>,
}

impl InMemoryEventSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<PipelineEvent> {
        self.events.lock().map(|e| e.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl PipelineEventSink for InMemoryEventSink {
    async fn emit(&self, event: PipelineEvent) {
        match self.events.lock() {
            Ok(mut events) => events.push(event),
            Err(_) => log::warn!("event sink poisoned, dropping pipeline event"),
        }
    }
}

/// 工具执行流水线：`find → validate → gateway → execute → persist`。
///
/// 顺序不可交换：校验先于审批，避免为非法输入弹审批框；执行前必须过网关，
/// 不存在绕过网关的执行路径。
pub struct ToolPipeline {
    registry: Arc<ToolRegistry>,
    gateway: Arc<ToolGateway>,
    storage: Arc<dyn ToolResultStorage>,
    default_timeout: Duration,
}

impl ToolPipeline {
    pub fn new(
        registry: Arc<ToolRegistry>,
        gateway: Arc<ToolGateway>,
        storage: Arc<dyn ToolResultStorage>,
    ) -> Self {
        Self {
            registry,
            gateway,
            storage,
            default_timeout: DEFAULT_TOOL_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// 按并发安全切分 batch：连续可并发的工具合并为一组，其余各自单独成组。
    /// 未注册的工具无法判断并发安全性，按不可并发处理（fail-closed）。
    pub fn partition_tool_batches(&self, tool_calls: &[ToolCall]) -> Vec<Vec<usize>> {
        let mut batches: Vec<Vec<usize>> = Vec::new();
        let mut open = false;

        for (index, call) in tool_calls.iter().enumerate() {
            let safe = self
                .registry
                .find_by_name(&call.tool_name)
                .map(|registered| registered.tool.is_concurrency_safe(Some(&call.arguments)))
                .unwrap_or(false);

            if safe && open {
                if let Some(last) = batches.last_mut() {
                    last.push(index);
                    continue;
                }
            }

            if safe {
                batches.push(vec![index]);
                open = true;
            } else {
                batches.push(vec![index]);
                open = false;
            }
        }

        batches
    }

    /// 执行一轮工具调用。结果按原始顺序返回，与 batch 切分方式无关。
    ///
    /// 落盘选择是**单轮级**决策：必须先拿到本轮全部结果，才能按大小降序
    /// 计算谁超出预算。因此执行与落盘分成两段，中间不做任何顺序假设。
    pub async fn execute(
        &self,
        tool_calls: &[ToolCall],
        context: &ToolUseContext,
        policy: &ResolvedPermissionPolicy,
        sink: &dyn PipelineEventSink,
    ) -> Vec<ToolOutcome> {
        let batches = self.partition_tool_batches(tool_calls);
        let mut slots: Vec<Option<ToolOutcome>> = vec![None; tool_calls.len()];

        for batch in batches {
            for &index in &batch {
                let call = &tool_calls[index];
                sink.emit(PipelineEvent::Started {
                    tool_call_id: call.tool_call_id.clone(),
                    tool_name: call.tool_name.clone(),
                    args_preview: args_preview(&call.tool_name, &call.arguments),
                })
                .await;
            }

            let collected: Vec<(usize, ToolOutcome)> = if batch.len() > 1 {
                let futures = batch.iter().map(|&index| async move {
                    let outcome = self.run_call(&tool_calls[index], context, policy).await;
                    (index, outcome)
                });
                futures::future::join_all(futures).await
            } else {
                let index = batch[0];
                let outcome = self.run_call(&tool_calls[index], context, policy).await;
                vec![(index, outcome)]
            };

            for (index, outcome) in collected {
                slots[index] = Some(outcome);
            }
        }

        let mut outcomes: Vec<ToolOutcome> = slots.into_iter().flatten().collect();
        let sizes: Vec<usize> = outcomes
            .iter()
            .map(|outcome| outcome.assistant_text.chars().count())
            .collect();
        let targets = select_persist_targets(&sizes);

        for (outcome, persist) in outcomes.iter_mut().zip(targets) {
            if persist {
                self.persist(outcome, context.session_id.as_deref()).await;
            }
            outcome.compact = Some(build_compact(
                &outcome.tool_name,
                &outcome.arguments,
                &outcome.result,
                outcome.persisted.as_ref(),
            ));
            self.report(outcome, sink).await;
        }

        outcomes
    }

    /// 阶段 1–4：查找、校验、网关、执行。不落盘、不发完成事件。
    async fn run_call(
        &self,
        call: &ToolCall,
        context: &ToolUseContext,
        policy: &ResolvedPermissionPolicy,
    ) -> ToolOutcome {
        let registered = match self.registry.find_by_name(&call.tool_name) {
            Some(registered) => registered,
            None => {
                return ToolOutcome::new(
                    call,
                    PipelineStage::Find,
                    ToolResult::from_exec_error_kind(
                        &call.tool_name,
                        ToolErrorKind::Internal,
                        format!("unknown tool '{}'", call.tool_name),
                    ),
                )
            }
        };
        let tool = registered.tool.as_ref();

        let mut call_context = context.clone();
        call_context.tool_call_id = Some(call.tool_call_id.clone());

        let validation = tool.validate_input(&call.arguments, &call_context);
        if !validation.ok {
            let message = validation
                .message
                .unwrap_or_else(|| "input validation failed".to_string());
            let message = match validation.error_code {
                Some(code) => format!("{}: {}", code, message),
                None => message,
            };
            let mut outcome = ToolOutcome::new(
                call,
                PipelineStage::Validate,
                ToolResult::from_exec_error_kind(&call.tool_name, ToolErrorKind::Business, message),
            );
            outcome.blocks_input_rewrite = validation.blocks_input_rewrite;
            return outcome;
        }

        let decision = match self
            .gateway
            .evaluate(tool, &call.arguments, &call_context, policy)
            .await
        {
            Ok(decision) => decision,
            Err(error) => {
                return ToolOutcome::new(
                    call,
                    PipelineStage::Gateway,
                    ToolResult::from_exec_error_kind(
                        &call.tool_name,
                        ToolErrorKind::Internal,
                        error,
                    ),
                )
            }
        };

        match decision {
            GatewayDecision::Allowed => {}
            GatewayDecision::Denied { action, resource } => {
                return ToolOutcome::new(
                    call,
                    PipelineStage::Gateway,
                    ToolResult::from_exec_error_kind(
                        &call.tool_name,
                        ToolErrorKind::Denied,
                        format!("permission denied for '{}' on '{}'", action, resource),
                    ),
                )
            }
            GatewayDecision::RejectedByUser { reason } => {
                let message = match reason {
                    Some(reason) => format!("rejected by user: {}", reason),
                    None => "rejected by user".to_string(),
                };
                return ToolOutcome::new(
                    call,
                    PipelineStage::Gateway,
                    ToolResult::from_exec_error_kind(
                        &call.tool_name,
                        ToolErrorKind::Denied,
                        message,
                    ),
                );
            }
            // 超时不是「用户拒绝」：打 `Timeout` 标并给出「无人应答」的文案，
            // 否则模型会误判用户意图（把「不在场」理解成「不同意」）。
            GatewayDecision::ApprovalTimedOut { timeout } => {
                return ToolOutcome::new(
                    call,
                    PipelineStage::Gateway,
                    ToolResult::from_exec_error_kind(
                        &call.tool_name,
                        ToolErrorKind::Timeout,
                        format!(
                            "approval timed out after {:?}; the user did not respond, \
                             so the call was not executed",
                            timeout
                        ),
                    ),
                )
            }
        }

        let result = exec::invoke(tool, &call.arguments, &call_context, self.default_timeout).await;

        ToolOutcome::new(call, PipelineStage::Execute, result)
    }

    /// 阶段 5：落盘单个结果，回填改为引用 + 预览。
    /// 只有成功结果落盘；失败结果的错误文本本身就该完整回填给模型。
    async fn persist(&self, outcome: &mut ToolOutcome, session_id: Option<&str>) {
        if !outcome.result.ok {
            return;
        }

        let request = PersistToolResult {
            tool_call_id: outcome.tool_call_id.clone(),
            tool_name: outcome.tool_name.clone(),
            session_id: session_id.map(str::to_string),
            content: outcome.assistant_text.clone(),
        };

        match self.storage.persist(request).await {
            Ok(reference) => {
                outcome.assistant_text =
                    persisted_reference(&reference.path, &outcome.assistant_text);
                outcome.persisted = Some(reference);
            }
            Err(error) => log::warn!(
                "persist tool result for '{}' failed, keeping inline: {}",
                outcome.tool_call_id,
                error
            ),
        }
    }

    async fn report(&self, outcome: &ToolOutcome, sink: &dyn PipelineEventSink) {
        let event = if outcome.result.ok {
            PipelineEvent::Finished {
                tool_call_id: outcome.tool_call_id.clone(),
                tool_name: outcome.tool_name.clone(),
                assistant_text: outcome.assistant_text.clone(),
                compact: outcome.compact.clone(),
            }
        } else {
            PipelineEvent::Failed {
                tool_call_id: outcome.tool_call_id.clone(),
                tool_name: outcome.tool_name.clone(),
                error: outcome.result.error.clone().unwrap_or_default(),
                error_kind: outcome.result.error_kind,
                compact: outcome.compact.clone(),
            }
        };
        sink.emit(event).await;
    }
}

impl ToolOutcome {
    fn new(call: &ToolCall, stage: PipelineStage, result: ToolResult) -> Self {
        let assistant_text = result.assistant_text();
        Self {
            tool_call_id: call.tool_call_id.clone(),
            tool_name: call.tool_name.clone(),
            stage,
            blocks_input_rewrite: false,
            result,
            assistant_text,
            persisted: None,
            arguments: call.arguments.clone(),
            compact: None,
        }
    }
}
