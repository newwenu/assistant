//! 对账余量扫描与快照状态机（I61 §3.11）：全仓库唯一允许对基根树 `read_dir` 的地方。
//!
//! 三条硬约束（G2 例外边界）：①**只读**——不删、不移、不写（连应用日志都不写，
//! 避免污染 `E12` 的自指字节）；②产出**绝不回流** `entries`（G10）；③有界
//! （深度 / 条目数 / 耗时）。扫描结果只作为「被减数」，减数来自条目本身。
//!
//! 减法放在**读路径**而不是扫描任务里：登记值必须与同一响应里的 `entries` 同源，
//! 若在后台任务里先减一遍，前端看到的 `entries` 与 `inventory` 会来自两个时刻。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

use crate::app_dirs::registry::{self, RootMount, RootScope};
use crate::app_dirs::AppDirs;
use crate::protocol::dto::{
    StorageInventoryClassDto, StorageInventoryDto, StorageInventoryItemDto,
    StorageInventoryNodeDto, StorageInventorySnapshotStatusDto, StorageInventoryStatusDto,
    StorageLayerDto,
};
use crate::protocol::events::{emit_broadcast, event_name};

use super::entries::{sample_for, EntrySample, ENTRY_COUNT};
use super::probe::ProbeRegistry;

/// 快照 TTL（§3.11.4）：扫描比观测贵得多，余量变化频率低。
pub const SCAN_TTL_SECONDS: i64 = 300;

/// 有界扫描上限（§3.11.2 第 5 条）。
const MAX_DEPTH: usize = 8;
const MAX_ENTRIES: usize = 50_000;
const MAX_DURATION: Duration = Duration::from_secs(5);
/// 未登记节点的明细项上限：明细只用于「看一眼是什么」，不做无界展开。
const MAX_CHILDREN: usize = 200;

/// 伴生后缀（§3.11.3）：计入扫描字节，但永远不计入登记值。
const COMPANION_SUFFIXES: &[&str] = &["-journal", "-wal", "-shm", ".tmp", ".lock"];

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// 单个首段的扫描结果；数字为 `None` 表示「知道有、不知多大」（触界 / 读不到）。
#[derive(Debug, Clone, PartialEq)]
pub struct RawNode {
    pub name: String,
    pub scope: RootScope,
    pub class: StorageInventoryClassDto,
    /// `declared` / `optionalNotMounted` 给出对应 `E` 编号，其余 `None`。
    pub entry: Option<&'static str>,
    pub scan_bytes: Option<u64>,
    pub scan_count: Option<u64>,
    pub children: Vec<StorageInventoryItemDto>,
    pub truncated: bool,
}

/// 一次扫描的产物：节点 + 全局触界标记。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScanResult {
    pub nodes: Vec<RawNode>,
    pub partial: bool,
    pub truncated: Vec<String>,
}

/// 首层分类（§3.11.3，互斥且穷尽）：纯函数，便于逐条断言。
///
/// `mounted` 是**已注册的能力名**（`ProbeRegistry::capabilities()`）：可选能力的
/// 挂载判定按登记表声明的能力名查表，不看是哪个 crate 实现的（G18）。
pub fn classify(
    name: &str,
    mounted: &[&'static str],
) -> (StorageInventoryClassDto, Option<&'static str>) {
    if let Some(root) = registry::find_by_first_segment(name) {
        let is_mounted = match root.mount {
            RootMount::Always => true,
            RootMount::Feature(capability) => mounted.contains(&capability),
        };
        return if is_mounted {
            (StorageInventoryClassDto::Declared, Some(root.entry))
        } else {
            (
                StorageInventoryClassDto::OptionalNotMounted,
                Some(root.entry),
            )
        };
    }
    if registry::IGNORED_NAMES.contains(&name) {
        return (StorageInventoryClassDto::Ignored, None);
    }
    if COMPANION_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
    {
        return (StorageInventoryClassDto::Companion, None);
    }
    (StorageInventoryClassDto::Unregistered, None)
}

/// 单段名校验（§3.11.3）：单段名不是路径，不得含分隔符 / 上跳 / NUL。
fn is_safe_segment(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.chars().any(|c| c == '\0' || c.is_control())
}

fn layer_of(scope: RootScope) -> StorageLayerDto {
    match scope {
        RootScope::Data => StorageLayerDto::Data,
        RootScope::Local => StorageLayerDto::Local,
    }
}

/// 有界遍历预算：深度 / 条目数 / 耗时三条上限，任一条触界即停。
struct Budget {
    entries: usize,
    deadline: Instant,
}

impl Budget {
    fn new() -> Self {
        Self {
            entries: 0,
            deadline: Instant::now() + MAX_DURATION,
        }
    }

    fn exhausted(&self) -> bool {
        self.entries >= MAX_ENTRIES || Instant::now() >= self.deadline
    }
}

/// 遍历中止原因：两者都降级为 `null`，但只有触界进 `truncatedNodes`。
enum Stop {
    Truncated,
    Unreadable,
}

/// 观测结果：字节 / 条数同一次遍历产出（§3.11.2 第 3 条，绝不分两次扫）。
struct Observed {
    bytes: Option<u64>,
    count: Option<u64>,
    truncated: bool,
}

fn observe(path: &Path) -> Observed {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Observed {
                bytes: Some(0),
                count: Some(0),
                truncated: false,
            }
        }
        Err(_) => {
            return Observed {
                bytes: None,
                count: None,
                truncated: false,
            }
        }
    };
    if meta.file_type().is_symlink() {
        return Observed {
            bytes: Some(0),
            count: Some(0),
            truncated: false,
        };
    }
    if meta.is_file() {
        return Observed {
            bytes: Some(meta.len()),
            count: Some(1),
            truncated: false,
        };
    }
    if !meta.is_dir() {
        return Observed {
            bytes: Some(0),
            count: Some(0),
            truncated: false,
        };
    }
    let mut bytes = 0u64;
    let mut count = 0u64;
    let mut budget = Budget::new();
    match walk(path, 0, &mut budget, &mut bytes, &mut count) {
        Ok(()) => Observed {
            bytes: Some(bytes),
            count: Some(count),
            truncated: false,
        },
        Err(Stop::Truncated) => Observed {
            bytes: None,
            count: None,
            truncated: true,
        },
        Err(Stop::Unreadable) => Observed {
            bytes: None,
            count: None,
            truncated: false,
        },
    }
}

fn walk(
    dir: &Path,
    depth: usize,
    budget: &mut Budget,
    bytes: &mut u64,
    count: &mut u64,
) -> Result<(), Stop> {
    if depth >= MAX_DEPTH || budget.exhausted() {
        return Err(Stop::Truncated);
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(Stop::Unreadable),
    };
    for entry in entries {
        if budget.exhausted() {
            return Err(Stop::Truncated);
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => return Err(Stop::Unreadable),
        };
        budget.entries = budget.entries.saturating_add(1);
        let path = entry.path();
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => return Err(Stop::Unreadable),
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            walk(&path, depth + 1, budget, bytes, count)?;
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        *bytes = bytes.saturating_add(meta.len());
        *count = count.saturating_add(1);
    }
    Ok(())
}

/// 未登记节点的明细：直接子项，单段名 + 字节 + 条数（§3.11.3）。
fn children_of(dir: &Path) -> Vec<StorageInventoryItemDto> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut items = Vec::new();
    for entry in entries {
        if items.len() >= MAX_CHILDREN {
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => break,
        };
        let name = match entry.file_name().to_str() {
            Some(name) if is_safe_segment(name) => name.to_string(),
            _ => continue,
        };
        let observed = observe(&entry.path());
        items.push(StorageInventoryItemDto {
            name,
            bytes: observed.bytes,
            count: observed.count,
        });
    }
    items
}

fn list_root(root: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = match entry.file_name().to_str() {
            Some(name) if is_safe_segment(name) => name.to_string(),
            _ => continue,
        };
        names.push((name, path));
    }
    names.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(names)
}

fn build_node(name: String, path: &Path, scope: RootScope, mounted: &[&'static str]) -> RawNode {
    let (class, entry) = classify(&name, mounted);
    let observed = observe(path);
    let children = if class == StorageInventoryClassDto::Unregistered && path.is_dir() {
        children_of(path)
    } else {
        Vec::new()
    };
    RawNode {
        name,
        scope,
        class,
        entry,
        scan_bytes: observed.bytes,
        scan_count: observed.count,
        children,
        truncated: observed.truncated,
    }
}

/// 扫描整棵基根树的首层（§3.11.2）：登记首段按登记表顺序在前，其余按名字排序在后。
pub fn scan(dirs: &AppDirs, mounted: &[&'static str]) -> std::io::Result<ScanResult> {
    let data_entries = list_root(dirs.data())?;
    let local_entries = list_root(dirs.local())?;
    let data_map: HashMap<&str, &PathBuf> = data_entries
        .iter()
        .map(|(name, path)| (name.as_str(), path))
        .collect();
    let local_map: HashMap<&str, &PathBuf> = local_entries
        .iter()
        .map(|(name, path)| (name.as_str(), path))
        .collect();

    let mut nodes = Vec::new();
    let mut used: HashSet<&str> = HashSet::new();
    let mut seen_first: HashSet<&str> = HashSet::new();
    for root in registry::MANAGED_ROOTS {
        let first = root.segments[0];
        if !seen_first.insert(first) {
            continue;
        }
        let (map, scope) = match root.scope {
            RootScope::Data => (&data_map, RootScope::Data),
            RootScope::Local => (&local_map, RootScope::Local),
        };
        if let Some(path) = map.get(first) {
            used.insert(first);
            nodes.push(build_node(first.to_string(), path, scope, mounted));
        }
    }
    for (name, path) in data_entries.iter() {
        if used.contains(name.as_str()) {
            continue;
        }
        nodes.push(build_node(name.clone(), path, RootScope::Data, mounted));
    }
    for (name, path) in local_entries.iter() {
        if used.contains(name.as_str()) {
            continue;
        }
        nodes.push(build_node(name.clone(), path, RootScope::Local, mounted));
    }

    let partial = nodes.iter().any(|node| node.truncated);
    let truncated = nodes
        .iter()
        .filter(|node| node.truncated)
        .map(|node| node.name.clone())
        .collect();
    Ok(ScanResult {
        nodes,
        partial,
        truncated,
    })
}

/// 减法（§3.11.3，纯函数、无 IO）：登记值只来自 `declared` 条目，其余类恒 `0`。
pub fn subtract_declared(
    node: &RawNode,
    samples: &[EntrySample; ENTRY_COUNT],
) -> (Option<u64>, Option<u64>) {
    if node.class != StorageInventoryClassDto::Declared {
        return (Some(0), Some(0));
    }
    let mut bytes: Option<u64> = Some(0);
    let mut count: Option<u64> = Some(0);
    for root in registry::MANAGED_ROOTS {
        if root.segments[0] != node.name.as_str() {
            continue;
        }
        let sample = sample_for(samples, root);
        bytes = match (bytes, sample.bytes) {
            (Some(acc), Some(value)) => Some(acc.saturating_add(value)),
            _ => None,
        };
        count = match (count, sample.count) {
            (Some(acc), Some(value)) => Some(acc.saturating_add(value)),
            _ => None,
        };
    }
    (bytes, count)
}

fn residual(scan: Option<u64>, declared: Option<u64>) -> Option<u64> {
    match (scan, declared) {
        (Some(scan), Some(declared)) => Some(scan.saturating_sub(declared)),
        _ => None,
    }
}

fn node_dto(node: &RawNode, samples: &[EntrySample; ENTRY_COUNT]) -> StorageInventoryNodeDto {
    let (declared_bytes, declared_count) = subtract_declared(node, samples);
    let over_registered = matches!(
        (declared_bytes, node.scan_bytes),
        (Some(declared), Some(scan)) if declared > scan
    );
    StorageInventoryNodeDto {
        name: node.name.clone(),
        class: node.class,
        scope: layer_of(node.scope),
        entry: node.entry.map(str::to_string),
        scan_bytes: node.scan_bytes,
        scan_count: node.scan_count,
        declared_bytes,
        declared_count,
        residual_bytes: residual(node.scan_bytes, declared_bytes),
        residual_count: residual(node.scan_count, declared_count),
        over_registered,
        children: node.children.clone(),
    }
}

fn sum_unregistered(nodes: &[RawNode], pick: impl Fn(&RawNode) -> Option<u64>) -> Option<u64> {
    let mut total: Option<u64> = Some(0);
    for node in nodes {
        if node.class != StorageInventoryClassDto::Unregistered {
            continue;
        }
        total = match (total, pick(node)) {
            (Some(acc), Some(value)) => Some(acc.saturating_add(value)),
            _ => None,
        };
    }
    total
}

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
    /// `samples` 是同一响应里的 13 条观测值——减法与 `entries` 同源，绝不错位。
    pub fn read(
        &self,
        now: i64,
        samples: &[EntrySample; ENTRY_COUNT],
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
            .map(|node| node_dto(node, samples))
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
        // 挂载判定只看「已注册的能力名」，不看 provider 类型 / 特性开关（G18）。
        let mounted: Vec<&'static str> = handle
            .try_state::<Arc<ProbeRegistry>>()
            .map(|state| state.inner().capabilities())
            .unwrap_or_default();
        let scan_cache = cache.clone();
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            let dirs = AppDirs::resolve().map_err(|error| error.to_string())?;
            scan(&dirs, &mounted).map_err(|error| error.to_string())
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
    use crate::protocol::dto::StorageAvailabilityDto;

    fn samples() -> [EntrySample; ENTRY_COUNT] {
        std::array::from_fn(|_| EntrySample::observed(1024, 1, Some(100), Some(200)))
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
    fn classification_is_exclusive_and_exhaustive() {
        let none: &[&'static str] = &[];
        let all = registry::feature_capabilities();
        assert_eq!(
            classify("attachments", none).0,
            StorageInventoryClassDto::Declared
        );
        assert_eq!(
            classify("security", none).0,
            StorageInventoryClassDto::OptionalNotMounted
        );
        assert_eq!(
            classify("security", &all).0,
            StorageInventoryClassDto::Declared
        );
        assert_eq!(
            classify("Thumbs.db", none).0,
            StorageInventoryClassDto::Ignored
        );
        assert_eq!(
            classify("assistant.db-wal", none).0,
            StorageInventoryClassDto::Companion
        );
        assert_eq!(
            classify("stray", none).0,
            StorageInventoryClassDto::Unregistered
        );
        assert_eq!(classify("attachments", none).1, Some("E1"));
        assert_eq!(classify("security", &all).1, Some("E7"));
    }

    #[test]
    fn optional_mount_is_keyed_by_capability_not_by_any_provider() {
        let all = registry::feature_capabilities();
        let wrong: &[&'static str] = &["other-capability"];
        assert_eq!(
            classify("security", wrong).0,
            StorageInventoryClassDto::OptionalNotMounted,
            "能力名不匹配时不得判为已挂载"
        );
        assert_eq!(
            all,
            vec![registry::SECURITY_OBSERVABILITY],
            "登记表声明的能力名去重后只有一个"
        );
        let feature_rows = registry::MANAGED_ROOTS
            .iter()
            .filter(|root| matches!(root.mount, RootMount::Feature(_)))
            .count();
        assert_eq!(feature_rows, 5, "E7–E11 五条都挂同一个能力名");
    }

    #[test]
    fn companion_suffixes_are_not_unregistered() {
        let none: &[&'static str] = &[];
        for name in ["x.tmp", "x.lock", "db-journal", "db-shm"] {
            assert_eq!(
                classify(name, none).0,
                StorageInventoryClassDto::Companion,
                "{name} 必须归 companion"
            );
        }
    }

    #[test]
    fn unsafe_segments_are_rejected() {
        for name in ["", ".", "..", "a/b", "a\\b", "a\u{0}b"] {
            assert!(!is_safe_segment(name), "`{name}` 必须被拒绝");
        }
        assert!(is_safe_segment("attachments"));
    }

    #[test]
    fn subtraction_only_applies_to_declared_nodes() {
        let samples = samples();
        let declared = raw(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(4_000),
        );
        let (declared_bytes, declared_count) = subtract_declared(&declared, &samples);
        // attachments 首段下有 E1 与 E2 两条登记。
        assert_eq!(declared_bytes, Some(2048));
        assert_eq!(declared_count, Some(2));

        let unregistered = raw("stray", StorageInventoryClassDto::Unregistered, Some(900));
        let (bytes, count) = subtract_declared(&unregistered, &samples);
        assert_eq!(bytes, Some(0), "未登记节点没有减数");
        assert_eq!(count, Some(0));
    }

    #[test]
    fn declared_unknown_degrades_residual_to_null() {
        let mut samples = samples();
        samples[0] = EntrySample::degraded(StorageAvailabilityDto::Unreadable, "io");
        let node = raw(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(4_000),
        );
        let (declared, _) = subtract_declared(&node, &samples);
        assert_eq!(declared, None, "任一登记值为 null 时登记值整体为 null");
        let dto = node_dto(&node, &samples);
        assert_eq!(dto.residual_bytes, None);
        assert!(!dto.over_registered);
    }

    #[test]
    fn over_registered_is_flagged() {
        let mut samples = samples();
        samples[0] = EntrySample::observed(5_000, 1, None, None);
        let node = raw(
            "attachments",
            StorageInventoryClassDto::Declared,
            Some(1_000),
        );
        let dto = node_dto(&node, &samples);
        assert!(dto.over_registered, "登记值大于扫描值时报警");
        assert_eq!(dto.residual_bytes, Some(0), "余量不得为负");
    }

    #[test]
    fn unregistered_sums_degrade_to_null() {
        let nodes = vec![
            raw("a", StorageInventoryClassDto::Unregistered, Some(10)),
            raw("b", StorageInventoryClassDto::Unregistered, None),
        ];
        assert_eq!(sum_unregistered(&nodes, |node| node.scan_bytes), None);
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
        let (dto, trigger) = cache.read(100, &samples());
        assert_eq!(dto.status, StorageInventoryStatusDto::NeverRun);
        assert!(trigger, "neverRun 必须触发后台扫描");
        assert!(dto.nodes.is_empty());

        assert!(cache.begin(), "首次 begin 成功");
        assert!(!cache.begin(), "running 期间不得叠加第二次扫描");
        let (dto, trigger) = cache.read(100, &samples());
        assert_eq!(dto.status, StorageInventoryStatusDto::Running);
        assert!(!trigger);

        let mut result = ScanResult::default();
        result.nodes.push(raw(
            "stray",
            StorageInventoryClassDto::Unregistered,
            Some(123),
        ));
        cache.finish(result, 100);
        let (dto, trigger) = cache.read(100, &samples());
        assert_eq!(dto.status, StorageInventoryStatusDto::Fresh);
        assert_eq!(dto.unregistered_bytes, Some(123));
        assert!(!trigger);
    }

    #[test]
    fn failure_keeps_previous_numbers() {
        let cache = InventoryCache::new();
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
        let (dto, _) = cache.read(200, &samples());
        assert_eq!(dto.status, StorageInventoryStatusDto::Failed);
        assert_eq!(dto.failed_at, Some(200));
        assert_eq!(dto.scanned_at, Some(100), "失败保留上次成功时刻");
        assert_eq!(dto.unregistered_bytes, Some(500), "失败保留上次数字");
    }
}
