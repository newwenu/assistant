// 参考: docs/store/I75-startup-layering-spec.md §9 #6 — 只留目录与实例，`clean_tmp` 已移出

use std::sync::Arc;

use tauri::Manager;

use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};
use assistant_storage::attachment_store::AttachmentStore;

pub fn build(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let dirs = ctx.dirs()?;
    let attachments_dir = dirs.attachments();
    std::fs::create_dir_all(&attachments_dir)
        .map_err(|e| CapErr::build("attachment_store", e.to_string()))?;
    let store = AttachmentStore::new(attachments_dir.clone());
    ctx.app.manage(store.clone());
    // 目录属于阶段行没有的上下文（「附件写到哪里」），dev 档可见、release 档静默。
    log::debug!(
        "AttachmentStore initialized at {}",
        attachments_dir.display()
    );
    Ok(Arc::new(store))
}
