//! 有界遍历扫描（§3.11.2）：**全仓库唯一允许对基根树 `read_dir` 的地方**。
//!
//! 只读、有界（深度 / 条目数 / 耗时），字节与条数同一次遍历产出（第 3 条，绝不分两次扫）。
//! 触界降级为 `null` 并记入 `truncatedNodes`；读不到只降级为 `null`，不算触界。
//!
//! `inventory = false` 的条目（`E12`）**仍要 `observe`**：它确实占盘，缺了分母就不完整
//! （I67 §3.5）。`inventory = false` 只关减法（`subtract_declared` 不主张登记值），
//! 不关观测——「不做减法」与「不做观测」是两件事。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::app_dirs::registry::RootScope;
use crate::app_dirs::AppDirs;
use crate::cmd::storage::entries::EntryView;
use crate::protocol::dto::{StorageInventoryClassDto, StorageInventoryItemDto};

use super::classify::{classify, is_safe_segment};
use super::{RawNode, ScanResult, ScanTotals, ScopeScanTotals};

/// 有界扫描上限（§3.11.2 第 5 条）。
const MAX_DEPTH: usize = 8;
const MAX_ENTRIES: usize = 50_000;
const MAX_DURATION: Duration = Duration::from_secs(5);
/// 未登记节点的明细项上限：明细只用于「看一眼是什么」，不做无界展开。
const MAX_CHILDREN: usize = 200;

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

/// 首层枚举结果：可命名的条目 + 不可命名部分的观测（§3.4）。
///
/// 不可命名条目（非法 UTF-8 / 非安全单段名）仍要 `observe`：知道有、知道多大、
/// 只是叫不出名字，就单列一个桶，不许静默丢弃（I67 §3.4）。
struct Listing {
    named: Vec<(String, PathBuf)>,
    unnameable: Observed,
}

fn list_root(root: &Path) -> std::io::Result<Listing> {
    let empty = Listing {
        named: Vec::new(),
        unnameable: Observed {
            bytes: Some(0),
            count: Some(0),
            truncated: false,
        },
    };
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(empty),
        Err(error) => return Err(error),
    };
    let mut listing = empty;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        match entry.file_name().to_str() {
            Some(name) if is_safe_segment(name) => listing.named.push((name.to_string(), path)),
            _ => merge_observed(&mut listing.unnameable, observe(&path)),
        }
    }
    listing.named.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(listing)
}

/// 不可命名部分的累加：任一子项触界 / 读不到 → 该桶为 `null`（下界，§3.4）。
fn merge_observed(acc: &mut Observed, next: Observed) {
    acc.truncated |= next.truncated;
    acc.bytes = match (acc.bytes, next.bytes) {
        (Some(a), Some(b)) => Some(a.saturating_add(b)),
        _ => None,
    };
    acc.count = match (acc.count, next.count) {
        (Some(a), Some(b)) => Some(a.saturating_add(b)),
        _ => None,
    };
}

fn build_node(name: String, path: &Path, scope: RootScope, views: &[EntryView]) -> RawNode {
    let (class, entry) = classify(&name, views);
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

/// 单个 scope 的分母：全部首层节点 + 不可命名部分，`null` 项跳过累加（下界）。
fn scope_scan_totals(
    nodes: &[RawNode],
    scope: RootScope,
    unnameable: &Observed,
) -> (ScopeScanTotals, bool) {
    let mut scan_bytes: Option<u64> = Some(0);
    let mut scan_count: Option<u64> = Some(0);
    let mut partial = false;
    for node in nodes.iter().filter(|node| node.scope == scope) {
        match node.scan_bytes {
            Some(value) => scan_bytes = scan_bytes.map(|acc| acc.saturating_add(value)),
            None => partial = true,
        }
        match node.scan_count {
            Some(value) => scan_count = scan_count.map(|acc| acc.saturating_add(value)),
            None => partial = true,
        }
    }
    match unnameable.bytes {
        Some(value) => scan_bytes = scan_bytes.map(|acc| acc.saturating_add(value)),
        None => partial = true,
    }
    match unnameable.count {
        Some(value) => scan_count = scan_count.map(|acc| acc.saturating_add(value)),
        None => partial = true,
    }
    (
        ScopeScanTotals {
            scan_bytes,
            scan_count,
            unnameable_bytes: unnameable.bytes,
            unnameable_count: unnameable.count,
        },
        partial,
    )
}

/// 扫描整棵基根树的首层（§3.11.2）：清单里的首段按清单顺序在前，其余按名字排序在后。
///
/// 清单来自 [`EntryView`]（core 条目 + 已挂载分区条目），所以「已登记首段」不必再查登记表。
pub fn scan(dirs: &AppDirs, views: &[EntryView]) -> std::io::Result<ScanResult> {
    let data = list_root(dirs.data())?;
    let local = list_root(dirs.local())?;
    let data_map: HashMap<&str, &PathBuf> = data
        .named
        .iter()
        .map(|(name, path)| (name.as_str(), path))
        .collect();
    let local_map: HashMap<&str, &PathBuf> = local
        .named
        .iter()
        .map(|(name, path)| (name.as_str(), path))
        .collect();

    let mut nodes = Vec::new();
    let mut used: HashSet<&str> = HashSet::new();
    let mut seen_first: HashSet<&str> = HashSet::new();
    for view in views {
        let Some(first) = view.segments.first().copied() else {
            continue;
        };
        if !seen_first.insert(first) {
            continue;
        }
        let (map, scope) = match view.scope {
            RootScope::Data => (&data_map, RootScope::Data),
            RootScope::Local => (&local_map, RootScope::Local),
        };
        if let Some(path) = map.get(first) {
            used.insert(first);
            nodes.push(build_node(first.to_string(), path, scope, views));
        }
    }
    for (name, path) in data.named.iter() {
        if used.contains(name.as_str()) {
            continue;
        }
        nodes.push(build_node(name.clone(), path, RootScope::Data, views));
    }
    for (name, path) in local.named.iter() {
        if used.contains(name.as_str()) {
            continue;
        }
        nodes.push(build_node(name.clone(), path, RootScope::Local, views));
    }

    let partial = nodes.iter().any(|node| node.truncated);
    let truncated = nodes
        .iter()
        .filter(|node| node.truncated)
        .map(|node| node.name.clone())
        .collect();
    let (data_totals, data_partial) = scope_scan_totals(&nodes, RootScope::Data, &data.unnameable);
    let (local_totals, local_partial) =
        scope_scan_totals(&nodes, RootScope::Local, &local.unnameable);
    Ok(ScanResult {
        nodes,
        partial,
        truncated,
        totals: ScanTotals {
            data: data_totals,
            local: local_totals,
        },
        totals_partial: data_partial || local_partial,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, scope: RootScope, scan: Option<u64>) -> RawNode {
        RawNode {
            name: name.to_string(),
            scope,
            class: StorageInventoryClassDto::Declared,
            entry: None,
            scan_bytes: scan,
            scan_count: scan.map(|_| 1),
            children: Vec::new(),
            truncated: false,
        }
    }

    fn observed(bytes: Option<u64>, count: Option<u64>) -> Observed {
        Observed {
            bytes,
            count,
            truncated: false,
        }
    }

    #[test]
    fn unnameable_bucket_sums_until_a_null_appears() {
        let mut acc = observed(Some(0), Some(0));
        merge_observed(&mut acc, observed(Some(4), Some(1)));
        merge_observed(&mut acc, observed(Some(6), Some(2)));
        assert_eq!((acc.bytes, acc.count), (Some(10), Some(3)));

        merge_observed(&mut acc, observed(None, Some(1)));
        assert_eq!((acc.bytes, acc.count), (None, Some(4)));
    }

    #[test]
    fn scope_totals_include_inventory_false_nodes_and_flag_null() {
        let nodes = vec![
            node("attachments", RootScope::Data, Some(10)),
            node("logs", RootScope::Local, Some(7)),
            node("stray", RootScope::Data, Some(8)),
        ];
        let (data, partial) =
            scope_scan_totals(&nodes, RootScope::Data, &observed(Some(0), Some(0)));
        assert_eq!((data.scan_bytes, data.scan_count), (Some(18), Some(2)));
        assert_eq!(
            (data.unnameable_bytes, data.unnameable_count),
            (Some(0), Some(0))
        );
        assert!(!partial, "全部可观测时不是下界");

        let (_, partial) = scope_scan_totals(&nodes, RootScope::Local, &observed(Some(0), Some(0)));
        assert!(!partial);

        let (data, partial) = scope_scan_totals(&nodes, RootScope::Data, &observed(None, Some(0)));
        assert_eq!(
            data.scan_bytes,
            Some(18),
            "不可命名项为 null 时跳过累加（下界）"
        );
        assert!(partial, "不可命名项读不到 → totals 为下界");
    }

    #[test]
    fn null_node_scan_skips_accumulation_but_flags_partial() {
        let nodes = vec![
            node("attachments", RootScope::Data, Some(10)),
            node("truncated", RootScope::Data, None),
        ];
        let (data, partial) =
            scope_scan_totals(&nodes, RootScope::Data, &observed(Some(0), Some(0)));
        assert_eq!(data.scan_bytes, Some(10));
        assert!(partial);
    }
}
