//! 条目统计缓存与后台补算（I79 §5.2 / §6.1）：读路径零等待，补算永远在后台、single-flight。
//!
//! 四条纪律：①缓存**只由 worker 写**——`read` 纯只读（INV6），计划命令既不读也不写（INV3）；
//! ②取数实现唯一——worker 只调 `rows` / `sections` 既有入口，本文件不写第二套遍历（§6.1 约束 1）；
//! ③闭包自结算——`finish` / `fail` / 清 `running` / 释放 permit 全在任务内部完成，
//! 超时只做 UI 占位，结果永不丢弃（INV12）；④结算权唯一——`begin` 分配 token，
//! token 不匹配的旧结果直接丢弃（INV11），租约兜底避免卡死任务永久占住并发槽（§6.1 约束 5）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tauri::{AppHandle, Manager};

use assistant_storage::attachment_store::AttachmentStore;
use assistant_tools::{ToolResultStorage, ToolScratchStore};

use crate::app_dirs::registry::{CoreEntry, StatsMode};
use crate::app_dirs::AppDirs;
use crate::cmd::storage::entries::{find_by_key, views, EntrySample, EntryView};
use crate::cmd::storage::section::{SectionRegistry, StorageSectionHandle};
use crate::db_state::DbState;
use crate::protocol::dto::StorageAvailabilityDto;
use crate::protocol::events::{emit_broadcast, event_name};

use super::rows::{
    derived_sample, sample_config_file, sample_crashes, sample_database, sample_index, sample_logs,
    sample_mascot, sample_scratch, sample_tool_results, sample_webview,
};
use super::sections::section_sample;
use super::{
    ENTRY_STATS_JOB_TIMEOUT_MS, ENTRY_STATS_LEASE_SECONDS, ENTRY_STATS_MAX_INFLIGHT,
    ENTRY_STATS_RETRY_SECONDS, ENTRY_STATS_TTL_SECONDS,
};

/// 事件合并窗口（I79 §6.2 / INV7）：一个窗口内完成 / 失败的条目合并为一次事件。
const ENTRY_STATS_EVENT_DRAIN_MS: u64 = 250;

/// 单条缓存记录（I79 §5.2）：`observed_at` 是「最近一次尝试时刻」，成功与失败都推进它。
///
/// **只存观测**：`policy` / `eligibility` 这类判据不进缓存。判据的输入（锁态、保留天数、
/// 上限）随时会变，读路径按分区 / owner 的**当前**接口现算（`sample/sections.rs` 的
/// `refresh_judgement`、`sample/rows.rs` 的 `refresh_tool_results_policy`）。
/// 因此本缓存不需要任何「功能侧通知失效」的钩子，core 也不认识分区语义（G17(b)）。
#[derive(Debug, Clone)]
struct EntryStat {
    observed_at: i64,
    availability: StorageAvailabilityDto,
    bytes: Option<u64>,
    count: Option<u64>,
    oldest_at: Option<i64>,
    newest_at: Option<i64>,
    reclaimable: Option<(u64, u64)>,
    /// 单飞 + 租约：`(token, started_at)`；`None` 表示无任务在跑（§6.1 约束 5）。
    running: Option<(u64, i64)>,
}

impl EntryStat {
    fn fresh(now: i64) -> Self {
        Self {
            observed_at: now,
            availability: StorageAvailabilityDto::NoReportYet,
            bytes: None,
            count: None,
            oldest_at: None,
            newest_at: None,
            reclaimable: None,
            running: None,
        }
    }

    fn is_success(&self) -> bool {
        matches!(
            self.availability,
            StorageAvailabilityDto::Available | StorageAvailabilityDto::Missing
        )
    }

    /// 算过但没结论且**重试无意义**：只有 `notImplemented`（实现不存在，再算也一样）。
    ///
    /// 其余失败类别一律按退避重试（I79 §5.2）——包括 `locked`：旧设计把它当终态、靠
    /// 「解锁事件」通知缓存失效；现在判据由读路径现算（`sample/sections.rs` 的
    /// `refresh_judgement`），缓存不承载任何解锁通知，锁一解除下一次读就能拿到数字。
    fn is_terminal(&self) -> bool {
        matches!(self.availability, StorageAvailabilityDto::NotImplemented)
    }

    fn sample_with_values(&self, availability: StorageAvailabilityDto) -> EntrySample {
        EntrySample {
            bytes: self.bytes,
            count: self.count,
            oldest_at: self.oldest_at,
            newest_at: self.newest_at,
            availability,
            // 判据不缓存：读路径现算后填（`refresh_judgement` / `refresh_tool_results_policy`）。
            policy: None,
            eligibility: None,
            reclaimable: self.reclaimable,
            observed_at: Some(self.observed_at),
        }
    }

    /// 读映射（I79 §5.2 表）：返回 `(样本, 是否触发补算)`。
    fn to_sample(&self, now: i64) -> (EntrySample, bool) {
        let age = now.saturating_sub(self.observed_at);
        let in_flight = self.running.is_some();
        if self.availability == StorageAvailabilityDto::NoReportYet {
            // 正在补算且无成功值：不重复触发（已 in-flight）。
            return (
                EntrySample::degraded(StorageAvailabilityDto::NoReportYet),
                !in_flight,
            );
        }
        if self.is_terminal() {
            return (EntrySample::degraded(self.availability), false);
        }
        if self.is_success() {
            let availability = if age >= ENTRY_STATS_TTL_SECONDS {
                StorageAvailabilityDto::StaleReport
            } else {
                self.availability
            };
            // `staleReport` 只替换可用性取值，不改数字（§5.2 约束 1）。
            let trigger = !in_flight && age >= ENTRY_STATS_TTL_SECONDS;
            return (self.sample_with_values(availability), trigger);
        }
        // 失败：不覆盖上一次成功数字，整条 `null` + 失败类别（§5.2 约束 2）。
        let trigger = !in_flight && age >= ENTRY_STATS_RETRY_SECONDS;
        (EntrySample::degraded(self.availability), trigger)
    }
}

#[derive(Debug, Default)]
struct CacheState {
    stats: HashMap<&'static str, EntryStat>,
    next_token: u64,
}

/// 条目统计缓存（I79 §5.2 / §6.1）：`Arc<Mutex<State>>` + 触发即后台，与 `InventoryCache` 同范式。
#[derive(Clone)]
pub struct EntryStatsCache {
    inner: Arc<Mutex<CacheState>>,
    /// 事件合并（INV7）：已排了一次待发事件时不再排新的。
    flush_scheduled: Arc<AtomicBool>,
    /// 全局并发上限（§6.1 约束 3）：跨多次 `trigger_stats` 共享，避免多次整页重拉叠加遍历。
    inflight: Arc<tokio::sync::Semaphore>,
}

impl Default for EntryStatsCache {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CacheState::default())),
            flush_scheduled: Arc::new(AtomicBool::new(false)),
            inflight: Arc::new(tokio::sync::Semaphore::new(ENTRY_STATS_MAX_INFLIGHT)),
        }
    }
}

impl EntryStatsCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 后台任务的并发槽（全局共享，permit 在任务闭包结束时释放）。
    fn inflight(&self) -> Arc<tokio::sync::Semaphore> {
        self.inflight.clone()
    }

    fn lock(&self) -> MutexGuard<'_, CacheState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 只读：按缓存状态映射 `Deferred` 条目，返回 `(samples, 需要补算的 key)`。
    ///
    /// `samples` 与 `views` 一一对齐：`Inline` 槽位留占位，由现场采样覆写（INV6：不写状态）。
    pub fn read(&self, now: i64, views: &[EntryView]) -> (Vec<EntrySample>, Vec<&'static str>) {
        let state = self.lock();
        let mut samples = Vec::with_capacity(views.len());
        let mut triggers = Vec::new();
        for view in views {
            if view.stats_mode != StatsMode::Deferred {
                samples.push(EntrySample::missing());
                continue;
            }
            match state.stats.get(view.key) {
                Some(stat) => {
                    let (sample, trigger) = stat.to_sample(now);
                    if trigger {
                        triggers.push(view.key);
                    }
                    samples.push(sample);
                }
                None => {
                    samples.push(EntrySample::degraded(StorageAvailabilityDto::NoReportYet));
                    triggers.push(view.key);
                }
            }
        }
        (samples, triggers)
    }

    /// single-flight + 租约：任务仍在跑且未超租约时返回 `None`（合并，不排队）。
    fn begin(&self, key: &'static str, now: i64) -> Option<u64> {
        let mut state = self.lock();
        state.next_token = state.next_token.wrapping_add(1);
        let token = state.next_token;
        let stat = state
            .stats
            .entry(key)
            .or_insert_with(|| EntryStat::fresh(now));
        if let Some((_, started_at)) = stat.running {
            if now.saturating_sub(started_at) < ENTRY_STATS_LEASE_SECONDS {
                return None;
            }
            log::warn!("条目 {key} 的后台统计超过租约 {ENTRY_STATS_LEASE_SECONDS}s，允许重试");
        }
        stat.running = Some((token, now));
        Some(token)
    }

    /// 结算：token 不匹配（已被 `invalidate` 或新任务接管）则丢弃，不覆盖新值（INV11）。
    fn finish(&self, key: &'static str, token: u64, sample: EntrySample, at: i64) {
        let mut state = self.lock();
        let Some(stat) = state.stats.get_mut(key) else {
            return;
        };
        if stat.running.map(|(running, _)| running) != Some(token) {
            return;
        }
        stat.running = None;
        stat.observed_at = at;
        stat.availability = sample.availability;
        stat.bytes = sample.bytes;
        stat.count = sample.count;
        stat.oldest_at = sample.oldest_at;
        stat.newest_at = sample.newest_at;
        stat.reclaimable = sample.reclaimable;
    }

    /// 失败：整条替换为 `null` + 失败类别，旧数字不再展示（§5.2 约束 2）。
    fn fail(&self, key: &'static str, token: u64, availability: StorageAvailabilityDto, at: i64) {
        let mut state = self.lock();
        let Some(stat) = state.stats.get_mut(key) else {
            return;
        };
        if stat.running.map(|(running, _)| running) != Some(token) {
            return;
        }
        stat.running = None;
        stat.observed_at = at;
        stat.availability = availability;
        stat.bytes = None;
        stat.count = None;
        stat.oldest_at = None;
        stat.newest_at = None;
        stat.reclaimable = None;
    }

    /// UI 占位（§5.2 约束 4）：任务超过 `ENTRY_STATS_JOB_TIMEOUT_MS` 先写 `unavailable`，
    /// 让页面脱离「统计中」；同一任务跑完后由同 token 的 `finish` 覆盖，`running` 保持不变。
    fn placeholder_timeout(&self, key: &'static str, token: u64, at: i64) {
        let mut state = self.lock();
        let Some(stat) = state.stats.get_mut(key) else {
            return;
        };
        if stat.running.map(|(running, _)| running) != Some(token) {
            return;
        }
        stat.observed_at = at;
        stat.availability = StorageAvailabilityDto::Unavailable;
        stat.bytes = None;
        stat.count = None;
        stat.oldest_at = None;
        stat.newest_at = None;
        stat.reclaimable = None;
    }

    /// 按条目失效（I79 §8.5）：删记录即回到「无记录」= `noReportYet` + 下次读触发补算；
    /// 在跑任务的 token 随之失效，结果丢弃（INV11）。
    pub fn invalidate(&self, keys: &[&str]) {
        let mut state = self.lock();
        for key in keys {
            state.stats.remove(*key);
        }
    }

    fn schedule_event(&self, app: &AppHandle) {
        if self.flush_scheduled.swap(true, Ordering::SeqCst) {
            return;
        }
        let handle = app.clone();
        let cache = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(ENTRY_STATS_EVENT_DRAIN_MS)).await;
            cache.flush_scheduled.store(false, Ordering::SeqCst);
            emit_broadcast(&handle, event_name::STORAGE_ENTRY_STATS_UPDATED, ());
        });
    }
}

/// 后台取数所需的句柄：**触发时**取一次，与 `inventory::trigger_scan` 同范式（§6.1 约束 2）。
#[derive(Clone)]
struct SampleSources {
    dirs: AppDirs,
    store: AttachmentStore,
    sections: Vec<StorageSectionHandle>,
    tool_results: Option<Arc<dyn ToolResultStorage>>,
    scratch: Option<Arc<dyn ToolScratchStore>>,
    db: Option<DbState>,
}

impl SampleSources {
    fn load(app: &AppHandle) -> Option<Self> {
        let dirs = match AppDirs::resolve() {
            Ok(dirs) => dirs,
            Err(error) => {
                log::warn!("条目统计补算跳过：基根解析失败 {error}");
                return None;
            }
        };
        let store = match app.try_state::<AttachmentStore>() {
            Some(state) => state.inner().clone(),
            None => {
                log::warn!("条目统计补算跳过：附件仓库未装配");
                return None;
            }
        };
        let sections = app
            .try_state::<Arc<SectionRegistry>>()
            .map(|state| state.inner().snapshot())
            .unwrap_or_default();
        Some(Self {
            dirs,
            store,
            sections,
            tool_results: crate::boot::tool_exec(app)
                .ok()
                .map(|exec| exec.storage().clone()),
            scratch: crate::boot::tool_scratch(app).ok(),
            db: app
                .try_state::<DbState>()
                .map(|state| state.inner().clone()),
        })
    }
}

/// 触发后台补算（幂等，I79 §5.1 第 4 步）：只为 `begin` 成功且仍在清单内的 key 起任务。
pub fn trigger_stats(app: &AppHandle, keys: &[&'static str]) {
    if keys.is_empty() {
        return;
    }
    let cache = match app.try_state::<EntryStatsCache>() {
        Some(state) => state.inner().clone(),
        None => return,
    };
    let keys = keys.to_vec();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(sources) = SampleSources::load(&handle) else {
            return;
        };
        // 清单按触发时刻的分区快照过滤：已卸载分区的 key 不主动清理，只是不再补算（§8.5）。
        let views = views(&sources.sections);
        let now = now_seconds();
        let semaphore = cache.inflight();
        for key in keys {
            let Some(view) = find_by_key(&views, key).cloned() else {
                continue;
            };
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => return,
            };
            let Some(token) = cache.begin(key, now) else {
                drop(permit);
                continue;
            };
            spawn_entry_task(&handle, &cache, view, sources.clone(), token, permit);
        }
    });
}

/// 单条补算任务：外层 `timeout` 只做 UI 占位，结算与 permit 释放都在任务内部（INV12）。
fn spawn_entry_task(
    app: &AppHandle,
    cache: &EntryStatsCache,
    view: EntryView,
    sources: SampleSources,
    token: u64,
    permit: tokio::sync::OwnedSemaphorePermit,
) {
    let key = view.key;
    let work_cache = cache.clone();
    let work_view = view;
    let work_sources = sources.clone();
    let mut work = tauri::async_runtime::spawn(async move {
        let _permit = permit;
        let sample = sample_entry(&work_view, &work_sources).await;
        work_cache.finish(key, token, sample.clone(), now_seconds());
        sample
    });
    let app = app.clone();
    let cache = cache.clone();
    tauri::async_runtime::spawn(async move {
        let emit = match tokio::time::timeout(
            Duration::from_millis(ENTRY_STATS_JOB_TIMEOUT_MS),
            &mut work,
        )
        .await
        {
            Ok(Ok(sample)) => !is_terminal_availability(sample.availability),
            Ok(Err(error)) => {
                log::error!("条目 {key} 的后台统计任务异常: {error}");
                cache.fail(
                    key,
                    token,
                    StorageAvailabilityDto::Unreadable,
                    now_seconds(),
                );
                true
            }
            // 超时占位是临时态：前端需要一次重拉才能看到「统计超时」。
            Err(_) => {
                cache.placeholder_timeout(key, token, now_seconds());
                true
            }
        };
        // `work` 被 drop 只是放弃等待（`spawn_blocking` 不可 abort），任务自己会结算。
        drop(work);
        // `notImplemented` 不触发补算、也不发事件（§6.2）。
        if emit {
            cache.schedule_event(&app);
        }
    });
}

/// 重试无意义的终态可用性（§5.2）：只有 `notImplemented`。`locked` 会按退避重试，
/// 其可用性变化仍要让前端知道（锁态解锁后数字会变），所以不算「不必重拉」。
fn is_terminal_availability(availability: StorageAvailabilityDto) -> bool {
    matches!(availability, StorageAvailabilityDto::NotImplemented)
}

/// 取数实现唯一（§6.1 约束 1）：core 按 `view.core` 穷尽匹配、分区按 `view.section` 路由。
async fn sample_entry(view: &EntryView, sources: &SampleSources) -> EntrySample {
    let entry = view.entry;
    match view.core {
        // `E2` 的 owner 入口本身是 async（`list_derived` 内部自枚举），不必搬进阻塞池。
        Some(CoreEntry::AttachmentsDerived) => match sources.store.list_derived().await {
            Ok(entries) => derived_sample(&entries),
            Err(error) => {
                log::debug!("派生侧车统计失败: {error}");
                EntrySample::degraded(StorageAvailabilityDto::Unreadable)
            }
        },
        Some(core) => {
            let dirs = sources.dirs.clone();
            let storage = sources.tool_results.clone();
            let scratch = sources.scratch.clone();
            let db = sources.db.clone();
            match tauri::async_runtime::spawn_blocking(move || {
                sample_core(core, &dirs, &storage, &scratch, &db)
            })
            .await
            {
                Ok(sample) => sample,
                Err(error) => {
                    log::error!("条目 {entry} 的后台统计任务失败: {error}");
                    EntrySample::degraded(StorageAvailabilityDto::Unreadable)
                }
            }
        }
        None => match view
            .section
            .and_then(|index| sources.sections.get(index))
            .cloned()
        {
            Some(section) => {
                let key = view.key;
                match tauri::async_runtime::spawn_blocking(move || section_sample(&section, key))
                    .await
                {
                    Ok(sample) => sample,
                    Err(error) => {
                        log::error!("条目 {entry} 的后台统计任务失败: {error}");
                        EntrySample::degraded(StorageAvailabilityDto::Unreadable)
                    }
                }
            }
            // 分区在触发后被卸载：行已不存在，按 `unavailable` 结算即可。
            None => EntrySample::degraded(StorageAvailabilityDto::Unavailable),
        },
    }
}

/// core 条目的阻塞取数：与 `collect` 的现场采样同一批入口（不另写遍历）。
fn sample_core(
    core: CoreEntry,
    dirs: &AppDirs,
    storage: &Option<Arc<dyn ToolResultStorage>>,
    scratch: &Option<Arc<dyn ToolScratchStore>>,
    db: &Option<DbState>,
) -> EntrySample {
    match core {
        // `E1` 恒 `Inline`（INV1）：账本必须与响应内顶层字段同源，永不进后台补算。
        CoreEntry::Attachments => {
            log::error!("E1 恒 Inline，不应进入后台补算");
            EntrySample::degraded(StorageAvailabilityDto::Unavailable)
        }
        // 由 `sample_entry` 的 async 分支处理，不会走到这里。
        CoreEntry::AttachmentsDerived => {
            log::error!("E2 由 async 分支处理，不应进入阻塞池");
            EntrySample::degraded(StorageAvailabilityDto::Unreadable)
        }
        CoreEntry::ToolResults => match storage {
            Some(storage) => sample_tool_results(storage),
            None => EntrySample::degraded(StorageAvailabilityDto::NotImplemented),
        },
        CoreEntry::ToolIndex => sample_index(&dirs.indexes()),
        CoreEntry::MascotAssets => sample_mascot(&dirs.mascot_assets()),
        CoreEntry::Database => match db {
            Some(db) => sample_database(db),
            None => EntrySample::degraded(StorageAvailabilityDto::Unavailable),
        },
        CoreEntry::Logs => sample_logs(&dirs.logs()),
        CoreEntry::Crashes => sample_crashes(&dirs.crashes()),
        CoreEntry::WebViewProfile => sample_webview(&dirs.webview_profile()),
        core @ (CoreEntry::Config | CoreEntry::OrbConfig | CoreEntry::MascotConfig) => {
            sample_config_file(&dirs.managed_path_of(core))
        }
        CoreEntry::ToolScratch => match scratch {
            Some(scratch) => sample_scratch(scratch),
            None => EntrySample::degraded(StorageAvailabilityDto::NotImplemented),
        },
    }
}

fn now_seconds() -> i64 {
    crate::cmd::storage::inventory::now_seconds()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn views_fixture() -> Vec<EntryView> {
        views(&[])
    }

    fn deferred_key() -> &'static str {
        crate::app_dirs::registry::find_by_entry(CoreEntry::AttachmentsDerived.id())
            .map(|root| root.key)
            .expect("E2 必须在登记表里")
    }

    /// 只留一条 `Deferred` 视图：状态机单测只关心该条记录，不掺其它条目的缺数触发。
    fn only_deferred(key: &str) -> Vec<EntryView> {
        views_fixture()
            .into_iter()
            .filter(|view| view.key == key)
            .collect()
    }

    fn slot(views: &[EntryView], key: &str) -> usize {
        views
            .iter()
            .position(|view| view.key == key)
            .expect("视图清单必须包含该 key")
    }

    #[test]
    fn every_deferred_view_without_record_triggers_and_inline_stays_missing() {
        let cache = EntryStatsCache::new();
        let views = views_fixture();
        let (samples, triggers) = cache.read(100, &views);
        let expected: Vec<&'static str> = views
            .iter()
            .filter(|view| view.stats_mode == StatsMode::Deferred)
            .map(|view| view.key)
            .collect();
        assert!(!expected.is_empty(), "登记表必须至少有一条 Deferred");
        assert_eq!(triggers, expected);
        for (view, sample) in views.iter().zip(&samples) {
            if view.stats_mode == StatsMode::Inline {
                // `Inline` 槽位是占位：由现场采样覆写，缓存不参与（INV6）。
                assert_eq!(sample.availability, StorageAvailabilityDto::Missing);
                assert_eq!(sample.bytes, Some(0));
            }
        }
    }

    #[test]
    fn no_record_means_no_report_yet_and_triggers() {
        let cache = EntryStatsCache::new();
        let key = deferred_key();
        let views = only_deferred(key);
        let (samples, triggers) = cache.read(100, &views);
        assert_eq!(triggers, vec![key]);
        let sample = &samples[slot(&views, key)];
        assert_eq!(sample.availability, StorageAvailabilityDto::NoReportYet);
        assert_eq!(sample.bytes, None);
        assert_eq!(sample.observed_at, None);
    }

    #[test]
    fn fresh_success_replays_values_without_trigger() {
        let cache = EntryStatsCache::new();
        let key = deferred_key();
        let views = only_deferred(key);
        let token = cache.begin(key, 100).expect("首次 begin 必须拿到 token");
        cache.finish(
            key,
            token,
            EntrySample::observed(1024, 3, Some(10), Some(20)),
            100,
        );
        let (samples, triggers) = cache.read(120, &views);
        assert!(triggers.is_empty());
        let sample = &samples[slot(&views, key)];
        assert_eq!(sample.availability, StorageAvailabilityDto::Available);
        assert_eq!(sample.bytes, Some(1024));
        assert_eq!(sample.observed_at, Some(100));
    }

    #[test]
    fn stale_success_replays_old_values_with_stale_report_and_triggers() {
        let cache = EntryStatsCache::new();
        let key = deferred_key();
        let views = only_deferred(key);
        let token = cache.begin(key, 100).expect("首次 begin 必须拿到 token");
        cache.finish(key, token, EntrySample::observed(1024, 3, None, None), 100);
        let (samples, triggers) = cache.read(100 + ENTRY_STATS_TTL_SECONDS, &views);
        assert_eq!(triggers, vec![key]);
        let sample = &samples[slot(&views, key)];
        assert_eq!(sample.availability, StorageAvailabilityDto::StaleReport);
        assert_eq!(sample.bytes, Some(1024));
        assert_eq!(sample.observed_at, Some(100));
    }

    #[test]
    fn failure_hides_numbers_and_respects_retry_backoff() {
        let cache = EntryStatsCache::new();
        let key = deferred_key();
        let views = only_deferred(key);
        let token = cache.begin(key, 100).expect("首次 begin 必须拿到 token");
        cache.fail(key, token, StorageAvailabilityDto::Unreadable, 100);
        let (_, triggers) = cache.read(110, &views);
        assert!(triggers.is_empty());
        let (samples, triggers) = cache.read(100 + ENTRY_STATS_RETRY_SECONDS, &views);
        assert_eq!(triggers, vec![key]);
        let sample = &samples[slot(&views, key)];
        assert_eq!(sample.availability, StorageAvailabilityDto::Unreadable);
        assert_eq!(sample.bytes, None);
    }

    #[test]
    fn terminal_states_never_trigger() {
        let cache = EntryStatsCache::new();
        let key = deferred_key();
        let views = only_deferred(key);
        let token = cache.begin(key, 100).expect("首次 begin 必须拿到 token");
        cache.finish(
            key,
            token,
            EntrySample::degraded(StorageAvailabilityDto::NotImplemented),
            100,
        );
        let (_, triggers) = cache.read(100 + ENTRY_STATS_TTL_SECONDS * 10, &views);
        assert!(triggers.is_empty());
    }

    /// `locked` 不是终态（I79 §5.2）：不再有「解锁事件通知缓存失效」的钩子，
    /// 只能按失败退避重试，否则锁解除后数字会永久停在 `null`。
    #[test]
    fn locked_retries_after_backoff() {
        let cache = EntryStatsCache::new();
        let key = deferred_key();
        let views = only_deferred(key);
        let token = cache.begin(key, 100).expect("首次 begin 必须拿到 token");
        cache.finish(
            key,
            token,
            EntrySample::degraded(StorageAvailabilityDto::Locked),
            100,
        );
        let (_, triggers) = cache.read(110, &views);
        assert!(triggers.is_empty());
        let (samples, triggers) = cache.read(100 + ENTRY_STATS_RETRY_SECONDS, &views);
        assert_eq!(triggers, vec![key]);
        let sample = &samples[slot(&views, key)];
        assert_eq!(sample.availability, StorageAvailabilityDto::Locked);
        assert_eq!(sample.bytes, None);
    }

    #[test]
    fn inflight_merges_and_placeholder_does_not_retrigger() {
        let cache = EntryStatsCache::new();
        let key = deferred_key();
        let views = only_deferred(key);
        let token = cache.begin(key, 100).expect("首次 begin 必须拿到 token");
        assert!(cache.begin(key, 101).is_none());
        cache.placeholder_timeout(key, token, 105);
        let (samples, triggers) = cache.read(110, &views);
        assert!(triggers.is_empty());
        let sample = &samples[slot(&views, key)];
        assert_eq!(sample.availability, StorageAvailabilityDto::Unavailable);
        assert_eq!(sample.bytes, None);
    }

    #[test]
    fn invalidate_discards_inflight_result() {
        let cache = EntryStatsCache::new();
        let key = deferred_key();
        let views = only_deferred(key);
        let token = cache.begin(key, 100).expect("首次 begin 必须拿到 token");
        cache.invalidate(&[key]);
        cache.finish(key, token, EntrySample::observed(1, 1, None, None), 110);
        let (samples, triggers) = cache.read(110, &views);
        assert_eq!(triggers, vec![key]);
        assert_eq!(
            samples[slot(&views, key)].availability,
            StorageAvailabilityDto::NoReportYet
        );
    }
}
