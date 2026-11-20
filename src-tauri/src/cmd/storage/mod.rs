pub mod entries;
pub mod inventory;
pub(crate) mod probe;
pub mod reclaim;
mod sample;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tauri::{command, AppHandle, Manager};

use crate::adapters::PipelineToolExecution;
use crate::protocol::dto::{
    AttachmentDeleteResultDto, AttachmentSummary, ConversationAttachmentSummary,
    StorageBreakdownDto, StorageInventorySnapshotStatusDto, StorageReclaimPlanDto,
    StorageReclaimPlanRequestDto, StorageReclaimRequestDto, StorageReclaimResultDto,
};
use crate::setup::DbState;
use assistant_storage::attachment_store::{self, AttachmentEntry, AttachmentStore};
use assistant_tools::ToolResultStorage;

use entries::BreakdownBase;
use inventory::InventoryCache;
use probe::ProbeRegistry;
use reclaim::{ReclaimPlanStore, ReclaimPorts};

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// 引用侧数据：`referenced` 判孤儿，`rows` 供会话分组与「共享 / 仅引用一次」计数。
struct AttachmentRefs {
    referenced: HashSet<String>,
    rows: Vec<ConvAttachmentRow>,
}

struct ConvAttachmentRow {
    conversation_id: String,
    attachment_id: String,
    title: String,
    updated_at: i64,
}

/// 一次连接、一条 JOIN 拿全：标题与 `updated_at` 随行带回，避免逐会话回查（N+1）。
fn load_attachment_refs(db: &DbState) -> Result<AttachmentRefs, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    let referenced = attachment_store::get_all_referenced_attachment_ids(&conn)?
        .into_iter()
        .collect();

    let mut stmt = conn
        .prepare(
            "SELECT ar.conversation_id, ar.attachment_id, \
                    COALESCE(c.title, ar.conversation_id), COALESCE(c.updated_at, 0) \
             FROM attachment_refs ar \
             LEFT JOIN conversations c ON c.id = ar.conversation_id \
             ORDER BY ar.conversation_id",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ConvAttachmentRow {
                conversation_id: row.get(0)?,
                attachment_id: row.get(1)?,
                title: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(AttachmentRefs { referenced, rows })
}

fn summary(entry: &AttachmentEntry) -> AttachmentSummary {
    AttachmentSummary {
        attachment_id: entry.id.clone(),
        media_type: media_type_from_id(&entry.id),
        bytes: entry.bytes,
        created_at: i64::try_from(entry.created_at).unwrap_or(i64::MAX),
    }
}

/// 由同一份 `list_all` 派生顶层字段与 `E1`（§3.10 第 1 条）：两边不可能对不上。
fn build_base(entries: &[AttachmentEntry], refs: AttachmentRefs) -> BreakdownBase {
    let entry_map: HashMap<&str, &AttachmentEntry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();

    let mut orphans: Vec<AttachmentSummary> = entries
        .iter()
        .filter(|entry| !refs.referenced.contains(&entry.id))
        .map(summary)
        .collect();
    orphans.sort_by_key(|orphan| std::cmp::Reverse(orphan.created_at));

    let mut conv_count: HashMap<&str, u64> = HashMap::new();
    let mut conv_attachments: HashMap<&str, (String, i64, Vec<AttachmentSummary>)> = HashMap::new();

    for row in &refs.rows {
        let Some(entry) = entry_map.get(row.attachment_id.as_str()) else {
            continue;
        };
        *conv_count.entry(entry.id.as_str()).or_default() += 1;
        let bucket = conv_attachments
            .entry(row.conversation_id.as_str())
            .or_insert_with(|| (row.title.clone(), row.updated_at, Vec::new()));
        bucket.2.push(summary(entry));
    }

    let mut conversations: Vec<ConversationAttachmentSummary> = conv_attachments
        .into_iter()
        .map(|(conversation_id, (title, updated_at, attachments))| {
            let total_bytes = attachments.iter().map(|attachment| attachment.bytes).sum();
            ConversationAttachmentSummary {
                conversation_id: conversation_id.to_string(),
                title,
                attachments,
                total_bytes,
                updated_at,
            }
        })
        .collect();
    conversations.sort_by_key(|conversation| std::cmp::Reverse(conversation.total_bytes));

    let shared_bytes: u64 = entries
        .iter()
        .filter(|entry| conv_count.get(entry.id.as_str()).copied().unwrap_or(0) >= 2)
        .map(|entry| entry.bytes)
        .sum();
    let referenced_once_bytes: u64 = entries
        .iter()
        .filter(|entry| conv_count.get(entry.id.as_str()).copied() == Some(1))
        .map(|entry| entry.bytes)
        .sum();

    BreakdownBase {
        total_bytes: entries.iter().map(|entry| entry.bytes).sum(),
        total_count: u32::try_from(entries.len()).unwrap_or(u32::MAX),
        orphans,
        conversations,
        shared_bytes,
        referenced_once_bytes,
    }
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
    let samples = sample::collect(&app, &dirs, &store, &all_entries, Some(&refs.referenced)).await;
    let base = build_base(&all_entries, refs);

    let cache = app.state::<InventoryCache>().inner().clone();
    let (inventory, trigger) = cache.read(now_seconds(), &samples);
    if trigger {
        inventory::trigger_scan(&app);
    }

    Ok(entries::assemble_breakdown(
        base, &samples, sampled_at, inventory,
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
    probes: Option<Arc<ProbeRegistry>>,
    tool_results: Option<Arc<dyn ToolResultStorage>>,
    db: Option<DbState>,
}

impl ReclaimContext {
    fn load(app: &AppHandle) -> Result<Self, String> {
        // 与 `get_storage_breakdown` 同口径：基根解析失败是装配级错误，整命令 `Err`。
        let dirs = crate::app_dirs::AppDirs::resolve().map_err(|e| e.to_string())?;
        let probes = app
            .try_state::<Arc<ProbeRegistry>>()
            .map(|state| state.inner().clone());
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
            probes,
            tool_results,
            db,
        })
    }

    fn ports(&self, now: i64) -> ReclaimPorts<'_> {
        ReclaimPorts {
            plans: &self.plans,
            dirs: &self.dirs,
            attachments: &self.store,
            probes: self.probes.clone(),
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

fn media_type_from_id(id: &str) -> String {
    if let Some(ext) = id.rsplit('.').next() {
        match ext {
            "png" => "image/png".to_string(),
            "jpg" | "jpeg" => "image/jpeg".to_string(),
            "gif" => "image/gif".to_string(),
            "webp" => "image/webp".to_string(),
            "pdf" => "application/pdf".to_string(),
            _ => format!("application/{}", ext),
        }
    } else {
        "application/octet-stream".to_string()
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

    fn row(conversation_id: &str, attachment_id: &str) -> ConvAttachmentRow {
        ConvAttachmentRow {
            conversation_id: conversation_id.to_string(),
            attachment_id: attachment_id.to_string(),
            title: conversation_id.to_string(),
            updated_at: 0,
        }
    }

    #[test]
    fn ledger_identity_splits_shared_once_and_orphan() {
        let entries = vec![
            entry("once.png", 100, 1),
            entry("shared.png", 200, 2),
            entry("orphan.pdf", 300, 3),
        ];
        let refs = AttachmentRefs {
            referenced: ["once.png", "shared.png"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            rows: vec![
                row("c1", "once.png"),
                row("c1", "shared.png"),
                row("c2", "shared.png"),
            ],
        };
        let base = build_base(&entries, refs);

        assert_eq!(base.total_bytes, 600, "合计 = 全部文件长度之和");
        assert_eq!(base.total_count, 3);
        assert_eq!(base.referenced_once_bytes, 100, "仅被一个会话引用");
        assert_eq!(base.shared_bytes, 200, "被多个会话共享");
        assert_eq!(base.orphans.len(), 1);
        let orphan_bytes: u64 = base.orphans.iter().map(|orphan| orphan.bytes).sum();
        assert_eq!(
            base.total_bytes,
            base.referenced_once_bytes + base.shared_bytes + orphan_bytes,
            "账本恒等式（V5）：每一字节只归属一处"
        );
    }

    #[test]
    fn conversations_group_by_id_with_title_from_join() {
        let entries = vec![entry("a.png", 10, 1), entry("b.png", 20, 2)];
        let refs = AttachmentRefs {
            referenced: ["a.png", "b.png"].into_iter().map(str::to_string).collect(),
            rows: vec![row("c1", "a.png"), row("c1", "b.png")],
        };
        let base = build_base(&entries, refs);

        assert_eq!(base.conversations.len(), 1);
        assert_eq!(base.conversations[0].conversation_id, "c1");
        assert_eq!(base.conversations[0].attachments.len(), 2);
        assert_eq!(base.conversations[0].total_bytes, 30);
    }
}
