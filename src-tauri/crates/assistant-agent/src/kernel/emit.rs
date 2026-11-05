use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::ids::TurnId;
use super::outcome::TurnStatus;
use super::phase::TurnPhase;
use super::ports::{ChatError, EventSink, ProviderDelta, TurnEvent};
use super::tasks::TurnTasks;

/// 事件生成的唯一入口：分配单调 `seq`，并在发射前做代际校验（spec §2.2 第一写侧）。
pub struct TurnEmitter {
    sink: Arc<dyn EventSink>,
    tasks: Arc<TurnTasks>,
    conversation_id: String,
    turn_id: TurnId,
    seq: AtomicU32,
    iteration: AtomicU32,
}

impl TurnEmitter {
    pub fn new(
        sink: Arc<dyn EventSink>,
        tasks: Arc<TurnTasks>,
        conversation_id: impl Into<String>,
        turn_id: TurnId,
    ) -> Self {
        Self {
            sink,
            tasks,
            conversation_id: conversation_id.into(),
            turn_id,
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

    pub fn tool_started(&self, call_id: impl Into<String>, name: impl Into<String>) {
        self.dispatch(TurnEvent::ToolStarted {
            turn_id: self.turn_id.clone(),
            call_id: call_id.into(),
            name: name.into(),
        });
    }

    pub fn tool_finished(&self, call_id: impl Into<String>, ok: bool) {
        self.dispatch(TurnEvent::ToolFinished {
            turn_id: self.turn_id.clone(),
            call_id: call_id.into(),
            ok,
        });
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
