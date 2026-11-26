// 参考: docs/store/I75-startup-layering-spec.md §9 #14 — Idle 泳道的清理

use std::sync::Arc;

use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};
use crate::db_state::DbState;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_storage::db;
use assistant_tools::cleanup_expired_results;

/// 审计裁剪 + 工具结果清理 + 附件临时区清理：纯本地 I/O，放在首帧之后执行，失败只降档不阻断。
pub fn build(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let conf = ctx.boot()?;
    let state = ctx.get::<DbState>("storage")?;

    // 审计表只追加、无上限，必须按保留期裁剪一次；配置读取失败按 `0`（不清理），
    // 宁可保留也不要误删历史决策。
    let retention_days = conf.app.audit_retention_days;
    let conn = state
        .0
        .lock()
        .map_err(|e| CapErr::build("storage_maintenance", format!("db lock: {e}")))?;
    match db::cleanup_expired_audit(&conn, retention_days, chrono::Utc::now().timestamp_millis()) {
        // 零删除是常态：`info` 只留给真有动作的那次，常态在 dev 档留一行「已跑过」，
        // 否则排查「清理到底有没有执行」时无从判断。
        Ok(0) => {
            log::debug!("permission audit: nothing to prune (retention {retention_days} days)")
        }
        Ok(pruned) => log::info!("Pruned {} expired permission audit rows", pruned),
        Err(e) => log::warn!("Failed to prune permission audit rows: {}", e),
    }
    drop(conn);

    // 工具结果清理（I46 §2.9 D9 / I75 §9 #14）：保留策略取装配期同一份快照（I61 §3.3），
    // 未注入策略（`None`）即不清理；放独立线程，纯本地 I/O 不阻塞首帧。
    if let Some((days, _)) = ctx
        .get::<crate::adapters::PipelineToolExecution>("tool_exec")
        .ok()
        .and_then(|exec| exec.storage().result_retention())
    {
        let root = ctx.dirs()?.tool_results();
        std::thread::spawn(move || {
            let report = cleanup_expired_results(&root, days);
            // 零删除是常态：只在真有动作时占 info 档；常态在 dev 档留一行，作为「跑过了」的凭据。
            if report.removed > 0 || report.failed > 0 {
                log::info!(
                    "tool result cleanup: removed {} expired file(s), {} failure(s) (retention {} days)",
                    report.removed, report.failed, days
                );
            } else {
                log::debug!("tool result cleanup: nothing to remove (retention {days} days)");
            }
        });
    }

    let store = ctx.get::<AttachmentStore>("attachment_store")?;
    tauri::async_runtime::spawn(async move {
        if let Err(e) = store.clean_tmp().await {
            log::warn!("Failed to clean tmp attachments: {}", e);
        }
    });

    Ok(Arc::new(()))
}
