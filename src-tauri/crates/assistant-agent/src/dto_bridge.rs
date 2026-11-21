// 域类型 → DTO 桥接（I62 §4.4）：按 Rust 孤儿规则归属本 crate。

use crate::agent::node_event::{
    AfterIterationDetail, AfterToolDetail, AgentNode, AgentNodeDetail, AgentNodeEvent, BrakeDetail,
    BrakeVerdict, BranchDetail, BudgetRemaining, BudgetSplitDetail, LadderDetail, MiddlewareAction,
    MiddlewareVerdict, ModelDetail, NodeState, PersistDetail, PipelineDetail,
};
use crate::agent::round_executor::{ToolErrorClass, ToolStage};
use crate::agent::tool_event::{CompactUnit, ToolCompact, ToolEventPayload};
use assistant_protocol::*;

impl From<ToolCompact> for ToolResultCompactDto {
    fn from(compact: ToolCompact) -> Self {
        Self {
            head: compact.head,
            truncated: compact.truncated.map(|info| TruncatedDto {
                unit: info.unit.into(),
                returned: info.returned,
                total: info.total,
            }),
            persisted: compact.persisted.map(|info| PersistedResultDto {
                path: info.path,
                bytes: info.bytes,
            }),
            error_kind: compact.error_kind.map(Into::into),
            anchor: compact.anchor.map(|anchor| DocAnchorDto {
                path: anchor.path,
                ranges: anchor
                    .ranges
                    .into_iter()
                    .map(|range| DocLineRangeDto {
                        start: range.start,
                        end: range.end,
                    })
                    .collect(),
            }),
        }
    }
}

impl From<CompactUnit> for TruncatedUnitDto {
    fn from(unit: CompactUnit) -> Self {
        match unit {
            CompactUnit::Lines => Self::Lines,
            CompactUnit::Items => Self::Items,
            CompactUnit::Bytes => Self::Bytes,
            CompactUnit::Unknown => Self::Unknown,
        }
    }
}

impl From<ToolErrorClass> for ToolErrorKindDto {
    fn from(kind: ToolErrorClass) -> Self {
        match kind {
            ToolErrorClass::Business => Self::Business,
            ToolErrorClass::Timeout => Self::Timeout,
            ToolErrorClass::Cancelled => Self::Cancelled,
            ToolErrorClass::Internal => Self::Internal,
            ToolErrorClass::Denied => Self::Denied,
        }
    }
}

impl From<ToolEventPayload> for AgentToolCallPayloadDto {
    fn from(payload: ToolEventPayload) -> Self {
        match payload {
            ToolEventPayload::Started { args_preview } => Self::Started { args_preview },
            ToolEventPayload::ArgsDelta { delta } => Self::ArgsDelta { delta },
            ToolEventPayload::Finished {
                result,
                confirmation_wait_ms,
                compact,
            } => Self::Finished {
                result,
                confirmation_wait_ms,
                compact: compact.map(Into::into),
            },
            ToolEventPayload::Failed {
                error,
                error_kind,
                confirmation_wait_ms,
                compact,
            } => Self::Failed {
                error,
                error_kind: error_kind.into(),
                confirmation_wait_ms,
                compact: compact.map(Into::into),
            },
        }
    }
}

impl From<AgentNode> for AgentNodeDto {
    fn from(node: AgentNode) -> Self {
        match node {
            AgentNode::Admit => Self::Admit,
            AgentNode::Prepare => Self::Prepare,
            AgentNode::Brake => Self::Brake,
            AgentNode::Ladder => Self::Ladder,
            AgentNode::Model => Self::Model,
            AgentNode::PersistAssistant => Self::PersistAssistant,
            AgentNode::Branch => Self::Branch,
            AgentNode::BudgetSplit => Self::BudgetSplit,
            AgentNode::Pipeline => Self::Pipeline,
            AgentNode::AfterTool => Self::AfterTool,
            AgentNode::PersistTools => Self::PersistTools,
            AgentNode::AfterIteration => Self::AfterIteration,
        }
    }
}

impl From<NodeState> for NodeStateDto {
    fn from(state: NodeState) -> Self {
        match state {
            NodeState::Pending => Self::Pending,
            NodeState::Running => Self::Running,
            NodeState::Waiting => Self::Waiting,
            NodeState::Finished => Self::Finished,
            NodeState::Error => Self::Error,
            NodeState::Skipped => Self::Skipped,
            NodeState::Cancelled => Self::Cancelled,
        }
    }
}

impl From<ToolStage> for PipelineStageDto {
    fn from(stage: ToolStage) -> Self {
        match stage {
            ToolStage::Find => Self::Find,
            ToolStage::Validate => Self::Validate,
            ToolStage::Gateway => Self::Gateway,
            ToolStage::Execute => Self::Execute,
            ToolStage::Persist => Self::Persist,
        }
    }
}

impl From<BrakeVerdict> for BrakeVerdictDto {
    fn from(verdict: BrakeVerdict) -> Self {
        match verdict {
            BrakeVerdict::Continue => Self::Continue,
            BrakeVerdict::Stop => Self::Stop,
        }
    }
}

impl From<MiddlewareAction> for MiddlewareActionDto {
    fn from(action: MiddlewareAction) -> Self {
        match action {
            MiddlewareAction::Continue => Self::Continue,
            MiddlewareAction::Stop => Self::Stop,
        }
    }
}

impl From<BudgetRemaining> for BudgetRemainingDto {
    fn from(budget: BudgetRemaining) -> Self {
        Self {
            remaining_iterations: budget.remaining_iterations,
            remaining_tool_calls: budget.remaining_tool_calls,
            remaining_wall_clock_ms: budget.remaining_wall_clock_ms,
        }
    }
}

impl From<BrakeDetail> for BrakeDetailDto {
    fn from(detail: BrakeDetail) -> Self {
        Self {
            verdict: detail.verdict.into(),
            hook: detail.hook,
            budget: detail.budget.into(),
        }
    }
}

impl From<LadderDetail> for LadderDetailDto {
    fn from(detail: LadderDetail) -> Self {
        Self {
            applied: detail.applied,
            dropped_messages: detail.dropped_messages,
            trimmed_chars: detail.trimmed_chars,
        }
    }
}

impl From<ModelDetail> for ModelDetailDto {
    fn from(detail: ModelDetail) -> Self {
        Self {
            attempt: detail.attempt,
            retry_reason: detail.retry_reason,
            prompt_tokens: detail.prompt_tokens,
            completion_tokens: detail.completion_tokens,
            finish_reason: detail.finish_reason,
            tool_call_count: detail.tool_call_count,
        }
    }
}

impl From<PipelineDetail> for PipelineDetailDto {
    fn from(detail: PipelineDetail) -> Self {
        Self {
            stage: detail.stage.map(Into::into),
            index: detail.index,
            total: detail.total,
            tool_name: detail.tool_name,
            tool_call_id: detail.tool_call_id,
        }
    }
}

impl From<BudgetSplitDetail> for BudgetSplitDetailDto {
    fn from(detail: BudgetSplitDetail) -> Self {
        Self {
            requested: detail.requested,
            executed: detail.executed,
            deferred: detail.deferred,
        }
    }
}

impl From<BranchDetail> for BranchDetailDto {
    fn from(detail: BranchDetail) -> Self {
        Self {
            has_tool_calls: detail.has_tool_calls,
            wrap_up: detail.wrap_up,
            tool_call_count: detail.tool_call_count,
        }
    }
}

impl From<PersistDetail> for PersistDetailDto {
    fn from(detail: PersistDetail) -> Self {
        Self {
            rows: detail.rows,
            message_ids: detail.message_ids,
        }
    }
}

impl From<MiddlewareVerdict> for MiddlewareVerdictDto {
    fn from(verdict: MiddlewareVerdict) -> Self {
        Self {
            middleware: verdict.middleware,
            action: verdict.action.into(),
            reason: verdict.reason,
        }
    }
}

impl From<AfterToolDetail> for AfterToolDetailDto {
    fn from(detail: AfterToolDetail) -> Self {
        Self {
            verdicts: detail.verdicts.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AfterIterationDetail> for AfterIterationDetailDto {
    fn from(detail: AfterIterationDetail) -> Self {
        Self {
            nudges: detail.nudges,
            next_iteration: detail.next_iteration,
        }
    }
}

impl From<AgentNodeDetail> for AgentNodeDetailDto {
    fn from(detail: AgentNodeDetail) -> Self {
        match detail {
            AgentNodeDetail::Brake(detail) => Self::Brake(detail.into()),
            AgentNodeDetail::Ladder(detail) => Self::Ladder(detail.into()),
            AgentNodeDetail::Model(detail) => Self::Model(detail.into()),
            AgentNodeDetail::Pipeline(detail) => Self::Pipeline(detail.into()),
            AgentNodeDetail::BudgetSplit(detail) => Self::BudgetSplit(detail.into()),
            AgentNodeDetail::Branch(detail) => Self::Branch(detail.into()),
            AgentNodeDetail::Persist(detail) => Self::Persist(detail.into()),
            AgentNodeDetail::AfterTool(detail) => Self::AfterTool(detail.into()),
            AgentNodeDetail::AfterIteration(detail) => Self::AfterIteration(detail.into()),
        }
    }
}

impl From<AgentNodeEvent> for AgentNodeEventDto {
    fn from(event: AgentNodeEvent) -> Self {
        Self {
            turn_id: event.turn_id,
            conversation_id: event.conversation_id,
            seq: event.seq,
            node: event.node.into(),
            iteration: event.iteration,
            state: event.state.into(),
            transition: event.transition,
            at_ms: event.at_ms,
            agent_id: event.agent_id,
            parent_turn_id: event.parent_turn_id,
            detail: event.detail.map(Into::into),
        }
    }
}

impl From<crate::kernel::TurnPhase> for TurnPhaseDto {
    fn from(phase: crate::kernel::TurnPhase) -> Self {
        use crate::kernel::TurnPhase;
        match phase {
            TurnPhase::Starting => Self::Starting,
            TurnPhase::Compacting => Self::Compacting,
            TurnPhase::Thinking => Self::Thinking,
            TurnPhase::Streaming => Self::Streaming,
            TurnPhase::ToolCalling => Self::ToolCalling,
            TurnPhase::AwaitingApproval => Self::AwaitingApproval,
            TurnPhase::Finalizing => Self::Finalizing,
        }
    }
}

impl From<crate::kernel::SessionState> for SessionStateDto {
    fn from(state: crate::kernel::SessionState) -> Self {
        use crate::kernel::SessionState;
        match state {
            SessionState::Idle => Self::Idle,
            SessionState::Processing { turn_id, phase } => Self::Processing {
                turn_id,
                phase: phase.into(),
            },
            SessionState::Error {
                message,
                recoverable,
            } => Self::Error {
                message,
                recoverable,
            },
        }
    }
}
