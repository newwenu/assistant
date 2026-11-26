// 参考: docs/store/I75-startup-layering-spec.md §4.2 — 启动上下文与阶段记录

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use crate::app_dirs::AppDirs;
use crate::boot::conf::BootConf;
use crate::boot::registry::Registry;
use crate::boot::resources::{CapErr, CapValue, Lane, Res};

/// 阶段结局。由调用点显式声明，不由失败本身推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Degraded,
    Fatal,
    Ignored,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Degraded => "degraded",
            Outcome::Fatal => "fatal",
            Outcome::Ignored => "ignored",
        }
    }
}

/// 进程级时钟。`t0` 与 `t0_epoch_ms` 必须同点采集，否则跨时钟域误差会污染首帧指标。
pub struct BootClock {
    startup_id: String,
    t0: Instant,
    t0_epoch_ms: i64,
    last: Mutex<Instant>,
}

impl BootClock {
    pub fn start() -> Self {
        let (t0, t0_epoch_ms) = (Instant::now(), epoch_ms());
        Self {
            startup_id: uuid::Uuid::new_v4().to_string(),
            t0,
            t0_epoch_ms,
            last: Mutex::new(t0),
        }
    }

    pub fn startup_id(&self) -> &str {
        &self.startup_id
    }

    pub fn t0_epoch_ms(&self) -> i64 {
        self.t0_epoch_ms
    }

    /// 进程累计耗时（内部诊断）：只供阶段行打印，不进结构、不出命令。
    pub(crate) fn process_ms(&self) -> u64 {
        millis(self.t0.elapsed())
    }

    /// 距上一阶段结束的空隙（内部诊断）：区分「阶段本身慢」与「阶段之间有空隙」。
    pub(crate) fn since_last_ms(&self) -> u64 {
        let last = *self.last.lock().unwrap_or_else(|e| e.into_inner());
        millis(last.elapsed())
    }

    fn touch(&self) {
        *self.last.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }
}

pub(crate) fn epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// 启动报告：只记「哪些阶段已跑过」，供 `run_idle` 去重；首帧与阶段毫秒只进日志。
///
/// 不存阶段毫秒与降档集合：同一份数据既进日志又存结构，就必须有第二个读者，
/// 否则只是给无人调用的查询命令供料，反而制造出两份可能不一致的事实。
pub struct StartupReport {
    phases: Mutex<HashSet<&'static str>>,
    /// 唤起轮次起点：`wake-<uuid>` → 入口 epoch 毫秒。首帧回传时取出并相减即得 T_wake。
    wake_starts: Mutex<HashMap<String, i64>>,
}

impl Default for StartupReport {
    fn default() -> Self {
        Self {
            phases: Mutex::new(HashSet::new()),
            wake_starts: Mutex::new(HashMap::new()),
        }
    }
}

impl StartupReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// 计时一个可能失败的阶段，并把原 `Result` 交还调用点。
    ///
    /// `on_err` 由调用点声明本阶段的失败语义：`Fatal` 表示失败会终止启动，
    /// `Degraded` 表示失败被吞掉但计入降档。
    pub fn stage<T, E: std::fmt::Display>(
        &self,
        clock: &BootClock,
        id: &'static str,
        lane: Lane,
        on_err: Outcome,
        body: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let started = Instant::now();
        let result = body();
        let outcome = if result.is_ok() { Outcome::Ok } else { on_err };
        self.commit(clock, id, lane, started, outcome);
        result
    }

    /// 计时一个失败不影响启动的阶段：失败时计入降档，不向调用点传播。
    ///
    /// 成功记 `ok`：`ignored` 专指 `BEST_EFFORT` 项失败，两类结局不可共用同一取值，
    /// 否则健康启动里每个 `Degraded` 项都会显示成失败。
    ///
    /// 失败原因由 `Registry::build` 记（装配入口唯一，覆盖 boot / idle / `ensure` / 重试），
    /// 本函数只落阶段行，避免同一失败记两次。
    pub fn stage_ignored<E: std::fmt::Display>(
        &self,
        clock: &BootClock,
        id: &'static str,
        lane: Lane,
        body: impl FnOnce() -> Result<(), E>,
    ) {
        let started = Instant::now();
        let outcome = if body().is_ok() {
            Outcome::Ok
        } else {
            Outcome::Degraded
        };
        self.commit(clock, id, lane, started, outcome);
    }

    /// 尽力而为：失败记 `ignored`，原因同样由 `Registry::build` 记。
    pub fn stage_best_effort<E: std::fmt::Display>(
        &self,
        clock: &BootClock,
        id: &'static str,
        lane: Lane,
        body: impl FnOnce() -> Result<(), E>,
    ) {
        let started = Instant::now();
        let outcome = if body().is_ok() {
            Outcome::Ok
        } else {
            Outcome::Ignored
        };
        self.commit(clock, id, lane, started, outcome);
    }

    /// 首帧回传：只落一条 `info` 锚点，前端算好的 epoch 毫秒与 `t0_epoch_ms` 相减。
    pub fn record_first_frame(&self, clock: &BootClock, frame_epoch_ms: i64) {
        let visible_ms =
            u64::try_from(frame_epoch_ms.saturating_sub(clock.t0_epoch_ms())).unwrap_or(0);
        log::info!(
            "startup first_frame startup_id={} visible_ms={}",
            clock.startup_id(),
            visible_ms
        );
    }

    /// 唤起入口打点：生成 id 的同一时刻记起点，首帧回传时相减即 T_wake。
    pub fn record_wake_start(&self, id: &str, epoch_ms: i64) {
        self.wake_starts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.to_string(), epoch_ms);
    }

    /// 取出唤起起点：同一 id 只量一次，避免重复回传重复计算。
    pub fn take_wake_start(&self, id: &str) -> Option<i64> {
        self.wake_starts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id)
    }

    /// 阶段是否已有记录：`run_idle` 用它跳过 `ensure` 已提前装配的项。
    pub fn has_phase(&self, id: &str) -> bool {
        self.phases
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
    }

    fn commit(
        &self,
        clock: &BootClock,
        id: &'static str,
        lane: Lane,
        started: Instant,
        outcome: Outcome,
    ) {
        let ms = millis(started.elapsed());
        // 空隙必须在 `touch` 之前取：`touch` 把基准移到本阶段结束点。
        let gap_ms = clock.since_last_ms();
        clock.touch();
        let process_ms = clock.process_ms();
        // 单阶段行只给 dev：release 档日志级别是 `Info`，`debug` 自然静默。
        // 三个毫秒各有分工：`ms` 是本阶段耗时，`gap_ms` 是距上一阶段结束的空隙，
        // `process_ms` 是从进程起点累计——只有同时看才能区分「阶段慢」与「阶段间空转」。
        // 它们只进这一行，不另存结构：没有读者的数据只是第二份会漂的事实。
        log::debug!(
            "startup stage={} lane={} startup_id={} ms={} gap_ms={} process_ms={} outcome={}",
            id,
            lane.label(),
            clock.startup_id(),
            ms,
            gap_ms,
            process_ms,
            outcome.label()
        );
        self.phases
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id);
    }
}

/// 装配期上下文：前序产物只经槽取，不存在第二个全局访问点。
pub struct StartupCtx<'a> {
    pub app: &'a AppHandle,
    pub clock: &'a BootClock,
    pub report: &'a StartupReport,
    registry: &'a Registry,
}

impl<'a> StartupCtx<'a> {
    pub fn new(
        app: &'a AppHandle,
        clock: &'a BootClock,
        report: &'a StartupReport,
        registry: &'a Registry,
    ) -> Self {
        Self {
            app,
            clock,
            report,
            registry,
        }
    }

    pub fn dirs(&self) -> Result<Arc<AppDirs>, CapErr> {
        self.get::<AppDirs>("dirs")
    }

    pub fn boot(&self) -> Result<Arc<BootConf>, CapErr> {
        self.get::<BootConf>("boot_conf")
    }

    /// 从槽还原一个已装配产物；未装配返回 `NotReady`，不 panic、不隐式装配。
    pub fn get<T: Any + Send + Sync>(&self, id: &'static str) -> Result<Arc<T>, CapErr> {
        let value = self.registry.get(id).ok_or(CapErr::not_ready(id))?;
        value
            .downcast::<T>()
            .map_err(|_| CapErr::build(id, "capability type mismatch"))
    }

    pub fn build(&self, res: &'static Res) -> Result<CapValue, CapErr> {
        self.registry.build(self, res)
    }

    /// 登记一个由别的资源顺带产出的槽（如表项 `tool_runtime` 产出的执行器端口）。
    pub fn inject(&self, id: &'static str, value: CapValue) {
        self.registry.inject(id, value);
    }
}
