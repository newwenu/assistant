// 参考: docs/store/I75-startup-layering-spec.md §9 #5 — 只留 DB 打开 / 迁移 / 种子

use std::sync::{Arc, Mutex};

use tauri::Manager;

use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};
use crate::db_state::DbState;
use assistant_storage::db;

/// 启动不做任何全表收尾：占位行机制（D27）让拓扑在声明时定死，残留的在途行
/// 由投影期按 `status` 补文案，工具结果读取也会过滤在途行。
/// 审计表裁剪已移出到 `storage_maintenance`（Idle 泳道）。
pub fn build(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let dirs = ctx.dirs()?;
    let db_path = dirs.database();
    let conn = db::init_db_localized(&db_path, &|key| assistant_i18n::t(key))
        .map_err(|e| CapErr::build("storage", e.to_string()))?;
    let state = DbState(Arc::new(Mutex::new(conn)), db_path);
    ctx.app.manage(state.clone());
    Ok(Arc::new(state))
}
