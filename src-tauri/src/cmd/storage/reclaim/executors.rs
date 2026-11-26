//! 各条目执行器（§5.1）：只调 owner 方法，自己不删文件。

use std::sync::Arc;

use assistant_storage::attachment_store::{self, AttachmentStore};
use assistant_tools::{DeleteReport, ExpiredCandidate, ToolResultStorage, ToolScratchStore};

use crate::app_dirs::AppDirs;
use crate::cmd::storage::section::{SectionError, StorageSectionHandle};
use crate::db_state::DbState;
use crate::protocol::dto::{
    ReclaimPolicyDto, StorageReclaimActionDto, StorageReclaimEntryResultDto,
    StorageReclaimErrorCodeDto, StorageReclaimStatusDto,
};

use super::super::sample::attachment_reason;
use super::report::{entry_result, error_dto, failure_errors, status_of, Totals};

/// `E1`：执行时按 owner 重算孤儿集合（不认计划里的数量），逐个在 ID 锁内查引用再删。
///
/// 三条纪律：①引用集合在 `spawn_blocking` 内、持 DB 连接时重算（§5.2 第 4 条），
/// 计划里的孤儿数只用于预览；②`mtime` 不早于 `sampledAt` 的文件由 owner 跳过
/// （挡住「采样后新导入、引用尚未落库」的窗口，§5.2 第 5 条）；③锁顺序恒为
/// 「DB 连接锁 → 附件 ID 锁」，与 `delete_attachments` / 引用写入一致，不会 ABBA。
pub(super) async fn execute_orphans(
    store: AttachmentStore,
    db: Option<DbState>,
    sampled_at: i64,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let Some(db) = db else {
        return entry_result(
            key,
            action,
            Totals::default(),
            vec![error_dto(
                StorageReclaimErrorCodeDto::OwnerUnavailable,
                "数据库暂不可用",
            )],
            StorageReclaimStatusDto::Failed,
        );
    };
    let entries = match store.list_all().await {
        Ok(entries) => entries,
        Err(error) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::OwnerUnavailable,
                    attachment_reason(&error),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let work = move || -> Result<assistant_storage::attachment_store::OrphanPurgeReport, String> {
        let conn = db.0.lock().map_err(|error| error.to_string())?;
        let referenced: std::collections::HashSet<String> =
            attachment_store::get_all_referenced_attachment_ids(&conn)?
                .into_iter()
                .collect();
        let orphans: Vec<String> = entries
            .into_iter()
            .filter(|entry| !referenced.contains(&entry.id))
            .map(|entry| entry.id)
            .collect();
        Ok(store.delete_orphans_by_ids(&conn, &orphans, sampled_at))
    };
    let report = match tauri::async_runtime::spawn_blocking(work).await {
        Ok(Ok(report)) => report,
        Ok(Err(_)) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Io,
                    "读取附件引用失败",
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
        Err(_) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Unknown,
                    "清理任务出错".to_string(),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed,
        skipped: report.skipped,
    };
    let errors = failure_errors(totals.failed, 0, "部分孤儿附件删除失败", "附件目录读取失败");
    entry_result(key, action, totals, errors, status_of(totals))
}

/// `E2`：执行时按 owner 重算孤儿（不认计划里的名字），只删「采样前就已存在」的。
pub(super) async fn execute_derived(
    store: &AttachmentStore,
    sampled_at: i64,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let orphans: Vec<String> = match store.list_derived().await {
        Ok(entries) => entries
            .into_iter()
            .filter(|entry| entry.orphan)
            .map(|entry| entry.name)
            .collect(),
        Err(error) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::OwnerUnavailable,
                    attachment_reason(&error),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let report = store.purge_derived_orphans(&orphans, sampled_at).await;
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed,
        skipped: report.skipped,
    };
    let errors = failure_errors(totals.failed, 0, "部分派生文件删除失败", "派生目录读取失败");
    entry_result(key, action, totals, errors, status_of(totals))
}

/// `E4`：全量重建缓存，删掉索引根下全部 `*.idx`（与 `index_stats` 同一遍历）。
pub(super) async fn execute_index(
    dirs: &AppDirs,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let root = dirs.indexes();
    let report = match tauri::async_runtime::spawn_blocking(move || {
        assistant_tools::os::purge_index_files(&root)
    })
    .await
    {
        Ok(report) => report,
        Err(_) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Unknown,
                    "清理任务出错".to_string(),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed.saturating_add(report.read_failures),
        skipped: report.skipped,
    };
    let errors = failure_errors(
        totals.failed,
        report.read_failures,
        "部分索引文件删除失败",
        "索引目录读取失败",
    );
    entry_result(key, action, totals, errors, status_of(totals))
}

/// `E3`：按计划时刻的保留天数收集过期候选（带字节）并逐个删除。
///
/// 三条纪律：①计划生成后策略被改（§7.3 第 4 条）→ `skipped`，不按旧策略删；
/// ②候选的字节在收集时取，删除时不再重算（删完取不到，§3.10 接口形状 1）；
/// ③「文件已不存在」算 `skipped` 不算失败，单条失败不影响其余（§5.1 第 2、3 条）。
pub(super) async fn execute_tool_results(
    storage: Option<Arc<dyn ToolResultStorage>>,
    planned_policy: Option<ReclaimPolicyDto>,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let Some(storage) = storage else {
        return entry_result(
            key,
            action,
            Totals::default(),
            vec![error_dto(
                StorageReclaimErrorCodeDto::OwnerUnavailable,
                "工具功能未启用",
            )],
            StorageReclaimStatusDto::Failed,
        );
    };
    let planned_days = planned_policy
        .and_then(|policy| policy.retention_days)
        .unwrap_or(0);
    let current_days = storage
        .result_retention()
        .map(|(days, _)| days)
        .unwrap_or(0);
    // 策略禁用（`0`）或与计划快照不一致：零副作用跳过，绝不按旧天数删。
    if planned_days == 0 || planned_days != current_days {
        return entry_result(
            key,
            action,
            Totals::default(),
            Vec::new(),
            StorageReclaimStatusDto::Skipped,
        );
    }

    let work = move || -> Option<(Vec<ExpiredCandidate>, DeleteReport)> {
        let candidates = storage.collect_expired()?;
        let report = storage.delete_candidates(&candidates)?;
        Some((candidates, report))
    };
    let report = match tauri::async_runtime::spawn_blocking(work).await {
        Ok(Some((_, report))) => report,
        Ok(None) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::OwnerUnavailable,
                    "该存储项暂不支持清理",
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
        Err(_) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Unknown,
                    "清理任务出错".to_string(),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed,
        skipped: report.skipped,
    };
    let errors = failure_errors(
        totals.failed,
        0,
        "部分工具结果删除失败",
        "工具结果目录读取失败",
    );
    entry_result(key, action, totals, errors, status_of(totals))
}

/// 可选分区条目：把 `key + action + 策略快照` 原样交给声明它的分区，core 不认识条目语义。
///
/// 三条纪律：①策略只从计划快照取，管理器不填默认值（§3.1）；
/// ②重新计数与删除都在 owner 侧一次完成（§5.2 第 4 条），计划后新增的文件不会被误删；
/// ③分区回 `None`（不支持该 `key + action`）→ `NotEligible`，不猜行为。
pub(super) async fn execute_section(
    section: StorageSectionHandle,
    planned_policy: Option<ReclaimPolicyDto>,
    key: &'static str,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let work = move || section.reclaim(key, action, planned_policy);
    let report = match tauri::async_runtime::spawn_blocking(work).await {
        Ok(Some(Ok(report))) => report,
        Ok(Some(Err(error))) => {
            let (code, message) = match error {
                SectionError::Locked | SectionError::Unavailable => (
                    StorageReclaimErrorCodeDto::OwnerUnavailable,
                    "分区当前不可用，请稍后重试",
                ),
                SectionError::Unreadable => (StorageReclaimErrorCodeDto::Io, "分区读取失败"),
            };
            log::error!("分区回收失败 {key}: {error:?}");
            return entry_result(
                key.to_string(),
                action,
                Totals::default(),
                vec![error_dto(code, message)],
                StorageReclaimStatusDto::Failed,
            );
        }
        Ok(None) => {
            return entry_result(
                key.to_string(),
                action,
                Totals::default(),
                Vec::new(),
                StorageReclaimStatusDto::NotEligible,
            );
        }
        Err(_) => {
            return entry_result(
                key.to_string(),
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Unknown,
                    "清理任务出错".to_string(),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed,
        skipped: 0,
    };
    let errors = failure_errors(totals.failed, 0, "部分文件删除失败", "目录读取失败");
    entry_result(key.to_string(), action, totals, errors, status_of(totals))
}

/// `E14`：运行期不删——只登记「待执行」，由下次启动在 `Builder::run` 之前消费。
///
/// WebView2 运行中持有 profile 文件句柄，运行期 `remove_dir_all` 会失败或损坏 profile，
/// 因此本执行器**只写标记、零删除**（§13.3.5）。返回 `scheduled`，让前端如实显示
/// 「已登记，重启后执行」，而不是假装删过（`done`）或静默吞掉请求（`skipped`）。
pub(super) async fn execute_webview(
    dirs: &AppDirs,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let profile = dirs.webview_profile();
    let written = tauri::async_runtime::spawn_blocking(move || {
        crate::app_dirs::webview_profile::mark_pending(&profile)
    })
    .await;
    match written {
        Ok(Ok(())) => entry_result(
            key,
            action,
            Totals::default(),
            Vec::new(),
            StorageReclaimStatusDto::Scheduled,
        ),
        Ok(Err(error)) => {
            log::warn!("WebView2 清理标记写入失败: {error}");
            entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Io,
                    "无法登记待执行的清理请求",
                )],
                StorageReclaimStatusDto::Failed,
            )
        }
        Err(_) => entry_result(
            key,
            action,
            Totals::default(),
            vec![error_dto(
                StorageReclaimErrorCodeDto::Unknown,
                "清理任务出错".to_string(),
            )],
            StorageReclaimStatusDto::Failed,
        ),
    }
}

/// `E19`：一键清空整根临时区（I77 §4.1）。
///
/// 三条纪律：①判据只有一个——用户显式点清空，**不读会话表、不算差集**（INV5）；
/// ②整根全清，`removed_bytes` 由 owner 在删除前统计（INV8）；
/// ③基根保留、部分失败只计数不上抛（INV6 / INV8），`prepare` 负责下一次自愈。
pub(super) async fn execute_scratch_clear(
    store: Option<Arc<dyn ToolScratchStore>>,
    key: String,
    action: StorageReclaimActionDto,
) -> StorageReclaimEntryResultDto {
    let Some(store) = store else {
        return entry_result(
            key,
            action,
            Totals::default(),
            vec![error_dto(
                StorageReclaimErrorCodeDto::OwnerUnavailable,
                "临时区不可用",
            )],
            StorageReclaimStatusDto::Failed,
        );
    };
    let report = match tauri::async_runtime::spawn_blocking(move || store.clear()).await {
        Ok(report) => report,
        Err(_) => {
            return entry_result(
                key,
                action,
                Totals::default(),
                vec![error_dto(
                    StorageReclaimErrorCodeDto::Unknown,
                    "清理任务出错".to_string(),
                )],
                StorageReclaimStatusDto::Failed,
            );
        }
    };
    let totals = Totals {
        removed: report.removed,
        removed_bytes: report.removed_bytes,
        failed: report.failed,
        skipped: report.skipped,
    };
    let errors = failure_errors(totals.failed, 0, "部分临时文件删除失败", "临时区读取失败");
    entry_result(key, action, totals, errors, status_of(totals))
}
