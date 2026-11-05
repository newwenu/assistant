use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use super::ids::TurnId;
use super::outcome::{TurnOutcome, TurnStatus};
use super::phase::{SessionState, TurnPhase};
use super::OnBusy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    /// 会话正在跑另一个 turn；由宿主命令翻译成用户可见错误。
    Busy,
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => write!(f, "session busy"),
        }
    }
}

impl std::error::Error for StateError {}

#[derive(Default)]
pub struct SessionStates(Arc<Mutex<HashMap<String, SessionState>>>);

impl SessionStates {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_map(&self) -> MutexGuard<'_, HashMap<String, SessionState>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("SessionStates mutex poisoned, recovering");
                poisoned.into_inner()
            }
        }
    }

    /// 未记录的会话视为 `Idle`；只写入 `Idle`，不覆盖已存在的 `Processing` / `Error`。
    pub fn ensure_idle(&self, conversation_id: &str) {
        self.lock_map()
            .entry(conversation_id.to_string())
            .or_insert(SessionState::Idle);
    }

    pub fn get(&self, conversation_id: &str) -> Option<SessionState> {
        self.lock_map().get(conversation_id).cloned()
    }

    /// 判据为行为：`Idle` 或可恢复的 `Error`；未记录同样可开始。
    pub fn can_start_new_turn(&self, conversation_id: &str) -> bool {
        match self.lock_map().get(conversation_id) {
            None | Some(SessionState::Idle) => true,
            Some(SessionState::Error { recoverable, .. }) => *recoverable,
            Some(SessionState::Processing { .. }) => false,
        }
    }

    pub fn begin_turn(&self, conversation_id: &str, turn_id: &TurnId) -> Result<(), StateError> {
        self.begin_turn_with(conversation_id, turn_id, OnBusy::Reject)
    }

    /// `Replace` 允许覆盖仍处于 `Processing` 的旧代际：`TurnTasks::begin` 已取消旧令牌，
    /// 旧 turn 的写侧随后都会因代际校验而静默退出，因此覆盖是安全且必要的。
    pub fn begin_turn_with(
        &self,
        conversation_id: &str,
        turn_id: &TurnId,
        on_busy: OnBusy,
    ) -> Result<(), StateError> {
        let mut map = self.lock_map();
        let blocked = match map.get(conversation_id) {
            Some(SessionState::Processing { .. }) => on_busy == OnBusy::Reject,
            Some(SessionState::Error {
                recoverable: false, ..
            }) => true,
            _ => false,
        };
        if blocked {
            return Err(StateError::Busy);
        }
        map.insert(
            conversation_id.to_string(),
            SessionState::Processing {
                turn_id: turn_id.clone(),
                phase: TurnPhase::Starting,
            },
        );
        Ok(())
    }

    /// 非当前代际的 `turn_id` 静默丢弃（design §3.3）。
    pub fn set_phase(&self, conversation_id: &str, turn_id: &TurnId, phase: TurnPhase) {
        let mut map = self.lock_map();
        let stale = match map.get(conversation_id) {
            Some(SessionState::Processing {
                turn_id: current, ..
            }) => current != turn_id,
            _ => true,
        };
        if stale {
            log::warn!(
                "set_phase ignored: conversation={} turn={} is not the current generation",
                conversation_id,
                turn_id
            );
            return;
        }
        map.insert(
            conversation_id.to_string(),
            SessionState::Processing {
                turn_id: turn_id.clone(),
                phase,
            },
        );
    }

    /// `Processing` 的唯一出口，必然写入终态（`Idle` 或 `Error`）。
    pub fn finish_turn(&self, conversation_id: &str, turn_id: &TurnId, outcome: &TurnOutcome) {
        let mut map = self.lock_map();
        let stale = match map.get(conversation_id) {
            Some(SessionState::Processing {
                turn_id: current, ..
            }) => current != turn_id,
            _ => true,
        };
        if stale {
            log::warn!(
                "finish_turn ignored: conversation={} turn={} is not the current generation",
                conversation_id,
                turn_id
            );
            return;
        }
        let next = match outcome.status {
            TurnStatus::Failed => SessionState::Error {
                message: outcome
                    .error
                    .as_ref()
                    .map(|err| format!("{err:?}"))
                    .unwrap_or_default(),
                recoverable: true,
            },
            TurnStatus::Completed | TurnStatus::Cancelled | TurnStatus::Interrupted => {
                SessionState::Idle
            }
        };
        map.insert(conversation_id.to_string(), next);
    }

    /// 回到「未记录」状态，等价于干净空闲。
    pub fn remove(&self, conversation_id: &str) {
        self.lock_map().remove(conversation_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrecoverable_error_blocks_new_turn() {
        let states = SessionStates::new();
        states.lock_map().insert(
            "conv".to_string(),
            SessionState::Error {
                message: "fatal".to_string(),
                recoverable: false,
            },
        );
        assert!(!states.can_start_new_turn("conv"));
        assert_eq!(
            states.begin_turn("conv", &"conv:1".to_string()),
            Err(StateError::Busy)
        );
    }

    #[test]
    fn poisoned_mutex_recovers() {
        let states = SessionStates::new();
        let inner = states.0.clone();
        let _ = std::thread::spawn(move || {
            let _guard = inner.lock().expect("lock");
            panic!("poison the state mutex");
        })
        .join();

        states
            .begin_turn("conv", &"conv:1".to_string())
            .expect("recovers after poison");
        assert!(matches!(
            states.get("conv"),
            Some(SessionState::Processing { .. })
        ));
    }
}
