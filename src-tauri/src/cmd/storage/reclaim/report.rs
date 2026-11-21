//! 结果报告装配（§5.3）：失败原因只报类别 + 通用文案。

use crate::protocol::dto::{
    StorageReclaimActionDto, StorageReclaimEntryResultDto, StorageReclaimErrorCodeDto,
    StorageReclaimErrorDto, StorageReclaimStatusDto,
};

/// 单个条目的 `errors` 上限（§5.3 语义硬约束 3）。
const MAX_ERRORS: usize = 5;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Totals {
    pub(super) removed: u64,
    pub(super) removed_bytes: u64,
    pub(super) failed: u64,
    pub(super) skipped: u64,
}

pub(super) fn status_of(totals: Totals) -> StorageReclaimStatusDto {
    if totals.failed > 0 {
        if totals.removed > 0 {
            StorageReclaimStatusDto::Partial
        } else {
            StorageReclaimStatusDto::Failed
        }
    } else if totals.removed > 0 {
        StorageReclaimStatusDto::Done
    } else {
        StorageReclaimStatusDto::Skipped
    }
}

/// 失败原因只报类别 + 通用文案，不带路径与文件名（§5.3 硬约束 3）。
pub(super) fn failure_errors(
    failed: u64,
    read_failures: u64,
    delete_message: &str,
    read_message: &str,
) -> Vec<StorageReclaimErrorDto> {
    let mut errors = Vec::new();
    if failed.saturating_sub(read_failures) > 0 {
        errors.push(error_dto(StorageReclaimErrorCodeDto::Io, delete_message));
    }
    if read_failures > 0 {
        errors.push(error_dto(StorageReclaimErrorCodeDto::Io, read_message));
    }
    errors
}

pub(super) fn error_dto(
    code: StorageReclaimErrorCodeDto,
    message: impl Into<String>,
) -> StorageReclaimErrorDto {
    StorageReclaimErrorDto {
        code,
        message: message.into(),
    }
}

pub(super) fn entry_result(
    key: String,
    action: StorageReclaimActionDto,
    totals: Totals,
    errors: Vec<StorageReclaimErrorDto>,
    status: StorageReclaimStatusDto,
) -> StorageReclaimEntryResultDto {
    StorageReclaimEntryResultDto {
        key,
        action,
        status,
        removed_count: totals.removed,
        removed_bytes: totals.removed_bytes,
        failed_count: totals.failed,
        skipped_count: totals.skipped,
        errors: errors.into_iter().take(MAX_ERRORS).collect(),
    }
}
