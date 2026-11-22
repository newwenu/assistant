//! `E14` EBWebView 的接管动作（I67 §13.3）：缓存白名单、待执行标记、启动期清理。
//!
//! 为什么需要「待执行」：WebView2 运行期持有 profile 文件句柄，运行期删除会失败或
//! 损坏 profile。唯一安全时机是任何 webview 创建之前（`Builder::run` 之前），因此
//! 运行期的清理请求只写标记，启动时消费标记再删（§13.3.5）。
//!
//! 标记文件放在 profile 目录内、后缀 `.tmp`：该目录已登记（`E14`），且 `.tmp` 属
//! 伴生文件，不计入 `tree_stats` 的 `bytes` / `count`，不会让统计多出一份无主字节。

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

/// 启动期清理结果：与回收结果的 `Totals` 同形，由命令层折算成 `EntryResult`。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PurgeReport {
    pub removed: u64,
    pub removed_bytes: u64,
    pub failed: u64,
    pub skipped: u64,
}

/// 可回收量：白名单子目录的字节与条数（与 [`purge_cache`] 同一份清单、同一口径）。
pub fn cache_totals(profile: &Path) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut count = 0u64;
    for sub in CACHE_SUBDIRS {
        match super::fs_stats::tree_stats(&profile.join(sub)) {
            Ok(stats) => {
                bytes = bytes.saturating_add(stats.bytes);
                count = count.saturating_add(stats.count);
            }
            Err(error) => log::debug!("webview cache '{}' stats failed: {error}", sub),
        }
    }
    (bytes, count)
}

pub fn pending_marker(profile: &Path) -> PathBuf {
    profile.join(PENDING_MARKER)
}

/// 运行期登记「待执行」：只写标记，不动任何文件（§13.3.5）。
pub fn mark_pending(profile: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(profile)?;
    std::fs::write(pending_marker(profile), b"pending\n")
}

/// 启动期消费标记：有标记才清理，清完删标记；无标记返回 `None`（零副作用）。
pub fn run_pending_purge(profile: &Path) -> Option<PurgeReport> {
    if !pending_marker(profile).exists() {
        return None;
    }
    let report = purge_cache(profile);
    if let Err(error) = std::fs::remove_file(pending_marker(profile)) {
        log::warn!("Failed to remove webview reclaim marker: {error}");
    }
    Some(report)
}

/// 删除白名单缓存子目录：逐项校验存在、失败如实计数、单条失败不影响其余。
pub fn purge_cache(profile: &Path) -> PurgeReport {
    let mut report = PurgeReport::default();
    for sub in CACHE_SUBDIRS {
        let path = profile.join(sub);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            Err(error) => {
                log::warn!("webview cache '{}' stat failed: {error}", path.display());
                report.failed = report.failed.saturating_add(1);
                continue;
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
    fn cache_totals_counts_only_whitelisted_subdirs() {
        let profile = unique_dir("totals");
        write_file(&profile.join("Default/Cache/data.bin"), 10);
        write_file(&profile.join("GPUCache/blob.bin"), 20);
        write_file(&profile.join("Default/Cookies"), 100);
        assert_eq!(cache_totals(&profile), (30, 2), "用户态文件不计入可回收量");
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
        let report = run_pending_purge(&profile).expect("有标记即清理");
        assert_eq!(report.removed, 1);
        assert!(!pending_marker(&profile).exists(), "标记消费后删除");
        assert_eq!(run_pending_purge(&profile), None, "标记只消费一次");
        std::fs::remove_dir_all(&profile).ok();
    }
}
