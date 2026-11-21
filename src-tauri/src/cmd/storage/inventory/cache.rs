//! 快照缓存与状态机（§3.11.4）：读路径零等待，扫描永远在后台、single-flight。

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};

use crate::app_dirs::AppDirs;
use crate::cmd::storage::entries::{views, EntrySample, EntryView};
use crate::cmd::storage::section::SectionRegistry;
use crate::protocol::dto::{
    StorageInventoryDto, StorageInventoryNodeDto, StorageInventorySnapshotStatusDto,
    StorageInventoryStatusDto,
};
use crate::protocol::events::{emit_broadcast, event_name};

use super::subtract::{node_dto, sum_unregistered};
use super::walk::scan;
use super::{now_seconds, RawNode, ScanResult, SCAN_TTL_SECONDS};

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
        let dto = StorageInventoryDto {
            status,
            scanned_at: state.scanned_at,
            failed_at: state.failed_at,
            unregistered_bytes: sum_unregistered(&state.nodes, |node| node.scan_bytes),
            unregistered_count: sum_unregistered(&state.nodes, |node| node.scan_count),
            partial: state.partial || over_registered,
            truncated_nodes: state.truncated.clone(),
            nodes,
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
