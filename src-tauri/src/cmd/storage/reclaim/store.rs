//! 计划缓存 + 单实例串行闸门（§5.2.1）：只持有状态，不做 IO。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::protocol::dto::{ReclaimPolicyDto, StorageReclaimActionDto};

use super::PLAN_TTL_SECONDS;

/// 已校验的条目快照：执行时只认「计划里出现过的 `key + action`」。
///
/// `policy` 是计划时刻的策略快照（§7.3 第 4 条）：配置热更新后执行侧要拿它比对，
/// 不一致就 `skipped`，不按旧策略删。`E2` / `E4` 的策略是静态档，比对恒等；
/// 带配置保留期的 `E3` 在 T3 接入时才真正用得上。
#[derive(Debug, Clone, PartialEq)]
pub struct CachedPlanEntry {
    pub key: String,
    pub action: StorageReclaimActionDto,
    pub policy: ReclaimPolicyDto,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedPlan {
    pub sampled_at: i64,
    pub created_at: i64,
    pub entries: Vec<CachedPlanEntry>,
}

impl CachedPlan {
    pub(super) fn allows(&self, key: &str, action: StorageReclaimActionDto) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.key == key && entry.action == action)
    }

    /// 计划时刻的策略快照：执行侧拿它与 owner 当前策略比对（§7.3 第 4 条）。
    pub(super) fn policy_of(&self, key: &str) -> Option<ReclaimPolicyDto> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.policy.clone())
    }
}

/// 计划查不到 / 已过期：两者都零副作用，错误码区分以便前端提示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanLookupError {
    NotFound,
    Expired,
}

impl PlanLookupError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "planNotFound",
            Self::Expired => "planExpired",
        }
    }
}

/// 计划缓存 + 单实例串行闸门：两者合一，保证「同一时刻只有一个执行在动文件」。
#[derive(Default)]
pub struct ReclaimPlanStore {
    inner: Mutex<HashMap<String, CachedPlan>>,
    executing: AtomicBool,
}

impl ReclaimPlanStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记计划并回收过期项（顺手裁剪，不做定时任务）。
    pub fn store(&self, plan: CachedPlan) -> String {
        let plan_id = uuid::Uuid::new_v4().to_string();
        let mut plans = self.lock();
        let now = plan.created_at;
        plans.retain(|_, cached| plan_age(now, cached.created_at) <= PLAN_TTL_SECONDS);
        plans.insert(plan_id.clone(), plan);
        plan_id
    }

    pub fn lookup(&self, plan_id: &str, now: i64) -> Result<CachedPlan, PlanLookupError> {
        let cached = self
            .lock()
            .get(plan_id)
            .cloned()
            .ok_or(PlanLookupError::NotFound)?;
        if plan_age(now, cached.created_at) > PLAN_TTL_SECONDS {
            self.lock().remove(plan_id);
            return Err(PlanLookupError::Expired);
        }
        Ok(cached)
    }

    /// 抢串行闸门；已有执行在跑时返回 `None`（命令层据此回 `busy`）。
    pub fn begin_execution(&self) -> Option<ExecutionGuard<'_>> {
        self.executing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| ExecutionGuard {
                flag: &self.executing,
            })
    }

    /// 毒锁不该让整个存储页失效：只取回内部数据，继续用。
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, CachedPlan>> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

fn plan_age(now: i64, created_at: i64) -> i64 {
    now.saturating_sub(created_at)
}

/// 执行期间持有；`Drop` 一定放闸（含提前 `?` 返回的路径）。
pub struct ExecutionGuard<'a> {
    flag: &'a AtomicBool,
}

impl Drop for ExecutionGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}
