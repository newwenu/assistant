//! 建父子链 + 深度优先树序（I76 §5.3）。
//!
//! 只做三件事：按 `parent_pid` 建子节点索引、从根做前序 DFS、丢弃不可信边。
//! 不碰角色判定与数值（那是 classify / lib 的事）。

use std::collections::{HashMap, HashSet};

use crate::source::{RawProcess, RawSnapshot};
use crate::{MAX_DEPTH, MAX_ENTRIES};

/// 建树结果。
#[derive(Debug)]
pub struct TreeScan<'a> {
    /// 前序：父在子前、根在最前、`pid` 不重复。
    pub rows: Vec<&'a RawProcess>,
    /// 深度或条数触顶（I76 §5.3）。
    pub truncated: bool,
    /// 仅宿主线程数；取不到为 `None`，禁止填 `0`（I76 §5.3）。
    pub host_thread_count: Option<u32>,
}

/// 从 `root_pid` 建子树。
///
/// 返回 `None` 表示根进程不在快照里（宿主已退出或快照被过滤），由调用方转 `unreadable`。
pub fn collect(root_pid: u32, raw: &RawSnapshot) -> Option<TreeScan<'_>> {
    let mut root_threads = 0u32;
    let mut processes: HashMap<u32, &RawProcess> = HashMap::with_capacity(raw.processes.len());
    for process in &raw.processes {
        // 线程不进树；但它们仍算宿主的线程数（I76 §2.2 约束 1、§5.3）。
        if process.is_thread {
            if process.parent_pid == root_pid {
                root_threads += 1;
            }
            continue;
        }
        processes.entry(process.pid).or_insert(process);
    }

    let root = *processes.get(&root_pid)?;

    let mut children: HashMap<u32, Vec<&RawProcess>> = HashMap::new();
    for process in processes.values() {
        if process.pid == root_pid {
            continue;
        }
        children
            .entry(process.parent_pid)
            .or_default()
            .push(process);
    }
    // 同一父下的兄弟顺序必须稳定，否则树序随 HashMap 迭代随机变化。
    for siblings in children.values_mut() {
        siblings.sort_by_key(|process| (process.start_time, process.pid));
    }

    let mut rows: Vec<&RawProcess> = Vec::new();
    let mut visited: HashSet<u32> = HashSet::new();
    let mut truncated = false;
    let mut stack: Vec<(&RawProcess, usize)> = vec![(root, 0)];

    while let Some((node, depth)) = stack.pop() {
        if !visited.insert(node.pid) {
            continue;
        }
        if rows.len() >= MAX_ENTRIES {
            truncated = true;
            break;
        }
        rows.push(node);

        let Some(siblings) = children.get(&node.pid) else {
            continue;
        };
        if depth >= MAX_DEPTH {
            truncated = true;
            continue;
        }
        // 逆序入栈，出栈即为 (start_time, pid) 升序。
        for child in siblings.iter().rev() {
            // PID 复用防护：子进程创建时刻必须不早于父进程（I76 §5.3）。
            // 父 PID 在快照内由索引构造本身保证——键是父 PID，父不在快照则这条边无从到达。
            if child.start_time < node.start_time {
                continue;
            }
            stack.push((child, depth + 1));
        }
    }

    let host_thread_count = raw
        .host_thread_count
        .or_else(|| (root_threads > 0).then_some(root_threads));

    Some(TreeScan {
        rows,
        truncated,
        host_thread_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::RawProcess;

    fn snapshot(processes: Vec<RawProcess>) -> RawSnapshot {
        RawSnapshot {
            sampled_at: 1_000,
            processes,
            host_thread_count: None,
        }
    }

    fn pids(scan: &TreeScan<'_>) -> Vec<u32> {
        scan.rows.iter().map(|process| process.pid).collect()
    }

    /// V8：父在子前、根在最前、`pid` 不重复（I76 §10.2 V8）。
    #[test]
    fn v8_preorder_and_unique_pids() {
        let raw = snapshot(vec![
            RawProcess::new(100, 1, 50, "host.exe"),
            RawProcess::new(200, 100, 60, "child.exe"),
            RawProcess::new(300, 200, 70, "grandchild.exe"),
            RawProcess::new(210, 100, 65, "child2.exe"),
        ]);
        let scan = collect(100, &raw).expect("root present");
        assert_eq!(pids(&scan), vec![100, 200, 300, 210]);
        assert!(!scan.truncated);
    }

    /// V9：同一 `pid` 只出现一次（I76 §10.2 V9）。
    #[test]
    fn v9_duplicate_pid_appears_once() {
        let raw = snapshot(vec![
            RawProcess::new(100, 1, 50, "host.exe"),
            RawProcess::new(200, 100, 60, "child.exe"),
            RawProcess::new(200, 100, 60, "child.exe"),
            RawProcess::new(200, 100, 60, "child.exe"),
        ]);
        let scan = collect(100, &raw).expect("root present");
        assert_eq!(pids(&scan), vec![100, 200]);
    }

    /// V10：同名但不在子树内的进程不得入树（I76 §10.2 V10）。
    #[test]
    fn v10_same_name_outside_subtree_excluded() {
        let raw = snapshot(vec![
            RawProcess::new(100, 1, 50, "host.exe"),
            RawProcess::new(200, 100, 60, "msedgewebview2.exe"),
            RawProcess::new(900, 42, 60, "msedgewebview2.exe"),
        ]);
        let scan = collect(100, &raw).expect("root present");
        assert_eq!(pids(&scan), vec![100, 200]);
    }

    /// V11：PID 复用（子早于父）与父不在快照，两条边都要丢弃（I76 §10.2 V11）。
    #[test]
    fn v11_reused_pid_and_missing_parent_excluded() {
        let raw = snapshot(vec![
            RawProcess::new(100, 1, 50, "host.exe"),
            // 父 100 现在创建于 50，但这条 200 创建于 20：旧 PID 100 的残留子进程。
            RawProcess::new(200, 100, 20, "stale.exe"),
            // 父 700 不在快照内：这条边无从到达。
            RawProcess::new(300, 700, 80, "orphan.exe"),
        ]);
        let scan = collect(100, &raw).expect("root present");
        assert_eq!(pids(&scan), vec![100]);
    }

    /// 根不在快照：返回 `None`，由调用方转 `unreadable`。
    #[test]
    fn missing_root_returns_none() {
        let raw = snapshot(vec![RawProcess::new(200, 100, 60, "child.exe")]);
        assert!(collect(100, &raw).is_none());
    }

    /// 线程条目被过滤，但计入宿主线程数（I76 §5.3）。
    #[test]
    fn threads_filtered_but_counted() {
        let raw = snapshot(vec![
            RawProcess::new(100, 1, 50, "host.exe"),
            RawProcess::thread(101, 100, 55, "worker"),
            RawProcess::thread(102, 100, 55, "worker"),
        ]);
        let scan = collect(100, &raw).expect("root present");
        assert_eq!(pids(&scan), vec![100]);
        assert_eq!(scan.host_thread_count, Some(2));
    }

    /// 平台直接给出线程数时优先用平台的（Linux `tasks()` 长度口径）。
    #[test]
    fn provided_host_threads_win() {
        let mut raw = snapshot(vec![
            RawProcess::new(100, 1, 50, "host.exe"),
            RawProcess::thread(101, 100, 55, "worker"),
        ]);
        raw.host_thread_count = Some(7);
        let scan = collect(100, &raw).expect("root present");
        assert_eq!(scan.host_thread_count, Some(7));
    }

    /// 深度触顶：`truncated = true`，且不再下钻（I76 §5.3）。
    #[test]
    fn depth_limit_marks_truncated() {
        let mut processes = vec![RawProcess::new(100, 1, 50, "host.exe")];
        for depth in 1..=(MAX_DEPTH + 2) {
            let pid = 100 + depth as u32;
            processes.push(RawProcess::new(pid, pid - 1, 50 + depth as i64, "deep.exe"));
        }
        let raw = snapshot(processes);
        let scan = collect(100, &raw).expect("root present");
        assert_eq!(scan.rows.len(), MAX_DEPTH + 1);
        assert!(scan.truncated);
    }

    /// 条数触顶：`truncated = true`（I76 §5.3）。
    #[test]
    fn entry_limit_marks_truncated() {
        let mut processes = vec![RawProcess::new(100, 1, 50, "host.exe")];
        for index in 0..MAX_ENTRIES {
            let pid = 1_000 + index as u32;
            processes.push(RawProcess::new(pid, 100, 60, "child.exe"));
        }
        let raw = snapshot(processes);
        let scan = collect(100, &raw).expect("root present");
        assert_eq!(scan.rows.len(), MAX_ENTRIES);
        assert!(scan.truncated);
    }
}
