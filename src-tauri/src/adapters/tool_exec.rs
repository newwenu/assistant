use std::sync::Arc;

use assistant_agent::agent::node_event::{AgentNode, AgentNodeDetail, NodeState, PipelineDetail};
use assistant_agent::agent::round_executor::{
    ToolErrorClass, ToolExecCtx, ToolExecResult, ToolExecutionPort, ToolStage, TurnObserver,
};
use assistant_agent::agent::tool_call::FinalizedToolCall;
use assistant_agent::agent::tool_event::{
    CompactAnchor, CompactLineRange, CompactPersisted, CompactTruncated, CompactUnit, ToolCompact,
    ToolEvent, ToolEventPayload,
};
use assistant_tools::compact::{ToolResultCompact, TruncatedUnit};
use assistant_tools::{
    PipelineEvent, PipelineEventSink, PipelineStage, ResolvedPermissionPolicy, ToolCall,
    ToolErrorKind, ToolOutcome, ToolPipeline, ToolRegistry, ToolUseContext,
};
use async_trait::async_trait;

use super::approval::ApprovalWaitRegistry;
use super::tool_escalation::{MaxToolTierSource, RegistryEscalationPort};

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
    fn tool_event(&self, event: PipelineEvent) -> Option<ToolEvent> {
        let event = match event {
            // 阶段进入是节点事实，不是工具事件：`agent-tool-call` 不扩展（I55 §4.7）。
            PipelineEvent::StageEntered {
                tool_call_id,
                tool_name,
                stage,
                index,
                total,
            } => {
                self.observer.node(
                    AgentNode::Pipeline,
                    self.iteration,
                    NodeState::Running,
                    false,
                    Some(AgentNodeDetail::Pipeline(PipelineDetail {
                        stage: Some(into_stage(stage)),
                        index: Some(index),
                        total: Some(total),
                        tool_name: Some(tool_name),
                        tool_call_id: Some(tool_call_id),
                    })),
                );
                return None;
            }
            PipelineEvent::Started {
                tool_call_id,
                tool_name,
                args_preview,
            } => ToolEvent {
                turn_id: self.turn_id.to_string(),
                iteration: self.iteration,
                call_id: tool_call_id,
                name: tool_name,
                payload: ToolEventPayload::Started { args_preview },
            },
            PipelineEvent::Finished {
                tool_call_id,
                tool_name,
                assistant_text,
                compact,
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
                        compact: compact.map(into_compact),
                    },
                }
            }
            PipelineEvent::Failed {
                tool_call_id,
                tool_name,
                error,
                error_kind,
                compact,
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
                        compact: compact.map(into_compact),
                    },
                }
            }
        };
        Some(event)
    }
}

#[async_trait]
impl PipelineEventSink for ObserverPipelineSink<'_> {
    async fn emit(&self, event: PipelineEvent) {
        if let Some(tool_event) = self.tool_event(event) {
            self.observer.emit_tool_event(tool_event);
        }
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
    /// 行索引缓存根（I58 §3.5）：由应用层装配时给出，库 crate 不解析基根。
    /// `None` = 未装配索引能力，`super_read` 据此报业务错误。
    index_root: Option<std::path::PathBuf>,
    /// 临时区所有者（I77 §3.2 / C4）：装配期由应用层给出**唯一**实例，
    /// 本适配器只把它搬进上下文；会话段由所有者 `session_root` 派生，不再自行拼路径。
    tool_scratch: Option<Arc<dyn assistant_tools::ToolScratchStore>>,
    /// 升级通道装配（I58 §3.8 ③）：注册表 + 层级来源。`None` = 不提供升级通道，
    /// `request_tool(detail = "enable")` 返回业务错误（§3.8 ⑥）。
    escalation: Option<EscalationWiring>,
}

/// 升级端口的装配事实：注册表引用 + 本轮层级来源。
///
/// 两者都**不是**每轮变化的配置：注册表进程级唯一，层级按会话每次求值，
/// 因此这里只存来源，端口在每次 `execute` 现构造。
struct EscalationWiring {
    registry: Arc<ToolRegistry>,
    max_tool_tier: MaxToolTierSource,
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
            index_root: None,
            tool_scratch: None,
            escalation: None,
        }
    }

    /// 装配行索引缓存根（I58 §3.5）：由应用层传入 `AppDirs::indexes()`，
    /// 本适配器只搬运，不自行解析基根。
    pub fn with_index_root(mut self, index_root: std::path::PathBuf) -> Self {
        self.index_root = Some(index_root);
        self
    }

    /// 装配临时区所有者（I77 §3.2 / C4）：由应用层传入装配期创建的唯一实例，
    /// 本适配器只搬运；会话段由所有者的 `session_root` 派生。
    pub fn with_tool_scratch(
        mut self,
        tool_scratch: Arc<dyn assistant_tools::ToolScratchStore>,
    ) -> Self {
        self.tool_scratch = Some(tool_scratch);
        self
    }

    /// 临时区所有者的只读句柄（I77 C4）：存储管理器据此观测与回收，
    /// 与 shell 工具共享同一实例，禁止另建第二套。
    pub fn scratch(&self) -> Option<&Arc<dyn assistant_tools::ToolScratchStore>> {
        self.tool_scratch.as_ref()
    }

    /// 装配升级通道（I58 §3.8 ③）：注册表 + 本轮层级来源。
    ///
    /// 不装配时 `request_tool(detail = "enable")` 返回业务错误（§3.8 ⑥）；
    /// `list` / `manual` 不受影响。
    pub fn with_escalation(
        mut self,
        registry: Arc<ToolRegistry>,
        max_tool_tier: MaxToolTierSource,
    ) -> Self {
        self.escalation = Some(EscalationWiring {
            registry,
            max_tool_tier,
        });
        self
    }

    /// 派生携带本轮策略的执行器：`pipeline` / 审批等待表 / 工作区来源共享，
    /// 只有策略随「会话有效工具配置」逐轮变化（I38d §4.6），不缓存在全局单例上。
    pub fn with_policy(&self, policy: ResolvedPermissionPolicy) -> Self {
        Self {
            pipeline: self.pipeline.clone(),
            waits: self.waits.clone(),
            workspace_root: self.workspace_root.clone(),
            policy: Arc::new(policy),
            index_root: self.index_root.clone(),
            tool_scratch: self.tool_scratch.clone(),
            escalation: self.escalation.as_ref().map(|wiring| EscalationWiring {
                registry: wiring.registry.clone(),
                max_tool_tier: wiring.max_tool_tier.clone(),
            }),
        }
    }

    pub fn waits(&self) -> &ApprovalWaitRegistry {
        &self.waits
    }

    /// 落盘存储的只读句柄（I61 §3.10，条目 `E3`）：存储管理器据此取 `result_stats` /
    /// `result_retention`，不另建第二套实例，也不自行拼路径。
    pub fn storage(&self) -> &Arc<dyn assistant_tools::ToolResultStorage> {
        self.pipeline.storage()
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
        // `tool_output_root` 取自 Pipeline 的落盘存储（唯一来源），不在此处重算路径。
        let session_id = ctx.conversation_id.to_string();
        // 域根在构造点归一为绑定形态（I73 §3.9 / P1.8）：之后的 `join` /
        // `ensure_within_root` / 审批 resource 都只认这一份。归一失败即视为没有输出域，
        // 不回退原始串——两侧形态不一致会让域内文件被误判成域外。
        let tool_output_root = self
            .pipeline
            .storage()
            .namespace_root(Some(&session_id))
            .and_then(|root| assistant_tools::os::path::canonicalize_lenient(&root).ok());
        // 临时区域根同口径（I73 §5.4 P4.0 / N17）：会话段由所有者唯一实现点派生
        // （I77 INV3：`crate` 内不得自行 join 会话段）。
        // 目录尚未创建时 `canonicalize_lenient` 仍给出确定形态，故这里无需建目录。
        let tool_scratch_root = self.tool_scratch.as_ref().and_then(|store| {
            assistant_tools::os::path::canonicalize_lenient(&store.session_root(Some(&session_id)))
                .ok()
        });
        // 升级端口每次 `execute` 现构造（I58 §3.8 ③）：层级按会话求值，收集器按轮次
        // 归属。任一侧缺失（未装配 / 本链路不支持升级）都保持 `None`，`enable` 分支
        // 据此返回业务错误（§3.8 ⑥），不会静默降级成「申请成功但没生效」。
        let escalation: Option<Arc<dyn assistant_tools::ToolEscalationPort>> =
            match (&self.escalation, ctx.escalations.as_ref()) {
                (Some(wiring), Some(sink)) => Some(Arc::new(RegistryEscalationPort::new(
                    wiring.registry.clone(),
                    (wiring.max_tool_tier)(ctx.conversation_id),
                    sink.clone(),
                ))),
                _ => None,
            };
        let context = ToolUseContext {
            workspace_root: (self.workspace_root)(ctx.conversation_id),
            session_id: Some(session_id),
            turn_id: Some(ctx.turn_id.to_string()),
            iteration: Some(ctx.iteration),
            tool_call_id: None,
            // 本轮取消令牌：MCP 客户端据此中止在途 `tools/call`（I38e §4.3）。
            cancel: Some(ctx.child_token.clone()),
            tool_output_root,
            tool_scratch_root,
            // 行索引缓存根：与输出域同为「可重建的工具产物」，由装配层注入（I58 §3.5）。
            index_root: self.index_root.clone(),
            // 本轮工具集名表：`request_tool` 的清单以它为准（I58 §3.1 G2）。
            // `None` 表示没有本轮事实，工具据此回退注册表并标注（G3）。
            available_tool_names: ctx
                .tools
                .map(|tools| tools.iter().map(|tool| tool.name.clone()).collect()),
            escalation,
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

/// 工具侧信封 → 内核中性信封：只搬字段，不做任何判定。
fn into_compact(compact: ToolResultCompact) -> ToolCompact {
    ToolCompact {
        head: compact.head,
        truncated: compact.truncated.map(|info| CompactTruncated {
            unit: into_unit(info.unit),
            returned: info.returned,
            total: info.total,
        }),
        persisted: compact.persisted.map(|info| CompactPersisted {
            path: info.path,
            bytes: info.bytes,
        }),
        error_kind: compact.error_kind.map(into_error_class),
        anchor: compact.anchor.map(|anchor| CompactAnchor {
            path: anchor.path,
            ranges: anchor
                .ranges
                .into_iter()
                .map(|range| CompactLineRange {
                    start: range.start,
                    end: range.end,
                })
                .collect(),
        }),
    }
}

fn into_unit(unit: TruncatedUnit) -> CompactUnit {
    match unit {
        TruncatedUnit::Lines => CompactUnit::Lines,
        TruncatedUnit::Items => CompactUnit::Items,
        TruncatedUnit::Bytes => CompactUnit::Bytes,
        TruncatedUnit::Unknown => CompactUnit::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use assistant_agent::kernel::{ProviderDelta, TurnPhase};

    use super::*;

    #[derive(Default)]
    struct Recorder {
        nodes: Mutex<Vec<NodeRecord>>,
        tools: Mutex<Vec<ToolEvent>>,
    }

    type NodeRecord = (AgentNode, u32, NodeState, bool, Option<AgentNodeDetail>);

    impl TurnObserver for Recorder {
        fn set_phase(&self, _phase: TurnPhase) {}

        fn set_iteration(&self, _iteration: u32) {}

        fn emit_tool_event(&self, event: ToolEvent) {
            self.tools.lock().unwrap().push(event);
        }

        fn delta(&self, _delta: &ProviderDelta) -> u32 {
            0
        }

        fn node(
            &self,
            node: AgentNode,
            iteration: u32,
            state: NodeState,
            transition: bool,
            detail: Option<AgentNodeDetail>,
        ) {
            self.nodes
                .lock()
                .unwrap()
                .push((node, iteration, state, transition, detail));
        }
    }

    fn sink<'a>(
        recorder: &'a Recorder,
        waits: &'a ApprovalWaitRegistry,
    ) -> ObserverPipelineSink<'a> {
        ObserverPipelineSink {
            observer: recorder,
            turn_id: "turn-1",
            iteration: 2,
            waits,
        }
    }

    /// G12（写侧）：`StageEntered` 是过程事实 —— 必须发 `transition: false` 的
    /// `pipeline` `running`，且**不得**产出 `agent-tool-call`（I55 §4.7）。
    #[test]
    fn stage_entered_is_a_process_fact_not_a_transition() {
        let recorder = Recorder::default();
        let waits = ApprovalWaitRegistry::new();
        let event = PipelineEvent::StageEntered {
            tool_call_id: "call-1".to_string(),
            tool_name: "echo".to_string(),
            stage: PipelineStage::Execute,
            index: 1,
            total: 2,
        };

        assert!(
            sink(&recorder, &waits).tool_event(event).is_none(),
            "StageEntered 不得扩展 agent-tool-call 事件族"
        );

        let nodes = recorder.nodes.lock().unwrap();
        assert_eq!(nodes.len(), 1, "一条 StageEntered 只对应一条节点事实");
        let (node, iteration, state, transition, detail) = &nodes[0];
        assert_eq!(*node, AgentNode::Pipeline);
        assert_eq!(*iteration, 2, "迭代号必须透传调用方，不自行推断");
        assert_eq!(*state, NodeState::Running);
        assert!(
            !*transition,
            "G12: StageEntered 必须是 transition:false，否则会把 5N 条子阶段事实混入状态迁移"
        );
        match detail {
            Some(AgentNodeDetail::Pipeline(detail)) => {
                assert_eq!(detail.stage, Some(ToolStage::Execute));
                assert_eq!(detail.index, Some(1));
                assert_eq!(detail.total, Some(2));
                assert_eq!(detail.tool_call_id.as_deref(), Some("call-1"));
                assert_eq!(detail.tool_name.as_deref(), Some("echo"));
            }
            other => panic!("expected pipeline detail, got {other:?}"),
        }
        assert!(
            recorder.tools.lock().unwrap().is_empty(),
            "过程事实不得混入工具事件通道"
        );
    }

    /// 对照：三态事件是工具事件，不产生节点状态迁移（同一个出口两条互斥分支）。
    #[test]
    fn started_stays_a_tool_event_and_emits_no_node() {
        let recorder = Recorder::default();
        let waits = ApprovalWaitRegistry::new();
        let event = PipelineEvent::Started {
            tool_call_id: "call-1".to_string(),
            tool_name: "echo".to_string(),
            args_preview: None,
        };

        let tool_event = sink(&recorder, &waits)
            .tool_event(event)
            .expect("Started 必须转成 agent-tool-call");
        assert!(matches!(
            tool_event.payload,
            ToolEventPayload::Started { .. }
        ));
        assert_eq!(tool_event.iteration, 2);
        assert!(
            recorder.nodes.lock().unwrap().is_empty(),
            "三态事件不得写节点状态迁移，否则 ⑨ 会出现重复 running"
        );
    }
}
