// 参考: docs/store/I75-startup-layering-spec.md §4.4 — single-flight 装配与状态机

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use assistant_protocol::startup::{CapStateDto, CapabilityStateDto};
use tauri::{AppHandle, Manager};

use crate::boot::ctx::{BootClock, StartupCtx, StartupReport};
use crate::boot::resources::{CapErr, CapValue, Res};

enum Slot {
    Pending,
    Ready(CapValue),
    Failed(CapErr),
}

/// single-flight 槽。`gate` 串行化装配，`slots` 只在状态迁移时短暂持有，
/// 因此 `build` 内可以安全地经 `ctx` 读前序槽（不会自锁）。
pub struct Registry {
    slots: Mutex<HashMap<&'static str, Slot>>,
    gate: Mutex<()>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
            gate: Mutex::new(()),
        }
    }

    fn slots(&self) -> MutexGuard<'_, HashMap<&'static str, Slot>> {
        self.slots.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 由 `run_boot` 注入进程级产物（`dirs`）：不经 `build`，但同样占槽。
    pub fn inject(&self, id: &'static str, value: CapValue) {
        self.slots().insert(id, Slot::Ready(value));
    }

    pub fn get(&self, id: &str) -> Option<CapValue> {
        match self.slots().get(id) {
            Some(Slot::Ready(value)) => Some(value.clone()),
            _ => None,
        }
    }

    /// 全表状态快照（表序）：状态查询命令与首屏水合共用，未触发过的按需项为 `Pending`。
    pub fn snapshot(&self) -> Vec<CapabilityStateDto> {
        let slots = self.slots();
        crate::boot::resources::RESOURCES
            .iter()
            .map(|res| {
                let (state, message) = match slots.get(res.id) {
                    Some(Slot::Ready(_)) => (CapStateDto::Ready, None),
                    Some(Slot::Failed(err)) => (CapStateDto::Failed, failure_message(err)),
                    _ => (CapStateDto::Pending, None),
                };
                CapabilityStateDto {
                    id: res.id.to_string(),
                    state,
                    message,
                }
            })
            .collect()
    }

    /// 幂等装配：已 Ready 直接返回；`retryable` 的失败允许下一次重试。
    pub fn build(&self, ctx: &StartupCtx<'_>, res: &'static Res) -> Result<CapValue, CapErr> {
        let _gate = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        match self.slots().get(res.id) {
            Some(Slot::Ready(value)) => return Ok(value.clone()),
            Some(Slot::Failed(err)) if !err.retryable() => return Err(err.clone()),
            _ => {}
        }
        self.slots().insert(res.id, Slot::Pending);
        emit(ctx, res.id, CapStateDto::Pending, None);
        let result = (res.build)(ctx);
        match &result {
            Ok(value) => {
                self.slots().insert(res.id, Slot::Ready(value.clone()));
                emit(ctx, res.id, CapStateDto::Ready, None);
            }
            Err(err) => {
                self.slots().insert(res.id, Slot::Failed(err.clone()));
                // 失败原因只在这里记一次：装配入口唯一，boot / idle / `ensure` / 重试
                // 四条路径因此都带原因，`runner` 不再重复记同一失败。
                match crate::boot::resources::severity(res.id) {
                    crate::boot::resources::Severity::Fatal => {
                        log::error!("capability {} build failed: {err}", res.id);
                    }
                    crate::boot::resources::Severity::Degraded => {
                        // 降档项在任何入口失败都要记一次：`ensure` 重试路径不经过 runner。
                        log::warn!("capability {} unavailable: {err}", res.id);
                    }
                    crate::boot::resources::Severity::Ignore => {
                        log::warn!("capability {} build failed (ignored): {err}", res.id);
                    }
                }
                emit(ctx, res.id, CapStateDto::Failed, failure_message(err));
            }
        }
        result
    }
}

fn failure_message(err: &CapErr) -> Option<String> {
    match err {
        CapErr::Build { message, .. } => Some(message.clone()),
        CapErr::NotReady { .. } => None,
    }
}

/// 状态变更广播（C4）：只在真实迁移处发，订阅方据此刷新「正在准备 / 失败原因 + 重试」。
fn emit(ctx: &StartupCtx<'_>, id: &str, state: CapStateDto, message: Option<String>) {
    crate::protocol::events::emit_broadcast(
        ctx.app,
        crate::protocol::events::event_name::STARTUP_CAPABILITY_CHANGED,
        CapabilityStateDto {
            id: id.to_string(),
            state,
            message,
        },
    );
}

/// 唯一装配入口：命令层 / 适配层 / idle pass 都调这一份，不存在第二套实现。
pub async fn ensure(app: &AppHandle, id: &'static str) -> Result<CapValue, CapErr> {
    let res = crate::boot::resources::find(id).ok_or(CapErr::not_ready(id))?;
    let registry = app.state::<Registry>();
    let clock = app.state::<BootClock>();
    let report = app.state::<StartupReport>();
    let ctx = StartupCtx::new(app, &clock, &report, &registry);
    registry.build(&ctx, res)
}

/// `run_idle` 与首次使用共用同一份装配路径，这里只按 id 遍历 Idle 泳道。
pub async fn run_idle(app: &AppHandle) {
    let registry = app.state::<Registry>();
    let clock = app.state::<BootClock>();
    let report = app.state::<StartupReport>();
    let ctx = StartupCtx::new(app, &clock, &report, &registry);
    crate::boot::runner::run_idle(&ctx);
}
