// 参考: stretchly/app/utils/scheduler.js — 以墙钟重算剩余时间，而不是递减计数

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use assistant_protocol::{ClockModeDto, ClockSnapshotDto, ClockStatusDto, Millis};

pub const MIN_DURATION_MS: Millis = 60_000;
pub const MAX_DURATION_MS: Millis = 10_800_000;
pub const DEFAULT_DURATION_MS: Millis = 25 * 60_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClockMode {
    Countdown,
    Stopwatch,
}

impl ClockMode {
    fn as_dto(self) -> ClockModeDto {
        match self {
            ClockMode::Countdown => ClockModeDto::Countdown,
            ClockMode::Stopwatch => ClockModeDto::Stopwatch,
        }
    }
}

impl From<ClockModeDto> for ClockMode {
    fn from(value: ClockModeDto) -> Self {
        match value {
            ClockModeDto::Countdown => ClockMode::Countdown,
            ClockModeDto::Stopwatch => ClockMode::Stopwatch,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClockStatus {
    Idle,
    Running,
    Paused,
    Finished,
}

impl ClockStatus {
    fn as_dto(self) -> ClockStatusDto {
        match self {
            ClockStatus::Idle => ClockStatusDto::Idle,
            ClockStatus::Running => ClockStatusDto::Running,
            ClockStatus::Paused => ClockStatusDto::Paused,
            ClockStatus::Finished => ClockStatusDto::Finished,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockSnapshot {
    pub mode: ClockMode,
    pub status: ClockStatus,
    pub duration_ms: Millis,
    /// 仅倒计时有值；普通计时为 None
    pub remaining_ms: Option<Millis>,
    /// 两种模式都有值；倒计时下等于 duration_ms - remaining_ms
    pub elapsed_ms: Millis,
    pub started_at_ms: Option<Millis>,
    pub elapsed_before_ms: Millis,
}

impl From<ClockSnapshot> for ClockSnapshotDto {
    fn from(value: ClockSnapshot) -> Self {
        Self {
            mode: value.mode.as_dto(),
            status: value.status.as_dto(),
            duration_ms: value.duration_ms,
            remaining_ms: value.remaining_ms,
            elapsed_ms: value.elapsed_ms,
            started_at_ms: value.started_at_ms,
            elapsed_before_ms: value.elapsed_before_ms,
        }
    }
}

struct ClockInner {
    mode: ClockMode,
    status: ClockStatus,
    duration_ms: Millis,
    started_at_ms: Option<Millis>,
    elapsed_before_ms: Millis,
}

impl Default for ClockInner {
    fn default() -> Self {
        Self {
            mode: ClockMode::Countdown,
            status: ClockStatus::Idle,
            duration_ms: DEFAULT_DURATION_MS,
            started_at_ms: None,
            elapsed_before_ms: 0,
        }
    }
}

pub struct ClockRuntime {
    inner: Mutex<ClockInner>,
    generation: AtomicU64,
}

pub fn now_ms() -> Millis {
    chrono::Utc::now().timestamp_millis()
}

impl ClockRuntime {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ClockInner::default()),
            generation: AtomicU64::new(0),
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn lock_inner(&self) -> std::sync::MutexGuard<'_, ClockInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn elapsed_for(inner: &ClockInner, now_ms: Millis) -> Millis {
        let running_delta = if inner.status == ClockStatus::Running {
            inner
                .started_at_ms
                .map(|started| (now_ms - started).max(0))
                .unwrap_or(0)
        } else {
            0
        };
        inner.elapsed_before_ms.saturating_add(running_delta)
    }

    fn snapshot_inner(inner: &ClockInner, now_ms: Millis) -> ClockSnapshot {
        let elapsed = Self::elapsed_for(inner, now_ms);
        let (remaining_ms, elapsed_ms) = match inner.mode {
            ClockMode::Countdown => {
                let remaining = (inner.duration_ms - elapsed).clamp(0, inner.duration_ms);
                (Some(remaining), inner.duration_ms - remaining)
            }
            ClockMode::Stopwatch => (None, elapsed),
        };

        ClockSnapshot {
            mode: inner.mode,
            status: inner.status,
            duration_ms: inner.duration_ms,
            remaining_ms,
            elapsed_ms,
            started_at_ms: inner.started_at_ms,
            elapsed_before_ms: inner.elapsed_before_ms,
        }
    }

    pub fn snapshot(&self, now_ms: Millis) -> ClockSnapshot {
        Self::snapshot_inner(&self.lock_inner(), now_ms)
    }

    pub fn remaining_ms(&self, now_ms: Millis) -> Option<Millis> {
        Self::snapshot_inner(&self.lock_inner(), now_ms).remaining_ms
    }

    /// 切换模式并重置当前轮次；相同模式幂等；模式变化且 Running 时返回错误。
    pub fn set_mode(&self, mode: ClockMode, now_ms: Millis) -> Result<ClockSnapshot, String> {
        let snapshot = {
            let mut inner = self.lock_inner();
            if inner.mode == mode {
                return Ok(Self::snapshot_inner(&inner, now_ms));
            }
            if inner.status == ClockStatus::Running {
                return Err("cannot switch clock mode while running".to_string());
            }
            inner.mode = mode;
            inner.status = ClockStatus::Idle;
            inner.started_at_ms = None;
            inner.elapsed_before_ms = 0;
            Self::snapshot_inner(&inner, now_ms)
        };
        self.bump_generation();
        Ok(snapshot)
    }

    /// 开始或重开一轮计时；倒计时使用传入时长或当前时长，普通计时忽略时长。
    pub fn start(
        &self,
        duration_ms: Option<Millis>,
        now_ms: Millis,
    ) -> Result<(ClockSnapshot, u64), String> {
        let snapshot = {
            let mut inner = self.lock_inner();
            if inner.mode == ClockMode::Countdown {
                let duration = duration_ms.unwrap_or(inner.duration_ms);
                if !(MIN_DURATION_MS..=MAX_DURATION_MS).contains(&duration) {
                    return Err(format!(
                        "duration_ms must be within {MIN_DURATION_MS}..={MAX_DURATION_MS}, got {duration}"
                    ));
                }
                inner.duration_ms = duration;
            }
            inner.status = ClockStatus::Running;
            inner.started_at_ms = Some(now_ms);
            inner.elapsed_before_ms = 0;
            Self::snapshot_inner(&inner, now_ms)
        };
        Ok((snapshot, self.bump_generation()))
    }

    /// Running 时暂停；其它状态为空操作。普通计时不裁剪到倒计时时长。
    pub fn pause(&self, now_ms: Millis) -> ClockSnapshot {
        let mut inner = self.lock_inner();
        if inner.status == ClockStatus::Running {
            let started = inner.started_at_ms.unwrap_or(now_ms);
            let elapsed = inner
                .elapsed_before_ms
                .saturating_add((now_ms - started).max(0));
            inner.elapsed_before_ms = match inner.mode {
                ClockMode::Countdown => elapsed.clamp(0, inner.duration_ms),
                ClockMode::Stopwatch => elapsed,
            };
            inner.started_at_ms = None;
            inner.status = ClockStatus::Paused;
            let snapshot = Self::snapshot_inner(&inner, now_ms);
            drop(inner);
            self.bump_generation();
            return snapshot;
        }
        Self::snapshot_inner(&inner, now_ms)
    }

    /// Paused 时继续；其它状态为空操作。返回值第三项表示是否真的从 Paused 恢复。
    pub fn resume(&self, now_ms: Millis) -> (ClockSnapshot, u64, bool) {
        let (snapshot, resumed) = {
            let mut inner = self.lock_inner();
            let resumed = inner.status == ClockStatus::Paused;
            if resumed {
                inner.started_at_ms = Some(now_ms);
                inner.status = ClockStatus::Running;
            }
            (Self::snapshot_inner(&inner, now_ms), resumed)
        };
        let generation = if resumed {
            self.bump_generation()
        } else {
            self.generation()
        };
        (snapshot, generation, resumed)
    }

    pub fn reset(&self) -> ClockSnapshot {
        let snapshot = {
            let mut inner = self.lock_inner();
            inner.status = ClockStatus::Idle;
            inner.started_at_ms = None;
            inner.elapsed_before_ms = 0;
            Self::snapshot_inner(&inner, now_ms())
        };
        self.bump_generation();
        snapshot
    }

    /// 仅倒计时 Running 时完成一次；普通计时与重复调用返回 None。
    pub fn mark_finished(&self, now_ms: Millis) -> Option<ClockSnapshot> {
        let mut inner = self.lock_inner();
        if inner.mode != ClockMode::Countdown || inner.status != ClockStatus::Running {
            return None;
        }
        inner.status = ClockStatus::Finished;
        inner.elapsed_before_ms = inner.duration_ms;
        inner.started_at_ms = None;
        Some(Self::snapshot_inner(&inner, now_ms))
    }
}

impl Default for ClockRuntime {
    fn default() -> Self {
        Self::new()
    }
}
