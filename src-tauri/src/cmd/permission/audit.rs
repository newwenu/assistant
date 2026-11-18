use assistant_tools::{redact_resource, ApprovalKey, ApprovalScope, AuditEntry};
use rusqlite::{params, params_from_iter, Connection};
use tauri::{command, AppHandle, Manager};

use crate::adapters::outcome_str;
use crate::protocol::{PermissionAuditEntryDto, PermissionAuditPageDto};

use super::{short_hash, tier_label};

/// 追溯面板单次拉取上限（规格 §3.7 / OF5：无「加载全部」入口）。
const AUDIT_LIMIT_DEFAULT: i64 = 100;
const AUDIT_LIMIT_MAX: i64 = 500;

/// 授权管理动作的审计词表（规格 §3.6）：`tool = "grant"`、`outcome` 一律 `allowed`。
const GRANT_AUDIT_TOOL: &str = "grant";
pub(super) const GRANT_ACTION_REVOKE: &str = "grant:revoke";
pub(super) const GRANT_ACTION_CLEAR: &str = "grant:clear";
pub(super) const GRANT_ACTION_SCOPE: &str = "grant:scope";
/// 批量动作（清除全部）没有单条目标，`resource` 落固定串而不是空串。
pub(super) const GRANT_AUDIT_NO_RESOURCE: &str = "none";

/// 管理动作审计用的作用域摘要：与列表展示的 `scope_summary` 同形，但路径一律脱敏。
///
/// 列表展示需要完整 `canonical`（用户要知道自己授权了哪个路径），审计落库不需要：
/// 主库是明文 SQLite，目录结构不该被持久化。两者因此分成两个函数，不复用同一份输出。
fn audit_scope_summary(scope: &ApprovalScope) -> String {
    match scope {
        ApprovalScope::Path { canonical } => redact_resource(canonical),
        // 审计词表不承载隔离强度（§8）：强度走工具结果与审批卡扩展，
        // 这里只落「哪条命令、在哪个目录」。
        ApprovalScope::Command {
            argv,
            cwd,
            sandbox_tier,
            ..
        } => format!(
            "[{}] {} · {}",
            tier_label(*sandbox_tier),
            redact_resource(cwd),
            argv.join(" ")
        ),
        ApprovalScope::Endpoint { host, method } => format!("{method} {host}"),
        ApprovalScope::Opaque { fingerprint } => fingerprint.clone(),
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

/// 把授权主键归一成审计 `resource`：解析不出作用域时落固定串，不回退原文。
///
/// 回退原文会把整段键 JSON（含 `canonical` 绝对路径）写进明文库，
/// 而审计只需要「被管理的是哪一类授权」，不需要原始键。
pub(super) fn audit_resource_from_key(grant_key: &str) -> String {
    let (_, _, key_json) = crate::adapters::decode_grant_key(grant_key);
    match serde_json::from_str::<ApprovalKey>(key_json) {
        Ok(key) => audit_scope_summary(&key.scope),
        Err(err) => {
            log::warn!("permission grant key decode failed for audit: {err}");
            "unparsed".to_string()
        }
    }
}

/// 写一条授权管理审计行，调用方必须已开启事务（审计与授权变更同进同退）。
///
/// 失败只 `warn` 不返回错误：审计不得阻断业务（§3.6 与 §4 的边界）。
/// 因此签名返回 `()`，调用点无法用 `?` 把写失败升级成前端可见错误。
pub(super) fn write_grant_audit(conn: &Connection, action: &str, resource: &str) {
    let entry = AuditEntry::allowed(GRANT_AUDIT_TOOL, action, resource);
    if let Err(err) = conn.execute(
        "INSERT INTO permission_audit
           (tool, action, resource, outcome, at_millis, conversation_id, turn_id, iteration)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL)",
        params![
            entry.tool,
            entry.action,
            entry.resource,
            outcome_str(entry.outcome),
            entry.at_millis
        ],
    ) {
        log::warn!("permission grant audit write failed ({action}): {err}");
    }
}

/// `limit` 夹取：非正值（含前端传 0 与负数）回落默认，超过上限截断。
///
/// 收 `i64` 而非 `u32`：负数若交给 `u32` 反序列化会在 IPC 层直接报错，
/// 前端传错值时宁可回落默认，也不要把列表打成异常弹窗。
pub fn clamp_audit_limit(limit: Option<i64>) -> i64 {
    match limit {
        Some(value) if value > 0 => value.min(AUDIT_LIMIT_MAX),
        _ => AUDIT_LIMIT_DEFAULT,
    }
}

/// 偏移量夹取：非正值回落 0（前端传错值时退化成第一页，而不是报错）。
pub fn clamp_audit_offset(offset: Option<i64>) -> i64 {
    match offset {
        Some(value) if value > 0 => value,
        _ => 0,
    }
}

/// 按 `at_millis` 倒序查询一页审计行。
///
/// `since` 为闭区间下界、`until` 为闭区间上界：翻页时前端把上界冻结在首页最新一行的
/// 时间戳上，新写入的决策不会把后续页挤偏。`offset` 是页内起点。
///
/// 排序加 `id DESC` 次级键，同毫秒多行的顺序才稳定可复现（R20 同源要求）。
///
/// `hasMore` 靠多取一行判定而不是让前端猜：`limit` 恰好等于窗口内总数时，
/// 「本页条数 == limit」与「还有更早记录」同形，前端无法区分。
pub fn query_permission_audit(
    conn: &Connection,
    limit: i64,
    since: Option<i64>,
    until: Option<i64>,
    offset: i64,
) -> Result<PermissionAuditPageDto, String> {
    let mut sql = String::from(
        "SELECT tool, action, resource, outcome, at_millis, conversation_id, turn_id, iteration \
         FROM permission_audit",
    );
    let mut bounds: Vec<&str> = Vec::new();
    if since.is_some() {
        bounds.push("at_millis >= ?");
    }
    if until.is_some() {
        bounds.push("at_millis <= ?");
    }
    if !bounds.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&bounds.join(" AND "));
    }
    sql.push_str(" ORDER BY at_millis DESC, id DESC LIMIT ? OFFSET ?");

    let mut args: Vec<i64> = Vec::new();
    if let Some(since_ms) = since {
        args.push(since_ms);
    }
    if let Some(until_ms) = until {
        args.push(until_ms);
    }
    args.push(limit.max(0).saturating_add(1));
    args.push(offset.max(0));

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(args.iter()), |row| {
            Ok(PermissionAuditEntryDto {
                tool: row.get(0)?,
                action: row.get(1)?,
                resource: row.get(2)?,
                outcome: row.get(3)?,
                at_millis: row.get(4)?,
                conversation_id: row.get(5)?,
                turn_id: row.get(6)?,
                iteration: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| e.to_string())?);
    }

    let keep = limit.max(0) as usize;
    let has_more = entries.len() > keep;
    entries.truncate(keep);
    Ok(PermissionAuditPageDto { entries, has_more })
}

/// 权限审计追溯查询（规格 §3.7 F2）。
#[command]
pub fn list_permission_audit(
    app: AppHandle,
    limit: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
    offset: Option<i64>,
) -> Result<PermissionAuditPageDto, String> {
    let state = app.state::<crate::setup::DbState>();
    let conn = super::super::get_db_conn(&state)?;
    query_permission_audit(
        &conn,
        clamp_audit_limit(limit),
        since,
        until,
        clamp_audit_offset(offset),
    )
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    fn temp_conn() -> (Connection, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let conn = assistant_storage::db::init_db(&dir.path().join("audit.db")).unwrap();
        (conn, dir)
    }

    fn insert(conn: &Connection, tool: &str, outcome: &str, at_millis: i64) {
        conn.execute(
            "INSERT INTO permission_audit (tool, action, resource, outcome, at_millis)
             VALUES (?1, 'exec', '/ws/a.txt', ?2, ?3)",
            params![tool, outcome, at_millis],
        )
        .unwrap();
    }

    /// 规格 §3.7 / OF5：非正值回落默认、超上限截断，`None` 取默认。
    #[test]
    fn limit_is_clamped_to_default_and_max() {
        assert_eq!(clamp_audit_limit(None), AUDIT_LIMIT_DEFAULT);
        assert_eq!(clamp_audit_limit(Some(0)), AUDIT_LIMIT_DEFAULT);
        assert_eq!(clamp_audit_limit(Some(-5)), AUDIT_LIMIT_DEFAULT);
        assert_eq!(clamp_audit_limit(Some(10)), 10);
        assert_eq!(
            clamp_audit_limit(Some(AUDIT_LIMIT_MAX + 1)),
            AUDIT_LIMIT_MAX
        );
    }

    /// 规格 §3.7：`since` 为闭区间下界，结果恒按 `at_millis` 倒序。
    #[test]
    fn since_filters_and_rows_are_desc() {
        let (conn, _dir) = temp_conn();
        insert(&conn, "agent_write", "allowed", 100);
        insert(&conn, "agent_write", "denied", 200);
        insert(&conn, "agent_write", "approved", 300);

        let all = query_permission_audit(&conn, 100, None, None, 0).unwrap();
        let times: Vec<i64> = all.entries.iter().map(|entry| entry.at_millis).collect();
        assert_eq!(times, vec![300, 200, 100], "必须按时间倒序");

        let since_200 = query_permission_audit(&conn, 100, Some(200), None, 0).unwrap();
        assert_eq!(since_200.entries.len(), 2, "since 必须含下界本身（闭区间）");
        assert_eq!(since_200.entries[1].outcome, "denied");
    }

    /// `until` 为闭区间上界：翻页冻结窗口后，窗口外的新行不得混入。
    #[test]
    fn until_freezes_the_window() {
        let (conn, _dir) = temp_conn();
        insert(&conn, "agent_write", "allowed", 100);
        insert(&conn, "agent_write", "denied", 200);
        insert(&conn, "agent_write", "approved", 300);

        let frozen = query_permission_audit(&conn, 100, None, Some(200), 0).unwrap();
        let times: Vec<i64> = frozen.entries.iter().map(|entry| entry.at_millis).collect();
        assert_eq!(times, vec![200, 100], "until 必须含上界本身且排除更晚的行");
    }

    /// 翻页按 `offset` 切片，页间不重叠、不遗漏，末页 `hasMore` 为 false。
    #[test]
    fn offset_pages_without_overlap() {
        let (conn, _dir) = temp_conn();
        for at in 0..5 {
            insert(&conn, "agent_write", "allowed", at);
        }

        let first = query_permission_audit(&conn, 2, None, None, 0).unwrap();
        let second = query_permission_audit(&conn, 2, None, None, 2).unwrap();
        let third = query_permission_audit(&conn, 2, None, None, 4).unwrap();

        let times: Vec<i64> = first
            .entries
            .iter()
            .chain(second.entries.iter())
            .chain(third.entries.iter())
            .map(|entry| entry.at_millis)
            .collect();
        assert_eq!(times, vec![4, 3, 2, 1, 0], "三页必须拼成完整且有序的序列");
        assert!(first.has_more, "第一页之后仍有记录");
        assert!(second.has_more);
        assert!(!third.has_more, "末页不得谎报还有更多");
    }

    /// `hasMore` 必须由多取一行判定：`limit` 恰好等于总数时不能说「还有更多」。
    #[test]
    fn has_more_distinguishes_exact_fit_from_truncation() {
        let (conn, _dir) = temp_conn();
        for at in 0..3 {
            insert(&conn, "agent_write", "allowed", at);
        }

        let exact = query_permission_audit(&conn, 3, None, None, 0).unwrap();
        assert_eq!(exact.entries.len(), 3);
        assert!(!exact.has_more, "3 条记录取 3 条不算截断");

        let cut = query_permission_audit(&conn, 2, None, None, 0).unwrap();
        assert_eq!(cut.entries.len(), 2);
        assert!(cut.has_more, "3 条记录取 2 条必须标记还有更多");
    }

    /// 上限档位同样要能报出 `hasMore`：`limit = 500` 时多取的一行不能被夹取吃掉。
    #[test]
    fn has_more_survives_max_limit() {
        let (conn, _dir) = temp_conn();
        for at in 0..=AUDIT_LIMIT_MAX {
            insert(&conn, "agent_write", "allowed", at);
        }
        let page = query_permission_audit(&conn, AUDIT_LIMIT_MAX, None, None, 0).unwrap();
        assert_eq!(page.entries.len(), AUDIT_LIMIT_MAX as usize);
        assert!(page.has_more, "上限档位下仍有更早记录时必须标记");
    }

    /// 偏移量夹取：非正值回落 0，正常值原样透传。
    #[test]
    fn offset_is_clamped_to_zero() {
        assert_eq!(clamp_audit_offset(None), 0);
        assert_eq!(clamp_audit_offset(Some(0)), 0);
        assert_eq!(clamp_audit_offset(Some(-10)), 0);
        assert_eq!(clamp_audit_offset(Some(40)), 40);
    }

    /// 同毫秒多行的顺序必须稳定：靠自增主键做次级排序，避免同 `at_millis` 抖动。
    #[test]
    fn same_millis_rows_have_stable_order() {
        let (conn, _dir) = temp_conn();
        insert(&conn, "first", "allowed", 500);
        insert(&conn, "second", "denied", 500);

        let rows = query_permission_audit(&conn, 100, None, None, 0).unwrap();
        assert_eq!(rows.entries[0].tool, "second", "同毫秒内后写入的行排在前");
        assert_eq!(rows.entries[1].tool, "first");
    }

    /// 上限截断真的作用在 SQL 上，而不是先全查再截断。
    #[test]
    fn limit_cuts_the_result_set() {
        let (conn, _dir) = temp_conn();
        for at in 0..5 {
            insert(&conn, "agent_write", "allowed", at);
        }
        let rows = query_permission_audit(&conn, 2, None, None, 0).unwrap();
        assert_eq!(rows.entries.len(), 2);
        assert_eq!(rows.entries[0].at_millis, 4);
    }
}
