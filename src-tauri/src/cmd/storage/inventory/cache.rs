//! 快照缓存与状态机（§3.11.4）：读路径零等待，扫描永远在后台、single-flight。

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};

use crate::app_dirs::AppDirs;
use crate::cmd::storage::entries::{find_by_entry, views, EntrySample, EntryView};
use crate::cmd::storage::section::SectionRegistry;
use crate::protocol::dto::{
    StorageInventoryClassDto, StorageInventoryDto, StorageInventoryNodeDto,
    StorageInventoryScopeTotalsDto, StorageInventorySnapshotStatusDto, StorageInventoryStatusDto,
    StorageInventoryTotalsDto, StorageLayerDto,
};
use crate::protocol::events::{emit_broadcast, event_name};

use super::subtract::{node_dto, sum_unregistered};
use super::walk::scan;
use super::{now_seconds, RawNode, ScanResult, ScanTotals, ScopeScanTotals, SCAN_TTL_SECONDS};

/// 快照状态（§3.11.4）：`running` 优先，其次失败，再按 TTL 区分 `fresh` / `stale`。
fn status_of(
    running: bool,
    scanned_at: Option<i64>,
    failed_at: Option<i64>,
    now: i64,
) -> StorageInventoryStatusDto {
    if running {
        return StorageInventoryStatusDto::Running;
    }
    if let Some(failed_at) = failed_at {
        if scanned_at.is_none_or(|at| failed_at >= at) {
            return StorageInventoryStatusDto::Failed;
        }
    }
    match scanned_at {
        None => StorageInventoryStatusDto::NeverRun,
        Some(at) if now.saturating_sub(at) >= SCAN_TTL_SECONDS => StorageInventoryStatusDto::Stale,
        Some(_) => StorageInventoryStatusDto::Fresh,
    }
}

#[derive(Debug, Default)]
struct CacheState {
    running: bool,
    nodes: Vec<RawNode>,
    scanned_at: Option<i64>,
    failed_at: Option<i64>,
    partial: bool,
    truncated: Vec<String>,
    totals: ScanTotals,
    totals_partial: bool,
}

/// 跳过 `null` 项累加：`total` 从 `Some(0)` 起，任一贡献为 `null` 也不变 `None`（下界，§3.6）。
fn add_option(total: &mut Option<u64>, value: Option<u64>) {
    if let Some(value) = value {
        *total = total.map(|acc| acc.saturating_add(value));
    }
}

fn combine(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.saturating_add(b)),
        _ => None,
    }
}

/// 该节点是否属于「只关减法、仍观测」的 `inventory = false` 首段（`E12`，§3.5）。
///
/// 只能按 `node.entry` 回查条目清单的 `inventory` 标志，不得匹配 `E` 字面量（§4.3）。
fn is_unreconciled(node: &StorageInventoryNodeDto, views: &[EntryView]) -> bool {
    node.entry
        .as_deref()
        .and_then(|entry| find_by_entry(views, entry))
        .is_some_and(|view| !view.inventory)
}

fn scope_totals_dto(
    nodes: &[StorageInventoryNodeDto],
    layer: StorageLayerDto,
    scan: &ScopeScanTotals,
    views: &[EntryView],
) -> StorageInventoryScopeTotalsDto {
    let mut declared_bytes: Option<u64> = Some(0);
    let mut declared_count: Option<u64> = Some(0);
    let mut residual_bytes: Option<u64> = Some(0);
    let mut residual_count: Option<u64> = Some(0);
    let mut unregistered_bytes: Option<u64> = Some(0);
    let mut unregistered_count: Option<u64> = Some(0);
    let mut unreconciled_bytes: Option<u64> = Some(0);
    let mut unreconciled_count: Option<u64> = Some(0);
    for node in nodes.iter().filter(|node| node.scope == layer) {
        if node.class == StorageInventoryClassDto::Declared {
            add_option(&mut declared_bytes, node.declared_bytes);
            add_option(&mut declared_count, node.declared_count);
        }
        add_option(&mut residual_bytes, node.residual_bytes);
        add_option(&mut residual_count, node.residual_count);
        if node.class == StorageInventoryClassDto::Unregistered {
            add_option(&mut unregistered_bytes, node.scan_bytes);
            add_option(&mut unregistered_count, node.scan_count);
        }
        if is_unreconciled(node, views) {
            add_option(&mut unreconciled_bytes, node.scan_bytes);
            add_option(&mut unreconciled_count, node.scan_count);
        }
    }
    StorageInventoryScopeTotalsDto {
        scan_bytes: scan.scan_bytes,
        scan_count: scan.scan_count,
        declared_bytes,
        declared_count,
        residual_bytes,
        residual_count,
        unregistered_bytes,
        unregistered_count,
        unreconciled_bytes,
        unreconciled_count,
        unnameable_bytes: scan.unnameable_bytes,
        unnameable_count: scan.unnameable_count,
    }
}

fn totals_dto(
    nodes: &[StorageInventoryNodeDto],
    totals: &ScanTotals,
    views: &[EntryView],
    partial: bool,
) -> StorageInventoryTotalsDto {
    let data = scope_totals_dto(nodes, StorageLayerDto::Data, &totals.data, views);
    let local = scope_totals_dto(nodes, StorageLayerDto::Local, &totals.local, views);
    StorageInventoryTotalsDto {
        scan_bytes: combine(data.scan_bytes, local.scan_bytes),
        scan_count: combine(data.scan_count, local.scan_count),
        residual_bytes: combine(data.residual_bytes, local.residual_bytes),
        residual_count: combine(data.residual_count, local.residual_count),
        partial,
        data,
        local,
    }
}

/// 余量快照的唯一持有者：读路径零等待，扫描永远在后台（§3.11.4）。
#[derive(Clone, Default)]
pub struct InventoryCache {
    inner: Arc<Mutex<CacheState>>,
}

impl InventoryCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CacheState> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// 读快照：返回 `(DTO, 是否应触发后台扫描)`。
    ///
    /// `views` / `samples` 是同一响应里的条目清单与观测值——减法与 `entries` 同源，绝不错位。
    pub fn read(
        &self,
        now: i64,
        views: &[EntryView],
        samples: &[EntrySample],
    ) -> (StorageInventoryDto, bool) {
        let state = self.lock();
        let status = status_of(state.running, state.scanned_at, state.failed_at, now);
        let trigger = !state.running
            && matches!(
                status,
                StorageInventoryStatusDto::NeverRun | StorageInventoryStatusDto::Stale
            );
        let nodes: Vec<StorageInventoryNodeDto> = state
            .nodes
            .iter()
            .map(|node| node_dto(node, views, samples))
            .collect();
        let over_registered = nodes.iter().any(|node| node.over_registered);
        let totals = if state.scanned_at.is_some() {
            totals_dto(&nodes, &state.totals, views, state.totals_partial)
        } else {
            StorageInventoryTotalsDto::default()
        };
        let dto = StorageInventoryDto {
            status,
            scanned_at: state.scanned_at,
            failed_at: state.failed_at,
            unregistered_bytes: sum_unregistered(&state.nodes, |node| node.scan_bytes),
            unregistered_count: sum_unregistered(&state.nodes, |node| node.scan_count),
            partial: state.partial || over_registered,
            truncated_nodes: state.truncated.clone(),
            nodes,
            totals,
        };
        (dto, trigger)
    }

    /// 只回状态、不回数字（`refresh_storage_inventory` 的返回类型据此成立）。
    pub fn status(&self, now: i64) -> StorageInventorySnapshotStatusDto {
        let state = self.lock();
        StorageInventorySnapshotStatusDto {
            status: status_of(state.running, state.scanned_at, state.failed_at, now),
            scanned_at: state.scanned_at,
            failed_at: state.failed_at,
        }
    }

    /// single-flight：`running` 期间重复触发直接复用，不排队、不叠加。
    pub fn begin(&self) -> bool {
        let mut state = self.lock();
        if state.running {
            return false;
        }
        state.running = true;
        true
    }

    pub fn finish(&self, result: ScanResult, at: i64) {
        let mut state = self.lock();
        state.running = false;
        state.nodes = result.nodes;
        state.partial = result.partial;
        state.truncated = result.truncated;
        state.totals = result.totals;
        state.totals_partial = result.totals_partial;
        state.scanned_at = Some(at);
        state.failed_at = None;
    }

    pub fn fail(&self, at: i64) {
        let mut state = self.lock();
        state.running = false;
        state.failed_at = Some(at);
    }
}

/// 触发后台一次扫描（幂等、single-flight）：失败保留上次数字 + `failedAt`。
pub fn trigger_scan(app: &AppHandle) {
    let cache = match app.try_state::<InventoryCache>() {
        Some(state) => state.inner().clone(),
        None => return,
    };
    if !cache.begin() {
        return;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // 扫描清单与本次响应同源：分区快照只在触发时取一次（未挂载分区不贡献首段）。
        let scan_views = handle
            .try_state::<Arc<SectionRegistry>>()
            .map(|state| views(&state.inner().snapshot()))
            .unwrap_or_else(|| views(&[]));
        let scan_cache = cache.clone();
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            let dirs = AppDirs::resolve().map_err(|error| error.to_string())?;
            scan(&dirs, &scan_views).map_err(|error| error.to_string())
        })
        .await;
        let at = now_seconds();
        match outcome {
            Ok(Ok(result)) => scan_cache.finish(result, at),
            Ok(Err(_)) | Err(_) => scan_cache.fail(at),
        }
        emit_broadcast(
            &handle,
            event_name::STORAGE_INVENTORY_UPDATED,
            cache.status(now_seconds()),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_dirs::registry::RootScope;
    use crate::protocol::dto::StorageInventoryClassDto;

    fn fixtures() -> (Vec<EntryView>, Vec<EntrySample>) {
        let views = views(&[]);
        let samples = views
            .iter()
            .map(|_| EntrySample::observed(1024, 1, Some(100), Some(200)))
            .collect();
        (views, samples)
    }

    fn raw(name: &str, class: StorageInventoryClassDto, scan: Option<u64>) -> RawNode {
        RawNode {
            name: name.to_string(),
            scope: RootScope::Data,
            class,
            entry: None,
            scan_bytes: scan,
            scan_count: scan.map(|_| 1),
            children: Vec::new(),
            truncated: false,
        }
    }

    #[test]
    fn status_machine_follows_ttl_and_failure() {
        assert_eq!(
            status_of(false, None, None, 100),
            StorageInventoryStatusDto::NeverRun
        );
        assert_eq!(
            status_of(true, Some(100), None, 200),
            StorageInventoryStatusDto::Running
        );
        assert_eq!(
            status_of(false, Some(100), None, 200),
            StorageInventoryStatusDto::Fresh
        );
        assert_eq!(
            status_of(false, Some(100), None, 100 + SCAN_TTL_SECONDS),
            StorageInventoryStatusDto::Stale
        );
        assert_eq!(
            status_of(false, Some(100), Some(300), 400),
            StorageInventoryStatusDto::Failed
        );
        assert_eq!(
            status_of(false, Some(300), Some(200), 400),
            StorageInventoryStatusDto::Fresh,
            "失败早于上次成功时以成功为准"
        );
    }

    #[test]
    fn cache_single_flight_and_read_never_waits() {
        let cache = InventoryCache::new();
        let (views, samples) = fixtures();
        let (dto, trigger) = cache.read(100, &views, &samples);
        assert_eq!(dto.status, StorageInventoryStatusDto::NeverRun);
        assert!(trigger, "neverRun 必须触发后台扫描");
        assert!(dto.nodes.is_empty());

        assert!(cache.begin(), "首次 begin 成功");
        assert!(!cache.begin(), "running 期间不得叠加第二次扫描");
        let (dto, trigger) = cache.read(100, &views, &samples);
        assert_eq!(dto.status, StorageInventoryStatusDto::Running);
        assert!(!trigger);

        let mut result = ScanResult::default();
        result.nodes.push(raw(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(123),
        ));
        cache.finish(result, 100);
        let (dto, trigger) = cache.read(100, &views, &samples);
        assert_eq!(dto.status, StorageInventoryStatusDto::Fresh);
        assert_eq!(dto.unregistered_bytes, Some(123));
        assert!(!trigger);
    }

    #[test]
    fn failure_keeps_previous_numbers() {
        let cache = InventoryCache::new();
        let (views, samples) = fixtures();
        assert!(cache.begin());
        let mut result = ScanResult::default();
        result.nodes.push(raw(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(500),
        ));
        cache.finish(result, 100);
        assert!(cache.begin());
        cache.fail(200);
        let (dto, _) = cache.read(200, &views, &samples);
        assert_eq!(dto.status, StorageInventoryStatusDto::Failed);
        assert_eq!(dto.failed_at, Some(200));
        assert_eq!(dto.scanned_at, Some(100), "失败保留上次成功时刻");
        assert_eq!(dto.unregistered_bytes, Some(500), "失败保留上次数字");
    }
}
