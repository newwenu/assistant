use assistant_tools::{ApprovalKey, ApprovalScope, GrantScope, ToolTier};
use rusqlite::{params, OptionalExtension};
use tauri::{command, AppHandle, Manager};

use crate::adapters::{decode_grant_key, encode_grant_key};
use crate::protocol::{GrantScopeDto, PermissionGrantDto, SetGrantScopeArgsDto};

fn scope_to_dto(scope: GrantScope) -> GrantScopeDto {
    match scope {
        GrantScope::Session => GrantScopeDto::Session,
        GrantScope::Global => GrantScopeDto::Global,
    }
}

fn scope_from_dto(scope: GrantScopeDto) -> GrantScope {
    match scope {
        GrantScopeDto::Session => GrantScope::Session,
        GrantScopeDto::Global => GrantScope::Global,
    }
}

fn tier_label(tier: ToolTier) -> &'static str {
    match tier {
        ToolTier::Sandbox => "sandbox",
        ToolTier::Os => "os",
        ToolTier::External => "external",
    }
}

/// 把 `ApprovalScope` 归一成人可读摘要。
///
/// 前端只渲染这一行、不解析 `grantKey`：作用域形状随工具增长，
/// 解析逻辑集中在 Rust 侧一处，前端不认识新作用域也不会显示原始 JSON。
fn scope_summary(scope: &ApprovalScope) -> String {
    match scope {
        ApprovalScope::Path { canonical } => canonical.clone(),
        ApprovalScope::Command {
            argv,
            cwd,
            sandbox_tier,
        } => format!(
            "[{}] {} · {}",
            tier_label(*sandbox_tier),
            cwd,
            argv.join(" ")
        ),
        ApprovalScope::Endpoint { host, method } => format!("{method} {host}"),
        ApprovalScope::Opaque { fingerprint } => fingerprint.clone(),
    }
}

/// 解析失败（历史遗留 / 手工写库）时退回原始键 JSON：宁可显示难读的键，也不隐藏授权行。
fn grant_scope_summary(key_json: &str) -> String {
    match serde_json::from_str::<ApprovalKey>(key_json) {
        Ok(key) => scope_summary(&key.scope),
        Err(err) => {
            log::warn!("permission grant key decode failed: {err}");
            key_json.to_string()
        }
    }
}

#[command]
pub fn list_permission_grants(app: AppHandle) -> Result<Vec<PermissionGrantDto>, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let mut stmt = conn
        .prepare(
            "SELECT grant_key, tool, created_at FROM permission_grants
             ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let grant_key: String = row.get(0)?;
            let tool: String = row.get(1)?;
            let created_at: i64 = row.get(2)?;
            Ok((grant_key, tool, created_at))
        })
        .map_err(|e| e.to_string())?;
    let mut grants = Vec::new();
    for row in rows {
        let (grant_key, tool, created_at) = row.map_err(|e| e.to_string())?;
        let (scope, conversation_id, key_json) = decode_grant_key(&grant_key);
        let scope = scope_to_dto(scope);
        let conversation_id = conversation_id.map(str::to_string);
        let scope_summary = grant_scope_summary(key_json);
        grants.push(PermissionGrantDto {
            grant_key,
            tool,
            scope,
            conversation_id,
            scope_summary,
            created_at,
        });
    }
    Ok(grants)
}

/// 单条撤销。返回 `false` 表示该行已不存在（并发撤销 / 已被全部清除）。
#[command]
pub fn revoke_permission_grant(app: AppHandle, grant_key: String) -> Result<bool, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    let affected = conn
        .execute(
            "DELETE FROM permission_grants WHERE grant_key = ?1",
            params![grant_key],
        )
        .map_err(|e| e.to_string())?;
    Ok(affected > 0)
}

/// 修改已有授权的作用域（单事务 delete + insert）。
///
/// 返回 `false` 表示源行已不存在（并发撤销 / 已清除），前端应重新拉列表对齐。
/// `created_at` 随行迁移，设置页按时间排序时才不会因改作用域而跳动。
#[command]
pub fn set_permission_grant_scope(
    app: AppHandle,
    args: SetGrantScopeArgsDto,
) -> Result<bool, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;

    let (source_scope, _, key_json) = decode_grant_key(&args.grant_key);
    let key_json = key_json.to_string();
    let target_scope = scope_from_dto(args.scope);
    let target_key = encode_grant_key(&key_json, target_scope, args.conversation_id.as_deref())?;

    let source = conn
        .query_row(
            "SELECT tool, created_at FROM permission_grants WHERE grant_key = ?1",
            params![args.grant_key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((tool, created_at)) = source else {
        return Ok(false);
    };

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    // 升级到全局时删除该键所有会话级行：全局已覆盖它们，保留只会产生同键多行。
    if target_scope == GrantScope::Global && source_scope == GrantScope::Session {
        let session_keys: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT grant_key FROM permission_grants")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let mut keys = Vec::new();
            for row in rows {
                let key = row.map_err(|e| e.to_string())?;
                let (scope, _, json) = decode_grant_key(&key);
                if scope == GrantScope::Session && json == key_json {
                    keys.push(key);
                }
            }
            keys
        };
        for key in session_keys {
            tx.execute(
                "DELETE FROM permission_grants WHERE grant_key = ?1",
                params![key],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    tx.execute(
        "DELETE FROM permission_grants WHERE grant_key = ?1",
        params![args.grant_key],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR IGNORE INTO permission_grants (grant_key, tool, created_at)
         VALUES (?1, ?2, ?3)",
        params![target_key, tool, created_at],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(true)
}

#[command]
pub fn clear_permission_grants(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::get_db_conn(&state)?;
    conn.execute("DELETE FROM permission_grants", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
