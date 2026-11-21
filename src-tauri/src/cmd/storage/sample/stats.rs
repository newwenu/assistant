//! 观测结构适配：把各 crate 同形状的 stats 折算成 `EntrySample`。
//!
//! IO 错误细节只进日志、不上 wire：可用性由封闭枚举 `availability` 表达（R-16）。

use crate::protocol::dto::StorageAvailabilityDto;

use super::super::entries::EntrySample;

pub(super) fn from_dir_stats(stats: impl StatsView) -> EntrySample {
    EntrySample::observed(
        stats.bytes(),
        stats.count(),
        stats.oldest_at(),
        stats.newest_at(),
    )
}

pub(super) fn from_io<T: StatsView>(result: std::io::Result<T>) -> EntrySample {
    match result {
        Ok(stats) => from_dir_stats(stats),
        Err(error) => {
            log::debug!("目录统计失败: {error}");
            EntrySample::degraded(StorageAvailabilityDto::Unreadable)
        }
    }
}

/// 各 crate 的同形状观测结构（避免管理器依赖它们的类型）统一取值。
pub(super) trait StatsView {
    fn bytes(&self) -> u64;
    fn count(&self) -> u64;
    fn oldest_at(&self) -> Option<i64>;
    fn newest_at(&self) -> Option<i64>;
}

macro_rules! impl_stats_view {
    ($ty:path) => {
        impl StatsView for $ty {
            fn bytes(&self) -> u64 {
                self.bytes
            }
            fn count(&self) -> u64 {
                self.count
            }
            fn oldest_at(&self) -> Option<i64> {
                self.oldest_at
            }
            fn newest_at(&self) -> Option<i64> {
                self.newest_at
            }
        }
    };
}

impl_stats_view!(crate::app_dirs::fs_stats::DirStats);
impl_stats_view!(assistant_tools::DirStats);
impl_stats_view!(assistant_crash::DirStats);

/// `E2`：回收错误的人类可读文案（`StorageReclaimErrorDto.message` 是允许的散文字段）。
pub fn attachment_reason(error: &assistant_storage::attachment_store::AttachmentError) -> String {
    match error {
        assistant_storage::attachment_store::AttachmentError::Io(io) => {
            let text = match io.kind() {
                std::io::ErrorKind::PermissionDenied => "没有访问权限",
                std::io::ErrorKind::NotFound => "文件不存在",
                std::io::ErrorKind::StorageFull => "磁盘空间不足",
                std::io::ErrorKind::InvalidData => "文件内容异常",
                _ => "读取失败",
            };
            text.to_string()
        }
        _ => "文件状态异常".to_string(),
    }
}
