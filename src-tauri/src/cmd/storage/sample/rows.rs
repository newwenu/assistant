//! 各条目取数：每条只调 owner 的公开入口，不拼路径、不读配置。

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use assistant_storage::attachment_store::AttachmentEntry;
use assistant_tools::os::index_stats;
use assistant_tools::ToolResultStorage;

use crate::db_state::DbState;
use crate::protocol::dto::StorageAvailabilityDto;

use super::super::entries::EntrySample;
use super::policy::config_policy;
use super::stats::{from_dir_stats, from_io};

/// `E1`：与顶层字段同源，直接由调用方给的那份 `list_all` 结果派生。
///
/// 可回收量 = 孤儿（未被任何会话引用）的字节与条数：判据与 `build_base` 同源
/// （同一份 `entries` + 同一份引用集合），因此页面上「孤儿合计」与 `E1` 的
/// `reclaimableBytes` 恒等。引用集合取不到时留 `null` → 资格按不可算降级（fail-closed）。
///
/// 这里只算数字，不做资格判定：`eligible` / `reason` / 确认档位全在 `entries.rs` 按登记表
/// 与规格派生；执行时删不删由 owner 在附件 ID 锁内重算引用集合决定（§3.3），本函数不参与。
pub(super) fn attachment_sample(
    entries: &[AttachmentEntry],
    referenced: Option<&HashSet<String>>,
) -> EntrySample {
    let bytes = entries.iter().map(|entry| entry.bytes).sum();
    let count = u64::try_from(entries.len()).unwrap_or(u64::MAX);
    let oldest = entries
        .iter()
        .map(|entry| i64::try_from(entry.created_at).unwrap_or(i64::MAX))
        .min();
    let newest = entries
        .iter()
        .map(|entry| i64::try_from(entry.created_at).unwrap_or(i64::MAX))
        .max();
    let mut sample = EntrySample::observed(bytes, count, oldest, newest);
    if let Some(referenced) = referenced {
        let orphans = entries
            .iter()
            .filter(|entry| !referenced.contains(&entry.id));
        let orphan_bytes = orphans.clone().map(|entry| entry.bytes).sum();
        let orphan_count = u64::try_from(orphans.count()).unwrap_or(u64::MAX);
        sample.reclaimable = Some((orphan_bytes, orphan_count));
    }
    sample
}

pub(super) fn sample_tool_results(storage: &Arc<dyn ToolResultStorage>) -> EntrySample {
    let stats = match storage.result_stats() {
        None => return EntrySample::degraded(StorageAvailabilityDto::NotImplemented),
        Some(Ok(stats)) => stats,
        Some(Err(error)) => {
            log::debug!("工具结果统计失败: {error}");
            return EntrySample::degraded(StorageAvailabilityDto::Unreadable);
        }
    };
    let mut sample = from_dir_stats(stats);
    if let Some((days, effective_since)) = storage.result_retention() {
        sample = sample.with_policy(config_policy(days, effective_since));
        // 可回收量 = 过期候选的条数与字节：与执行侧同一判据、同一遍历（`collect_expired`）。
        if days > 0 {
            if let Some(candidates) = storage.collect_expired() {
                let bytes = candidates.iter().map(|candidate| candidate.bytes).sum();
                let count = u64::try_from(candidates.len()).unwrap_or(u64::MAX);
                sample.reclaimable = Some((bytes, count));
            }
        }
    }
    sample
}

pub(super) fn sample_index(root: &Path) -> EntrySample {
    match index_stats(root) {
        Ok(stats) => {
            let mut sample = from_dir_stats(stats);
            // `E4` 的可回收量就是全部索引文件：owner 判定「可重建」，管理器不自判。
            sample.reclaimable = Some((sample.bytes.unwrap_or(0), sample.count.unwrap_or(0)));
            sample
        }
        Err(error) => {
            log::debug!("索引目录统计失败: {error}");
            EntrySample::degraded(StorageAvailabilityDto::Unreadable)
        }
    }
}

/// `E2`：与 `derived_stats` 同源（`list_derived` 是唯一遍历点），孤儿即可回收量。
pub(super) fn derived_sample(
    entries: &[assistant_storage::attachment_store::DerivedEntry],
) -> EntrySample {
    let bytes = entries.iter().map(|entry| entry.bytes).sum();
    let count = u64::try_from(entries.len()).unwrap_or(u64::MAX);
    let oldest = entries.iter().filter_map(|entry| entry.modified_at).min();
    let newest = entries.iter().filter_map(|entry| entry.modified_at).max();
    let mut sample = EntrySample::observed(bytes, count, oldest, newest);
    let orphans = entries.iter().filter(|entry| entry.orphan);
    let orphan_bytes = orphans.clone().map(|entry| entry.bytes).sum();
    let orphan_count = u64::try_from(orphans.count()).unwrap_or(u64::MAX);
    sample.reclaimable = Some((orphan_bytes, orphan_count));
    sample
}

pub(super) fn sample_mascot(root: &Path) -> EntrySample {
    from_io(crate::cmd::mascot_asset::asset_stats(root))
}

pub(super) fn sample_crashes(root: &Path) -> EntrySample {
    from_io(assistant_crash::dump_stats(root))
}

/// `E12`：日志目录由 `tauri-plugin-log` 轮转，插件不暴露 stats API。
///
/// 目录本身是应用自有路径（`AppDirs::logs()`），因此走与 `E5` 相同的
/// [`crate::app_dirs::fs_stats::tree_stats`]（伴生文件同口径计入，见该模块说明）。
/// 回收仍归插件，管理器只观测。
pub(super) fn sample_logs(root: &Path) -> EntrySample {
    from_io(crate::app_dirs::fs_stats::tree_stats(root))
}

/// `E19`：shell 临时区（I73 §3.8）。目录由应用自持、会话结束时整棵删除，
/// 故与 `E12` 同口径走 `tree_stats`，只观测占用、不提供回收入口。
pub(super) fn sample_scratch(root: &Path) -> EntrySample {
    from_io(crate::app_dirs::fs_stats::tree_stats(root))
}

/// `E14`：WebView2 profile 整棵树由应用接管观测，可回收量只算缓存白名单子目录。
///
/// `bytes` 是整棵树（与扫描同一棵树 ⇒ `residualBytes` 只剩噪声量级）；`reclaimable`
/// 是缓存子目录求和——用户态部分（Cookies / Local Storage / IndexedDB）不删，因此
/// 不塞进 `residualBytes`，而是让「可回收 < 已记录」由两个量表达（§13.3.4）。
pub(super) fn sample_webview(profile: &Path) -> EntrySample {
    match crate::app_dirs::webview_profile::profile_stats(profile) {
        Ok((stats, cache)) => {
            let mut sample = from_dir_stats(stats);
            sample.reclaimable = Some(cache);
            sample
        }
        Err(error) => {
            log::debug!("WebView2 profile 统计失败: {error}");
            EntrySample::degraded(StorageAvailabilityDto::Unreadable)
        }
    }
}

/// `E15` / `E17` / `E18`：core 侧配置文件，只 `metadata`、不读内容、不开库（与 `E6` 同形）。
///
/// 文件不存在 → `file_stats` 返回全零 → [`from_io`] 折成 `missing` / `0 B`（`missing` 是
/// 合法状态，Linux / macOS 上 config 基根与数据基根分离，这些条目恒 `missing`，§13.4.4）；
/// 其余 IO 失败降级 `unreadable`，绝不把「不知道」写成 `0`。
pub(super) fn sample_config_file(path: &Path) -> EntrySample {
    from_io(crate::app_dirs::fs_stats::file_stats(path))
}

/// `E6`：主库只读 `metadata`；失败按 `unavailable`（文件正在替换 / 迁移，§7.1 特例 2）。
pub(super) fn sample_database(db: &DbState) -> EntrySample {
    match db.db_file_stats() {
        Ok(stats) => from_dir_stats(stats),
        Err(error) => {
            log::debug!("主库文件统计失败: {error}");
            EntrySample::degraded(StorageAvailabilityDto::Unavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, bytes: u64, created_at: u64) -> AttachmentEntry {
        AttachmentEntry {
            id: id.to_string(),
            bytes,
            created_at,
        }
    }

    #[test]
    fn attachment_sample_matches_list_all() {
        let entries = vec![entry("a.png", 10, 100), entry("b.pdf", 20, 200)];
        let sample = attachment_sample(&entries, None);
        assert_eq!(sample.bytes, Some(30));
        assert_eq!(sample.count, Some(2));
        assert_eq!(sample.oldest_at, Some(100));
        assert_eq!(sample.newest_at, Some(200));
        assert_eq!(sample.availability, StorageAvailabilityDto::Available);
    }

    /// `E1` 的可回收量只算孤儿：引用集合里的 ID 不计入。
    #[test]
    fn attachment_sample_reclaimable_counts_only_orphans() {
        let entries = vec![entry("a.png", 10, 100), entry("b.pdf", 20, 200)];
        let referenced: HashSet<String> = ["a.png".to_string()].into_iter().collect();
        let sample = attachment_sample(&entries, Some(&referenced));
        assert_eq!(sample.reclaimable, Some((20, 1)), "只算未被引用的那份");
        let none: HashSet<String> = HashSet::new();
        let sample = attachment_sample(&entries, Some(&none));
        assert_eq!(sample.reclaimable, Some((30, 2)), "全无引用时全部可回收");
    }

    #[test]
    fn empty_attachments_are_missing_not_degraded() {
        let sample = attachment_sample(&[], None);
        assert_eq!(sample.bytes, Some(0));
        assert_eq!(sample.count, Some(0));
        assert_eq!(sample.availability, StorageAvailabilityDto::Missing);
    }
}
