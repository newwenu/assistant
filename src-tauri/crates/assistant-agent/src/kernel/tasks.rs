use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use tokio_util::sync::CancellationToken;

use super::ids::TurnId;
use super::OnBusy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Busy;

impl std::fmt::Display for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session busy")
    }
}

impl std::error::Error for Busy {}

pub struct TurnEntry {
    pub token: CancellationToken,
    pub counter: u32,
    pub turn_id: TurnId,
}

pub struct TurnTasks(Mutex<HashMap<String, TurnEntry>>);

impl Default for TurnTasks {
    fn default() -> Self {
        Self::new()
    }
}

impl TurnTasks {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    fn lock_map(&self) -> MutexGuard<'_, HashMap<String, TurnEntry>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("TurnTasks mutex poisoned, recovering");
                poisoned.into_inner()
            }
        }
    }

    /// `Replace`：取消旧令牌、计数 +1、返回新 turn（chat 现状）。
    /// `Reject`：返回 `Busy`，旧令牌原样继续跑（agent）；旧代际已取消时按 `Replace` 顶替。
    pub fn begin(
        &self,
        conversation_id: &str,
        on_busy: OnBusy,
    ) -> Result<(TurnId, CancellationToken), Busy> {
        let mut map = self.lock_map();
        let token = CancellationToken::new();
        let next_counter = match map.get(conversation_id) {
            Some(existing) => {
                if on_busy == OnBusy::Reject && !existing.token.is_cancelled() {
                    return Err(Busy);
                }
                existing.token.cancel();
                existing.counter + 1
            }
            None => 1,
        };
        let turn_id = format!("{}:{}", conversation_id, next_counter);
        map.insert(
            conversation_id.to_string(),
            TurnEntry {
                token: token.clone(),
                counter: next_counter,
                turn_id: turn_id.clone(),
            },
        );
        Ok((turn_id, token))
    }

    /// 只置位令牌，不 `abort` 任务。
    pub fn cancel(&self, conversation_id: &str) -> bool {
        match self.lock_map().get(conversation_id) {
            Some(entry) => {
                entry.token.cancel();
                true
            }
            None => false,
        }
    }

    /// 恒校验代际：传旧 `turn_id` 不生效，避免旧轮迟到完成清掉新轮。
    pub fn finish(&self, conversation_id: &str, turn_id: &TurnId) {
        let mut map = self.lock_map();
        let current = map
            .get(conversation_id)
            .map(|entry| entry.turn_id == *turn_id)
            .unwrap_or(false);
        if !current {
            log::debug!(
                "finish ignored: conversation={} turn={} is not the current generation",
                conversation_id,
                turn_id
            );
            return;
        }
        map.remove(conversation_id);
    }

    pub fn current(&self, conversation_id: &str) -> Option<TurnId> {
        self.lock_map()
            .get(conversation_id)
            .map(|entry| entry.turn_id.clone())
    }

    pub fn is_current(&self, conversation_id: &str, turn_id: &TurnId) -> bool {
        self.current(conversation_id)
            .map(|current| current == *turn_id)
            .unwrap_or(false)
    }

    /// 互斥判定：存在当前代际且令牌未取消。取消意图一旦置位即视为解除忙，
    /// 无需等待收尾完成（I41 §2）。
    pub fn is_busy(&self, conversation_id: &str) -> bool {
        self.lock_map()
            .get(conversation_id)
            .map(|entry| !entry.token.is_cancelled())
            .unwrap_or(false)
    }

    pub fn cancel_token(&self, conversation_id: &str) -> Option<CancellationToken> {
        self.lock_map()
            .get(conversation_id)
            .map(|entry| entry.token.clone())
    }

    /// 子令牌用于模型流 / 工具执行 / 审批等待；父令牌取消时一并失效。
    pub fn child_token(token: &CancellationToken) -> CancellationToken {
        token.child_token()
    }
}
