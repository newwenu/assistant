use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::agent::node_event::{AgentNode, AgentNodeDetail, AgentNodeEvent, NodeState};
use crate::agent::tool_event::ToolEvent;

use crate::kernel::ids::TurnId;
use crate::kernel::outcome::TurnStatus;
use crate::kernel::phase::TurnPhase;
use crate::kernel::ports::{ChatError, EventSink, ProviderDelta, TurnEvent};
use crate::kernel::tasks::TurnTasks;

/// 事件生成的唯一入口：分配单调 `seq`，并在发射前做代际校验（spec §2.2 第一写侧）。
pub struct TurnEmitter {
    sink: Arc<dyn EventSink>,
    tasks: Arc<TurnTasks>,
    conversation_id: String,
    turn_id: TurnId,
    agent_id: String,
    parent_turn_id: Option<String>,
    seq: AtomicU32,
    iteration: AtomicU32,
}

impl TurnEmitter {
    pub fn new(
        sink: Arc<dyn EventSink>,
        tasks: Arc<TurnTasks>,
        conversation_id: impl Into<String>,
        turn_id: TurnId,
        agent_id: impl Into<String>,
        parent_turn_id: Option<String>,
    ) -> Self {
        Self {
            sink,
            tasks,
            conversation_id: conversation_id.into(),
            turn_id,
            agent_id: agent_id.into(),
            parent_turn_id,
            seq: AtomicU32::new(0),
            iteration: AtomicU32::new(1),
        }
    }

    /// 进入新迭代时更新；`Delta` 事件携带该值，适配层不再猜迭代号。
    pub fn set_iteration(&self, iteration: u32) {
        self.iteration.store(iteration, Ordering::SeqCst);
    }

    fn current_iteration(&self) -> u32 {
        self.iteration.load(Ordering::SeqCst)
    }

    pub fn turn_id(&self) -> &str {
        &self.turn_id
    }

    fn is_current(&self) -> bool {
        self.tasks.is_current(&self.conversation_id, &self.turn_id)
    }

    fn dispatch(&self, event: TurnEvent) {
        if !self.is_current() {
            log::debug!(
                "event skipped: turn={} is not the current generation",
                self.turn_id
            );
            return;
        }
        self.sink.emit(event);
    }

    pub fn stream_start(&self, phase: TurnPhase) {
        self.dispatch(TurnEvent::StreamStart {
            turn_id: self.turn_id.clone(),
            phase,
        });
    }

    pub fn phase_changed(&self, phase: TurnPhase) {
        self.dispatch(TurnEvent::PhaseChanged {
            turn_id: self.turn_id.clone(),
            phase,
        });
    }

    /// `seq` 同 turn 内严格单调（首条为 1），跨 turn 由新实例归零。
    pub fn delta(&self, delta: &ProviderDelta) -> u32 {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        self.dispatch(TurnEvent::Delta {
            turn_id: self.turn_id.clone(),
            seq,
            iteration: self.current_iteration(),
            delta: delta.clone(),
        });
        seq
    }

    /// 本 turn 已发射的最后一条 `Delta` 的 `seq`；无增量时为 0。
    pub fn last_seq(&self) -> u32 {
        self.seq.load(Ordering::SeqCst)
    }

    pub fn tool_event(&self, event: ToolEvent) {
        self.dispatch(TurnEvent::Tool(event));
    }

    /// 节点事件（I55 §4.2）：与 `Delta` 共用同一 `seq` 计数器，混排才有时间线语义。
    ///
    /// `iteration` / `at_ms` 都由调用方给出：迭代外节点恒 `iteration = 0`，
    /// `at_ms` 必须来自 `Clock` 端口（内核禁止 `SystemTime::now()`）。
    pub fn node(
        &self,
        node: AgentNode,
        iteration: u32,
        state: NodeState,
        transition: bool,
        at_ms: u64,
        detail: Option<AgentNodeDetail>,
    ) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        self.dispatch(TurnEvent::Node(AgentNodeEvent {
            turn_id: self.turn_id.clone(),
            conversation_id: self.conversation_id.clone(),
            seq,
            node,
            iteration,
            state,
            transition,
            at_ms,
            agent_id: self.agent_id.clone(),
            parent_turn_id: self.parent_turn_id.clone(),
            detail,
        }));
    }

    pub fn compaction_started(&self) {
        self.dispatch(TurnEvent::CompactionStarted {
            turn_id: self.turn_id.clone(),
        });
    }

    pub fn compaction_finished(&self, ok: bool) {
        self.dispatch(TurnEvent::CompactionFinished {
            turn_id: self.turn_id.clone(),
            ok,
        });
    }

    pub fn terminated(&self, status: TurnStatus, error: Option<ChatError>) {
        self.dispatch(TurnEvent::Terminated {
            turn_id: self.turn_id.clone(),
            status,
            last_seq: self.last_seq(),
            error,
        });
    }

    pub fn user_message_confirmed(&self, user_msg_id: impl Into<String>) {
        self.dispatch(TurnEvent::UserMessageConfirmed {
            conversation_id: self.conversation_id.clone(),
            user_msg_id: user_msg_id.into(),
        });
    }
}
