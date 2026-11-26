//! `E14` EBWebView 的接管动作（I67 §13.3）：缓存白名单、待执行标记、启动期清理。
//!
//! 为什么需要「待执行」：WebView2 运行期持有 profile 文件句柄，运行期删除会失败或
//! 损坏 profile。唯一安全时机是任何 webview 创建之前（`Builder::run` 之前），因此
//! 运行期的清理请求只写标记，启动时消费标记再删（§13.3.5）。
//!
//! 标记文件放在 profile 目录内、后缀 `.tmp`：该目录已登记（`E14`），而 `tree_stats`
//! 对伴生文件与普通文件同口径计数，所以标记也计入 `E14` 的 `bytes` / `count`——它是
//! 管理器自己写的真实文件（待清理子目录清单），只存在到清单推进完为止，不产生无主字节。

use std::path::{Path, PathBuf};

/// 可重建的缓存子目录（相对 profile 根）；用户态部分（Cookies / Local Storage /
/// IndexedDB / Login Data）**不在**此列，一律不删（§13.3.4）。
const CACHE_SUBDIRS: &[&str] = &[
    "BrowserMetrics",
    "Crashpad",
    "GrShaderCache",
    "GPUCache",
    "GraphiteDawnCache",
    "ShaderCache",
    "component_crx_cache",
    "extensions_crx_cache",
    "Default/Cache",
    "Default/Code Cache",
    "Default/GPUCache",
    "Default/DawnGraphiteCache",
    "Default/DawnWebGPUCache",
];

const PENDING_MARKER: &str = "reclaim-pending.tmp";

/// 单次启动最多清理的子目录数：profile 常有上万文件，整目录删除会拉长
/// `run_pending_purge` 的阻塞时间（`main.rs` 在 `Builder::run` 之前同步调用）。
/// 剩余子目录写回标记，逐次启动推进（§14 R2）；整目录清理仍由设置页手动入口提供。
const PURGE_PER_BOOT: usize = 4;

/// 启动期清理结果：与回收结果的 `Totals` 同形，由命令层折算成 `EntryResult`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PurgeReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
    pub skipped: u64,
}

/// 观测：一次遍历同时给出整棵树与缓存白名单子目录（与 [`purge_cache`] 同一份清单、同一口径）。
///
/// 为什么合并成一次遍历：profile 常有上万文件（缓存占绝大多数），分两次走会把元数据
/// 访问翻倍；500 ms 采样预算下（`sample::timeout`）极易超时降级为 `unavailable`。
/// 两个量同源同刻，也顺带消除「整棵树」与「可回收」之间的跨遍历漂移。
pub fn profile_stats(profile: &Path) -> std::io::Result<(super::fs_stats::DirStats, (u64, u64))> {
    let mut cache = (0u64, 0u64);
    let stats = super::fs_stats::tree_stats_with(profile, &mut |path, meta| {
        let Ok(relative) = path.strip_prefix(profile) else {
            return;
        };
        // 只认白名单子目录**内部**的文件：与 `tree_stats(&profile.join(sub))` 等价
        // （同名但落在根上的普通文件在那边会因 `read_dir` 失败而被跳过）。
        if CACHE_SUBDIRS
            .iter()
            .any(|sub| relative.starts_with(sub) && relative != Path::new(sub))
        {
            cache.0 = cache.0.saturating_add(meta.len());
            cache.1 = cache.1.saturating_add(1);
        }
    })?;
    Ok((stats, cache))
}

pub fn pending_marker(profile: &Path) -> PathBuf {
    profile.join(PENDING_MARKER)
}

/// 运行期登记「待执行」：只写标记，不动任何文件（§13.3.5）。
///
/// 标记内容是待清理子目录清单。已有标记时不覆盖：上一次启动的进度（剩余清单）
/// 比重新排队全部子目录更准确。
pub fn mark_pending(profile: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(profile)?;
    let marker = pending_marker(profile);
    if marker.exists() {
        return Ok(());
    }
    std::fs::write(marker, CACHE_SUBDIRS.join("\n"))
}

/// 启动期消费标记：有标记才清理，限量清理 `PURGE_PER_BOOT` 项后把剩余写回标记；
/// 清单清空时删标记。无标记返回 `None`（零副作用）。
pub fn run_pending_purge(profile: &Path) -> Option<PurgeReport> {
    let marker = pending_marker(profile);
    if !marker.exists() {
        return None;
    }
    let pending = pending_subdirs(&marker);
    let mut report = PurgeReport::default();
    let mut leftover: Vec<&str> = Vec::new();
    let mut purged = 0usize;
    for sub in pending {
        if purged >= PURGE_PER_BOOT {
            leftover.push(sub);
            continue;
        }
        merge(&mut report, purge_subdir(profile, sub));
        purged = purged.saturating_add(1);
    }
    if leftover.is_empty() {
        if let Err(error) = std::fs::remove_file(&marker) {
            log::warn!("Failed to remove webview reclaim marker: {error}");
        }
    } else if let Err(error) = std::fs::write(&marker, leftover.join("\n")) {
        log::warn!("Failed to rewrite webview reclaim marker: {error}");
    }
    Some(report)
}

/// 读标记里的待清理清单。内容为空或全不认识的（如旧版单行 `pending`）回退为完整清单，
/// 保证标记一旦存在就一定有可推进的工作，不会留下永远清不掉的空标记。
fn pending_subdirs(marker: &Path) -> Vec<&'static str> {
    let text = std::fs::read_to_string(marker).unwrap_or_default();
    let known: Vec<&'static str> = CACHE_SUBDIRS
        .iter()
        .copied()
        .filter(|sub| text.lines().any(|line| line.trim() == *sub))
        .collect();
    if known.is_empty() {
        CACHE_SUBDIRS.to_vec()
    } else {
        known
    }
}

fn merge(into: &mut PurgeReport, from: PurgeReport) {
    into.removed = into.removed.saturating_add(from.removed);
    into.removed_bytes = into.removed_bytes.saturating_add(from.removed_bytes);
    into.failed = into.failed.saturating_add(from.failed);
    into.skipped = into.skipped.saturating_add(from.skipped);
}

/// 删除白名单缓存子目录：逐项校验存在、失败如实计数、单条失败不影响其余。
pub fn purge_cache(profile: &Path) -> PurgeReport {
    let mut report = PurgeReport::default();
    for sub in CACHE_SUBDIRS {
        merge(&mut report, purge_subdir(profile, sub));
    }
    report
}

fn purge_subdir(profile: &Path, sub: &str) -> PurgeReport {
    let mut report = PurgeReport::default();
    let path = profile.join(sub);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            report.skipped = report.skipped.saturating_add(1);
            return report;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.skipped = report.skipped.saturating_add(1);
            return report;
        }
        Err(error) => {
            log::warn!("webview cache '{}' stat failed: {error}", path.display());
            report.failed = report.failed.saturating_add(1);
            return report;
        }
    }
    let bytes = super::fs_stats::tree_stats(&path)
        .map(|stats| stats.bytes)
        .unwrap_or(0);
    match std::fs::remove_dir_all(&path) {
        Ok(()) => {
            report.removed = report.removed.saturating_add(1);
            report.removed_bytes = report.removed_bytes.saturating_add(bytes);
        }
        Err(error) => {
            log::warn!("webview cache '{}' removal failed: {error}", path.display());
            report.failed = report.failed.saturating_add(1);
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn unique_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("assistant-webview-{}-{}", name, nanos))
    }

    fn write_file(path: &Path, bytes: usize) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("父目录可建");
        }
        std::fs::write(path, vec![0u8; bytes]).expect("测试文件可写");
    }

    #[test]
    fn profile_stats_splits_cache_from_user_state_in_one_pass() {
        let profile = unique_dir("totals");
        write_file(&profile.join("Default/Cache/data.bin"), 10);
        write_file(&profile.join("GPUCache/blob.bin"), 20);
        write_file(&profile.join("Default/Cookies"), 100);
        let (stats, cache) = profile_stats(&profile).expect("扫描应成功");
        assert_eq!(stats.bytes, 130, "整棵树含用户态文件");
        assert_eq!(stats.count, 3);
        assert_eq!(cache, (30, 2), "可回收量只算白名单子目录");
        std::fs::remove_dir_all(&profile).ok();
    }

    #[test]
    fn profile_stats_missing_profile_is_empty_not_error() {
        let profile = unique_dir("missing");
        let (stats, cache) = profile_stats(&profile).expect("缺失目录应为空态");
        assert_eq!(stats.bytes, 0);
        assert_eq!(stats.count, 0);
        assert_eq!(cache, (0, 0));
    }

    #[test]
    fn profile_stats_counts_companion_files_in_both_views() {
        let profile = unique_dir("companion");
        write_file(&profile.join("Default/Cache/data.bin"), 10);
        write_file(&profile.join("Default/Cache/data.bin.tmp"), 7);
        let (stats, cache) = profile_stats(&profile).expect("扫描应成功");
        assert_eq!(stats.bytes, 17, "伴生文件与普通文件同口径计入整棵树");
        assert_eq!(cache, (17, 2), "白名单子目录内的伴生文件同样可回收");
        std::fs::remove_dir_all(&profile).ok();
    }

    #[test]
    fn profile_stats_counts_pending_marker() {
        let profile = unique_dir("marker");
        mark_pending(&profile).expect("标记可写");
        let marker_len = std::fs::metadata(pending_marker(&profile))
            .expect("标记存在")
            .len();
        let (stats, cache) = profile_stats(&profile).expect("扫描应成功");
        assert_eq!(stats.count, 1, "标记是 profile 内的真实文件");
        assert_eq!(stats.bytes, marker_len);
        assert_eq!(cache, (0, 0), "标记不在白名单子目录，不计入可回收");
        std::fs::remove_dir_all(&profile).ok();
    }

    #[test]
    fn purge_removes_cache_and_keeps_user_state() {
        let profile = unique_dir("purge");
        write_file(&profile.join("Default/Cache/data.bin"), 10);
        write_file(&profile.join("Default/Cookies"), 100);
        write_file(&profile.join("Local State"), 5);
        let report = purge_cache(&profile);
        assert_eq!(report.removed, 1);
        assert_eq!(report.removed_bytes, 10);
        assert!(profile.join("Default/Cookies").exists(), "用户态不得删除");
        assert!(profile.join("Local State").exists(), "用户态不得删除");
        assert!(!profile.join("Default/Cache").exists(), "缓存应已删除");
        std::fs::remove_dir_all(&profile).ok();
    }

    #[test]
    fn pending_marker_gates_purge() {
        let profile = unique_dir("pending");
        assert_eq!(run_pending_purge(&profile), None, "无标记时零副作用");
        write_file(&profile.join("GPUCache/blob.bin"), 8);
        mark_pending(&profile).expect("标记可写");
        let mut removed = 0;
        let mut rounds = 0;
        while pending_marker(&profile).exists() {
            let report = run_pending_purge(&profile).expect("有标记即清理");
            removed += report.removed;
            rounds += 1;
            assert!(
                rounds < CACHE_SUBDIRS.len() + 2,
                "限量清理应在有限轮次内推进完"
            );
        }
        assert_eq!(removed, 1);
        assert!(!profile.join("GPUCache").exists(), "缓存应已删除");
        assert_eq!(run_pending_purge(&profile), None, "标记只消费一次");
        std::fs::remove_dir_all(&profile).ok();
    }

    #[test]
    fn purge_budget_advances_across_rounds() {
        let profile = unique_dir("budget");
        for sub in ["GPUCache", "ShaderCache", "GrShaderCache"] {
            write_file(&profile.join(sub).join("blob.bin"), 4);
        }
        mark_pending(&profile).expect("标记可写");
        let first = run_pending_purge(&profile).expect("有标记即清理");
        assert!(
            first.removed <= PURGE_PER_BOOT as u64,
            "单次启动清理数不得超过限量：{}",
            first.removed
        );
        let text = std::fs::read_to_string(pending_marker(&profile)).expect("剩余清单可读");
        let leftover = text.lines().filter(|line| !line.trim().is_empty()).count();
        assert_eq!(
            leftover,
            CACHE_SUBDIRS.len() - PURGE_PER_BOOT,
            "剩余清单应等于未推进的项数"
        );
        std::fs::remove_dir_all(&profile).ok();
    }
}
