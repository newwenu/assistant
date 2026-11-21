pub mod entries;
pub mod inventory;
mod ledger;
pub mod reclaim;
mod sample;
pub mod section;

use std::sync::Arc;

use tauri::{command, AppHandle, Manager};

use crate::adapters::PipelineToolExecution;
use crate::protocol::dto::{
    AttachmentDeleteResultDto, StorageBreakdownDto, StorageInventorySnapshotStatusDto,
    StorageReclaimPlanDto, StorageReclaimPlanRequestDto, StorageReclaimRequestDto,
    StorageReclaimResultDto,
};
use crate::setup::DbState;
use assistant_storage::attachment_store::{self, AttachmentStore};
use assistant_tools::ToolResultStorage;

use inventory::InventoryCache;
use ledger::{build_base, load_attachment_refs};
use reclaim::{ReclaimPlanStore, ReclaimPorts};
use section::{SectionRegistry, StorageSectionHandle};

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[command]
pub async fn get_storage_breakdown(app: AppHandle) -> Result<StorageBreakdownDto, String> {
    let sampled_at = now_seconds();

    // 基根解析失败是装配级错误（与 `E1` 同级）：整命令 `Err`，
    // 不在采样层把它伪装成「条目不可用」（§7.1 特例 1）。
    let dirs = crate::app_dirs::AppDirs::resolve().map_err(|e| e.to_string())?;

    let store = app.state::<AttachmentStore>().inner().clone();
    let all_entries = store.list_all().await.map_err(|e| e.to_string())?;

    let refs = {
        let db = app.state::<DbState>().inner().clone();
        tauri::async_runtime::spawn_blocking(move || load_attachment_refs(&db))
            .await
            .map_err(|e| e.to_string())??
    };

    // `E1` 的孤儿判据与顶层孤儿列表同一份引用集合：两边不可能对不上（§3.10 第 1 条）。
    let sampled = sample::collect(&app, &dirs, &store, &all_entries, Some(&refs.referenced)).await;
    let base = build_base(&all_entries, refs);

    let cache = app.state::<InventoryCache>().inner().clone();
    let (inventory, trigger) = cache.read(now_seconds(), &sampled.views, &sampled.samples);
    if trigger {
        inventory::trigger_scan(&app);
    }

    Ok(entries::assemble_breakdown(
        base,
        &sampled.views,
        &sampled.samples,
        sampled_at,
        inventory,
    ))
}

/// 只触发后台扫描并回状态：本命令的返回类型在类型层面就无法回数字（§6.2）。
#[command]
pub fn refresh_storage_inventory(app: AppHandle) -> StorageInventorySnapshotStatusDto {
    inventory::trigger_scan(&app);
    app.state::<InventoryCache>().inner().status(now_seconds())
}

/// 回收命令的依赖打包：两个命令共用一处取句柄逻辑，`ReclaimPorts` 只借用本结构。
struct ReclaimContext {
    plans: Arc<ReclaimPlanStore>,
    dirs: crate::app_dirs::AppDirs,
    store: AttachmentStore,
    sections: Vec<StorageSectionHandle>,
    tool_results: Option<Arc<dyn ToolResultStorage>>,
    db: Option<DbState>,
}

impl ReclaimContext {
    fn load(app: &AppHandle) -> Result<Self, String> {
        // 与 `get_storage_breakdown` 同口径：基根解析失败是装配级错误，整命令 `Err`。
        let dirs = crate::app_dirs::AppDirs::resolve().map_err(|e| e.to_string())?;
        let sections = app
            .try_state::<Arc<SectionRegistry>>()
            .map(|state| state.inner().snapshot())
            .unwrap_or_default();
        let tool_results: Option<Arc<dyn ToolResultStorage>> = app
            .try_state::<Arc<PipelineToolExecution>>()
            .map(|state| state.storage().clone());
        let db = app
            .try_state::<DbState>()
            .map(|state| state.inner().clone());
        Ok(Self {
            plans: app.state::<Arc<ReclaimPlanStore>>().inner().clone(),
            dirs,
            store: app.state::<AttachmentStore>().inner().clone(),
            sections,
            tool_results,
            db,
        })
    }

    fn ports(&self, now: i64) -> ReclaimPorts<'_> {
        ReclaimPorts {
            plans: &self.plans,
            dirs: &self.dirs,
            attachments: &self.store,
            sections: self.sections.clone(),
            tool_results: self.tool_results.clone(),
            db: self.db.clone(),
            now,
        }
    }
}

/// 计划：只读预览，10 分钟内可执行（`planId` 不透明，前端只回传不解析）。
#[command]
pub async fn get_storage_reclaim_plan(
    app: AppHandle,
    request: StorageReclaimPlanRequestDto,
) -> Result<StorageReclaimPlanDto, String> {
    let context = ReclaimContext::load(&app)?;
    let now = now_seconds();
    reclaim::plan_storage_reclaim(context.ports(now), request).await
}

/// 执行：必须命中未过期计划且落在白名单，任一不符整体拒绝（零副作用）。
#[command]
pub async fn reclaim_storage(
    app: AppHandle,
    request: StorageReclaimRequestDto,
) -> Result<StorageReclaimResultDto, String> {
    let context = ReclaimContext::load(&app)?;
    let now = now_seconds();
    reclaim::execute_storage_reclaim(context.ports(now), request).await
}

#[command]
pub async fn delete_attachments(
    app: AppHandle,
    attachment_ids: Vec<String>,
) -> Result<AttachmentDeleteResultDto, String> {
    for id in &attachment_ids {
        if !attachment_store::is_plain_file_name(id) {
            return Err(format!("invalid attachment id: {}", id));
        }
    }

    let attachment_store = app.state::<AttachmentStore>().inner().clone();

    let (removed_count, removed_bytes) = {
        let state = app.state::<crate::setup::DbState>();
        let conn = super::get_db_conn(&state)?;
        let mut removed_count: u64 = 0;
        let mut removed_bytes: u64 = 0;

        for id in &attachment_ids {
            // 与引用写入共用同一把 ID 锁（§5.5）：锁内先删引用、再删文件，
            // 顺序固定，避免「引用已删、文件还在」被 `A1` 抢先回收后引用又落库。
            let _guard = attachment_store.id_guard(id);
            attachment_store::delete_attachment_refs_by_attachment_id(&conn, id)?;
            match attachment_store.delete_blocking(id) {
                Ok(Some(bytes)) => {
                    removed_count += 1;
                    removed_bytes += bytes;
                }
                Ok(None) => {}
                Err(e) => log::warn!("Failed to delete attachment file {}: {}", id, e),
            }
        }

        (removed_count, removed_bytes)
    };

    let removed_count = u32::try_from(removed_count)
        .map_err(|_| "removed attachment count exceeds u32".to_string())?;

    Ok(AttachmentDeleteResultDto {
        removed_count,
        removed_bytes,
    })
}

#[command]
pub async fn open_attachment(app: AppHandle, attachment_id: String) -> Result<bool, String> {
    let path = match app.state::<AttachmentStore>().file_path(&attachment_id) {
        Ok(path) => path,
        // 非法 id 与文件不存在同为「不可打开」：保持原有 `false` 语义，
        // 由前端提示「附件已不存在」，不升格为命令错误。
        Err(attachment_store::AttachmentError::InvalidId(_)) => return Ok(false),
        Err(e) => return Err(e.to_string()),
    };

    tauri::async_runtime::spawn_blocking(move || {
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.to_string()),
        };
        if !metadata.is_file() {
            return Ok(false);
        }
        tauri_plugin_opener::open_path(path.as_path(), None::<&str>).map_err(|e| e.to_string())?;
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}
