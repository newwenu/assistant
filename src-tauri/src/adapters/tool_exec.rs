use std::sync::Arc;

use assistant_agent::agent::round_executor::{
    ToolErrorClass, ToolExecCtx, ToolExecResult, ToolExecutionPort, ToolStage, TurnObserver,
};
use assistant_agent::agent::tool_call::FinalizedToolCall;
use assistant_agent::agent::tool_event::{ToolEvent, ToolEventPayload};
use assistant_tools::{
    PipelineEvent, PipelineEventSink, PipelineStage, ResolvedPermissionPolicy, ToolCall,
    ToolErrorKind, ToolOutcome, ToolPipeline, ToolUseContext,
};
use async_trait::async_trait;

use super::approval::ApprovalWaitRegistry;

/// 单次 `pipeline.execute` 的事件出口：把 P2 的三态事件转成内核 `ToolEvent`，
/// 补 `turn_id` / `iteration` / `confirmation_wait_ms` 后交给 `TurnObserver`（§6.1）。
///
/// 只在一次执行期间存活，因此直接借用执行上下文，不持有 `Arc`。
/// `ArgsDelta` 不在此处发 —— 那是流式阶段的产物，与工具执行无关。
struct ObserverPipelineSink<'a> {
    observer: &'a dyn TurnObserver,
    turn_id: &'a str,
    iteration: u32,
    waits: &'a ApprovalWaitRegistry,
}

impl ObserverPipelineSink<'_> {
    fn tool_event(&self, event: PipelineEvent) -> ToolEvent {
        match event {
            PipelineEvent::Started {
                tool_call_id,
                tool_name,
            } => ToolEvent {
                turn_id: self.turn_id.to_string(),
                iteration: self.iteration,
                call_id: tool_call_id,
                name: tool_name,
                payload: ToolEventPayload::Started,
            },
            PipelineEvent::Finished {
                tool_call_id,
                tool_name,
                assistant_text,
            } => {
                let confirmation_wait_ms = self.waits.peek(&tool_call_id);
                ToolEvent {
                    turn_id: self.turn_id.to_string(),
                    iteration: self.iteration,
                    call_id: tool_call_id,
                    name: tool_name,
                    payload: ToolEventPayload::Finished {
                        result: assistant_text,
                        confirmation_wait_ms,
                    },
                }
            }
            PipelineEvent::Failed {
                tool_call_id,
                tool_name,
                error,
                error_kind,
            } => {
                let confirmation_wait_ms = self.waits.peek(&tool_call_id);
                ToolEvent {
                    turn_id: self.turn_id.to_string(),
                    iteration: self.iteration,
                    call_id: tool_call_id,
                    name: tool_name,
                    payload: ToolEventPayload::Failed {
                        error,
                        error_kind: error_kind.map(into_error_class).unwrap_or_else(|| {
                            log::warn!(
                                "pipeline failed event without error_kind, defaulting to internal"
                            );
                            ToolErrorClass::Internal
                        }),
                        confirmation_wait_ms,
                    },
                }
            }
        }
    }
}

#[async_trait]
impl PipelineEventSink for ObserverPipelineSink<'_> {
    async fn emit(&self, event: PipelineEvent) {
        self.observer.emit_tool_event(self.tool_event(event));
    }
}

/// `ToolExecutionPort` 的生产适配器：持有 P2 的 `ToolPipeline` 并只做三件事
/// —— 类型转换、`confirmation_wait_ms` 注入、字段搬运（§4.5 职责切分）。
///
/// 分批、并行、落盘、事件发射全部由 `ToolPipeline::execute` 内部完成，
/// 本适配器**不得**重复调用 `partition_tool_batches` / `select_persist_targets`。
pub struct PipelineToolExecution {
    pipeline: Arc<ToolPipeline>,
    waits: ApprovalWaitRegistry,
    workspace_root: Option<String>,
}

impl PipelineToolExecution {
    pub fn new(
        pipeline: Arc<ToolPipeline>,
        waits: ApprovalWaitRegistry,
        workspace_root: Option<String>,
    ) -> Self {
        Self {
            pipeline,
            waits,
            workspace_root,
        }
    }

    pub fn waits(&self) -> &ApprovalWaitRegistry {
        &self.waits
    }

    /// 结果映射按规格 §4.5 的字段表逐项搬运；等待时长取出后即从注册表移除，
    /// 避免跨迭代无界增长。
    fn exec_result(&self, outcome: ToolOutcome) -> ToolExecResult {
        let confirmation_wait_ms = self.waits.take(&outcome.tool_call_id);
        ToolExecResult {
            call_id: outcome.tool_call_id,
            tool_name: outcome.tool_name,
            content: outcome.assistant_text,
            is_error: !outcome.result.ok,
            stage: into_stage(outcome.stage),
            error_kind: outcome.result.error_kind.map(into_error_class),
            confirmation_wait_ms,
        }
    }
}

#[async_trait]
impl ToolExecutionPort for PipelineToolExecution {
    async fn execute(
        &self,
        calls: Vec<FinalizedToolCall>,
        ctx: &ToolExecCtx<'_>,
    ) -> Vec<ToolExecResult> {
        let tool_calls: Vec<ToolCall> = calls.into_iter().map(into_tool_call).collect();

        // 每次执行现构造：`workspace_root` / `session_id` 是会话级信息，
        // `tool_call_id` 由 Pipeline 在每次调用内部逐条填充。
        let context = ToolUseContext {
            workspace_root: self.workspace_root.clone(),
            session_id: Some(ctx.conversation_id.to_string()),
            tool_call_id: None,
        };
        let policy = ResolvedPermissionPolicy::default_global();

        let sink = ObserverPipelineSink {
            observer: ctx.observer,
            turn_id: ctx.turn_id,
            iteration: ctx.iteration,
            waits: &self.waits,
        };

        self.pipeline
            .execute(&tool_calls, &context, &policy, &sink)
            .await
            .into_iter()
            .map(|outcome| self.exec_result(outcome))
            .collect()
    }
}

fn into_tool_call(call: FinalizedToolCall) -> ToolCall {
    ToolCall::new(call.tool_id, call.tool_name, call.arguments)
}

fn into_stage(stage: PipelineStage) -> ToolStage {
    match stage {
        PipelineStage::Find => ToolStage::Find,
        PipelineStage::Validate => ToolStage::Validate,
        PipelineStage::Gateway => ToolStage::Gateway,
        PipelineStage::Execute => ToolStage::Execute,
        PipelineStage::Persist => ToolStage::Persist,
    }
}

fn into_error_class(kind: ToolErrorKind) -> ToolErrorClass {
    match kind {
        ToolErrorKind::Business => ToolErrorClass::Business,
        ToolErrorKind::Timeout => ToolErrorClass::Timeout,
        ToolErrorKind::Cancelled => ToolErrorClass::Cancelled,
        ToolErrorKind::Internal => ToolErrorClass::Internal,
        ToolErrorKind::Denied => ToolErrorClass::Denied,
    }
}
