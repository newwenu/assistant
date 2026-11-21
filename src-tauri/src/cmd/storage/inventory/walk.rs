//! 有界遍历扫描（§3.11.2）：**全仓库唯一允许对基根树 `read_dir` 的地方**。
//!
//! 只读、有界（深度 / 条目数 / 耗时），字节与条数同一次遍历产出（第 3 条，绝不分两次扫）。
//! 触界降级为 `null` 并记入 `truncatedNodes`；读不到只降级为 `null`，不算触界。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::app_dirs::registry::RootScope;
use crate::app_dirs::AppDirs;
use crate::cmd::storage::entries::EntryView;
use crate::protocol::dto::{StorageInventoryClassDto, StorageInventoryItemDto};

use super::classify::{classify, is_safe_segment};
use super::{RawNode, ScanResult};

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

/// 扫描整棵基根树的首层（§3.11.2）：清单里的首段按清单顺序在前，其余按名字排序在后。
///
/// 清单来自 [`EntryView`]（core 8 条 + 已挂载分区条目），所以「已登记首段」不必再查登记表。
pub fn scan(dirs: &AppDirs, views: &[EntryView]) -> std::io::Result<ScanResult> {
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
    for (name, path) in data_entries.iter() {
        if used.contains(name.as_str()) {
            continue;
        }
        nodes.push(build_node(name.clone(), path, RootScope::Data, views));
    }
    for (name, path) in local_entries.iter() {
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
    Ok(ScanResult {
        nodes,
        partial,
        truncated,
    })
}
