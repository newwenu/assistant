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
///
/// 工作区根目录的取值来源：按会话 id 每次执行重新求值（I38h §4 #1）。
///
/// 入参是 `conversation_id` 而不是空参——无参会让实现退回「全局取值」的老路，
/// 而工作区是会话级绑定。
pub type WorkspaceRootSource = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

pub struct PipelineToolExecution {
    pipeline: Arc<ToolPipeline>,
    waits: ApprovalWaitRegistry,
    workspace_root: WorkspaceRootSource,
    policy: Arc<ResolvedPermissionPolicy>,
}

impl PipelineToolExecution {
    pub fn new(
        pipeline: Arc<ToolPipeline>,
        waits: ApprovalWaitRegistry,
        workspace_root: Option<String>,
    ) -> Self {
        Self::with_workspace_source(
            pipeline,
            waits,
            Arc::new(move |_: &str| workspace_root.clone()),
        )
    }

    pub fn with_workspace_source(
        pipeline: Arc<ToolPipeline>,
        waits: ApprovalWaitRegistry,
        workspace_root: WorkspaceRootSource,
    ) -> Self {
        Self {
            pipeline,
            waits,
            workspace_root,
            policy: Arc::new(ResolvedPermissionPolicy::default_global()),
        }
    }

    /// 派生携带本轮策略的执行器：`pipeline` / 审批等待表 / 工作区来源共享，
    /// 只有策略随「会话有效工具配置」逐轮变化（I38d §4.6），不缓存在全局单例上。
    pub fn with_policy(&self, policy: ResolvedPermissionPolicy) -> Self {
        Self {
            pipeline: self.pipeline.clone(),
            waits: self.waits.clone(),
            workspace_root: self.workspace_root.clone(),
            policy: Arc::new(policy),
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
        // Agent 链路只填 `workspace_root`：它兼作权限边界，同时经 `resolution_base`
        // 的回退作为相对路径基准。不额外填 `resolve_base`，避免出现「有基准无边界」的上下文。
        let context = ToolUseContext {
            workspace_root: (self.workspace_root)(ctx.conversation_id),
            session_id: Some(ctx.conversation_id.to_string()),
            turn_id: Some(ctx.turn_id.to_string()),
            iteration: Some(ctx.iteration),
            tool_call_id: None,
            ..Default::default()
        };
        let policy = self.policy.as_ref();

        let sink = ObserverPipelineSink {
            observer: ctx.observer,
            turn_id: ctx.turn_id,
            iteration: ctx.iteration,
            waits: &self.waits,
        };

        self.pipeline
            .execute(&tool_calls, &context, policy, &sink)
            .await
            .into_iter()
            .map(|outcome| self.exec_result(outcome))
            .collect()
    }
}

fn into_tool_call(call: FinalizedToolCall) -> ToolCall {
    ToolCall::new(call.tool_id, call.tool_name, call.arguments)
}

/// `PipelineStage` → `ToolStage` 端口镜像（一一对应）。
///
/// 不单独设镜像测试：本函数为穷尽 match，`PipelineStage` 新增变体即在此编译期报
/// non-exhaustive；各 stage 取值由 `crates/assistant-tools/tests/pipeline.rs` 覆盖。
/// 若在测试内另抄一份映射再断言往返恒等，断言只校验副本，改坏本函数也不会失败。
fn into_stage(stage: PipelineStage) -> ToolStage {
    match stage {
        PipelineStage::Find => ToolStage::Find,
        PipelineStage::Validate => ToolStage::Validate,
        PipelineStage::Gateway => ToolStage::Gateway,
        PipelineStage::Execute => ToolStage::Execute,
        PipelineStage::Persist => ToolStage::Persist,
    }
}

/// `ToolErrorKind` → `ToolErrorClass` 端口镜像（一一对应），理由同 [`into_stage`]。
fn into_error_class(kind: ToolErrorKind) -> ToolErrorClass {
    match kind {
        ToolErrorKind::Business => ToolErrorClass::Business,
        ToolErrorKind::Timeout => ToolErrorClass::Timeout,
        ToolErrorKind::Cancelled => ToolErrorClass::Cancelled,
        ToolErrorKind::Internal => ToolErrorClass::Internal,
        ToolErrorKind::Denied => ToolErrorClass::Denied,
    }
}
