use assistant_tools::{ApprovalKey, ApprovalScope, GrantScope};
use rusqlite::{params, Connection, OptionalExtension};
use tauri::{command, AppHandle, Manager};

use crate::adapters::{decode_grant_key, encode_grant_key};
use crate::protocol::{GrantScopeDto, PermissionGrantDto, SetGrantScopeArgsDto};

use super::audit::{
    audit_resource_from_key, write_grant_audit, GRANT_ACTION_CLEAR, GRANT_ACTION_REVOKE,
    GRANT_ACTION_SCOPE, GRANT_AUDIT_NO_RESOURCE,
};
use super::{short_hash, tier_label};

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
        // MCP 授权精确到一次调用：摘要里只给哈希前缀，完整指纹对用户无意义。
        ApprovalScope::Mcp {
            server_id,
            tool,
            args_fingerprint,
            config_hash,
        } => format!(
            "mcp:{server_id} · {tool} · args {} · cfg {}",
            short_hash(args_fingerprint),
            short_hash(config_hash)
        ),
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
    let conn = super::super::get_db_conn(&state)?;
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
    let conn = super::super::get_db_conn(&state)?;
    revoke_grant_row(&conn, &grant_key)
}

/// `revoke_permission_grant` 的纯库层实现（事务 + 审计都在这里，便于单测）。
fn revoke_grant_row(conn: &Connection, grant_key: &str) -> Result<bool, String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let affected = tx
        .execute(
            "DELETE FROM permission_grants WHERE grant_key = ?1",
            params![grant_key],
        )
        .map_err(|e| e.to_string())?;
    // 目标已不存在（并发撤销 / 已清除）时没有真实变更，不写审计行。
    if affected > 0 {
        write_grant_audit(
            &tx,
            GRANT_ACTION_REVOKE,
            &audit_resource_from_key(grant_key),
        );
    }
    tx.commit().map_err(|e| e.to_string())?;
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
    let conn = super::super::get_db_conn(&state)?;
    move_grant_scope(&conn, &args)
}

/// `set_permission_grant_scope` 的纯库层实现（事务 + 审计都在这里，便于单测）。
fn move_grant_scope(conn: &Connection, args: &SetGrantScopeArgsDto) -> Result<bool, String> {
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
    // `resource` 记**变更前**的源键（脱敏后）：追溯面板要能对上「哪一条授权被改了作用域」。
    write_grant_audit(
        &tx,
        GRANT_ACTION_SCOPE,
        &audit_resource_from_key(&args.grant_key),
    );
    tx.commit().map_err(|e| e.to_string())?;

    Ok(true)
}

#[command]
pub fn clear_permission_grants(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::super::get_db_conn(&state)?;
    clear_grant_rows(&conn)
}

/// `clear_permission_grants` 的纯库层实现（事务 + 审计都在这里，便于单测）。
fn clear_grant_rows(conn: &Connection) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let affected = tx
        .execute("DELETE FROM permission_grants", [])
        .map_err(|e| e.to_string())?;
    // 表本来就空时不写审计：没有变更就没有可追溯的管理动作。
    if affected > 0 {
        write_grant_audit(&tx, GRANT_ACTION_CLEAR, GRANT_AUDIT_NO_RESOURCE);
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod grant_tests {
    use super::*;

    fn temp_conn() -> (Connection, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = assistant_storage::db::init_db(&dir.path().join("audit.db")).unwrap();
        (conn, dir)
    }

    /// 必须与 `ApprovalKey` 的线上形态一致（`scope.kind` 是 serde tag），
    /// 否则审计解析不出作用域，只能落 `unparsed`。
    const KEY_JSON: &str =
        r#"{"tool":"agent_write","scope":{"kind":"path","canonical":"/ws/a.txt"}}"#;

    fn global_key() -> String {
        format!("g|{KEY_JSON}")
    }

    fn insert_grant(conn: &Connection, grant_key: &str) {
        conn.execute(
            "INSERT INTO permission_grants (grant_key, tool, created_at)
             VALUES (?1, 'agent_write', 1)",
            params![grant_key],
        )
        .unwrap();
    }

    fn grant_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM permission_grants", [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    /// 审计行形状：`(tool, action, resource, outcome)`。
    fn audit_rows(conn: &Connection) -> Vec<(String, String, String, String)> {
        let mut stmt = conn
            .prepare("SELECT tool, action, resource, outcome FROM permission_audit ORDER BY id")
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap();
        rows.map(|row| row.unwrap()).collect()
    }

    /// 规格 §3.6：撤销单条产生 1 行 `grant` / `grant:revoke` / `allowed`，
    /// `resource` 为脱敏后的作用域摘要（绝对路径只留末段，不落目录结构）。
    #[test]
    fn revoke_writes_one_grant_audit_row() {
        let (conn, _dir) = temp_conn();
        let key = global_key();
        insert_grant(&conn, &key);

        assert!(revoke_grant_row(&conn, &key).unwrap());
        assert_eq!(grant_count(&conn), 0);

        let rows = audit_rows(&conn);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "grant");
        assert_eq!(rows[0].1, "grant:revoke");
        assert_eq!(rows[0].2, "…/a.txt");
        assert_eq!(rows[0].3, "allowed");
    }

    /// 目标已不存在时不写审计（没有变更就没有可追溯的管理动作）。
    #[test]
    fn revoke_missing_grant_writes_no_audit_row() {
        let (conn, _dir) = temp_conn();
        assert!(!revoke_grant_row(&conn, &global_key()).unwrap());
        assert!(audit_rows(&conn).is_empty());
    }

    /// 清除全部：`resource` 固定 `none`；表本来就空时不留审计。
    #[test]
    fn clear_writes_audit_only_when_rows_were_removed() {
        let (conn, _dir) = temp_conn();
        insert_grant(&conn, &global_key());
        insert_grant(&conn, &format!("s:conv-1|{KEY_JSON}"));

        clear_grant_rows(&conn).unwrap();
        assert_eq!(grant_count(&conn), 0);
        let rows = audit_rows(&conn);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "grant:clear");
        assert_eq!(rows[0].2, "none");
        assert_eq!(rows[0].3, "allowed");

        clear_grant_rows(&conn).unwrap();
        assert_eq!(audit_rows(&conn).len(), 1, "空表再清一次不得再写审计");
    }

    /// 改作用域：`resource` 必须是**变更前**的源键摘要，行也真的迁到目标键。
    #[test]
    fn scope_change_audits_the_source_key() {
        let (conn, _dir) = temp_conn();
        let source = global_key();
        insert_grant(&conn, &source);

        let args = SetGrantScopeArgsDto {
            grant_key: source.clone(),
            scope: GrantScopeDto::Session,
            conversation_id: Some("conv-1".to_string()),
        };
        assert!(move_grant_scope(&conn, &args).unwrap());

        let rows = audit_rows(&conn);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "grant:scope");
        assert_eq!(
            rows[0].2, "…/a.txt",
            "resource 必须记源键摘要，不能记新算出的目标键"
        );
        assert_eq!(rows[0].3, "allowed");

        let stored: String = conn
            .query_row("SELECT grant_key FROM permission_grants", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored, format!("s:conv-1|{KEY_JSON}"));
    }

    /// 源行已不存在时不写审计，命令返回 `false`。
    #[test]
    fn scope_change_on_missing_grant_writes_no_audit_row() {
        let (conn, _dir) = temp_conn();
        let args = SetGrantScopeArgsDto {
            grant_key: global_key(),
            scope: GrantScopeDto::Session,
            conversation_id: Some("conv-1".to_string()),
        };
        assert!(!move_grant_scope(&conn, &args).unwrap());
        assert!(audit_rows(&conn).is_empty());
    }
}
