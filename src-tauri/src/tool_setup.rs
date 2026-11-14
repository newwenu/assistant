// 工具基础设施的生产装配：注册表 → 分组注册 → 授权/审计 → 审批 → 网关 → 流水线 → 执行器端口。

use std::sync::Arc;
use tauri::Manager;

use assistant_config::app_conf::AppConf;
use assistant_tools::{
    cleanup_expired_results, os, ApprovalPrompter, FileToolResultStorage, InMemoryReadReceiptStore,
    PermissionAuditStore, PermissionGrantStore, ReadReceiptStore, ToolGateway, ToolPipeline,
    ToolRegistry, ToolResultStorage,
};

use crate::adapters::{
    ApprovalWaitRegistry, DbPermissionAuditStore, DbPermissionGrantStore, PendingApprovalRegistry,
    PendingQuestionRegistry, PipelineToolExecution, TauriApprovalPrompter, TauriQuestionPrompter,
    TimingApprovalPrompter,
};

/// 启动时按 `mtime` 年龄清理落盘目录（I46 compact §2.9 D9，门禁 10）。
///
/// 放到独立线程：清理是纯本地 I/O，不应阻塞启动；失败只 warn、绝不抛出。
/// 保留天数每次启动读一次配置（`0` → 禁用），运行期改设置下次重启生效。
fn spawn_result_cleanup(root: std::path::PathBuf) {
    let retention_days = match AppConf::load() {
        Ok(conf) => conf.result_retention_days,
        Err(e) => {
            log::warn!("Failed to load app config for result retention: {e}");
            assistant_config::constant::RESULT_RETENTION_DAYS
        }
    };
    std::thread::spawn(move || {
        let report = cleanup_expired_results(&root, retention_days);
        log::info!(
            "tool result cleanup: removed {} expired file(s), {} failure(s) (retention {} days)",
            report.removed,
            report.failed,
            retention_days
        );
    });
}

/// 审批等待上限的兜底值：配置 `approvalTimeoutSecs` 缺省或非法时用它。
/// 超时仍归一为 `Deny`（I38a §2.8），不得默认放行。
const APPROVAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// 审批超时的取值来源：每次审批现读配置，改设置后对下一次审批立即生效。
/// `Some(0)` 在保存时已被 `AppConf::validate` 拒绝；手改配置文件写进 0 时按兜底值处理，
/// 不允许「零超时」这种既非放行也非等待的语义。
fn approval_timeout_from_conf() -> std::time::Duration {
    match AppConf::load() {
        Ok(conf) => match conf.approval_timeout_secs {
            Some(secs) if secs > 0 => std::time::Duration::from_secs(u64::from(secs)),
            Some(_) => {
                log::warn!("approval_timeout_secs = 0 is invalid, falling back to default");
                APPROVAL_TIMEOUT
            }
            None => APPROVAL_TIMEOUT,
        },
        Err(e) => {
            log::warn!("Failed to load app config for approval timeout: {e}");
            APPROVAL_TIMEOUT
        }
    }
}

/// 提问等待上限的兜底值：配置 `userQuestionTimeoutSecs` 缺省时用它。
/// 与审批不同，`Some(0)` 是**合法**取值，表示不设 deadline（I38c §11 OQ5）。
const USER_QUESTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

/// 提问超时的取值来源：每次提问现读配置，改设置后下一次提问立即生效。
///
/// 返回 `None` 表示**不设 deadline**（`Some(0)` 或配置读取失败降级为兜底值之外的
/// 唯一例外）；读取失败按兜底 180s 处理，不静默变成「无限等待」。
fn user_question_timeout_from_conf() -> Option<std::time::Duration> {
    match AppConf::load() {
        Ok(conf) => match conf.user_question_timeout_secs {
            Some(0) => None,
            Some(secs) => Some(std::time::Duration::from_secs(u64::from(secs))),
            None => Some(USER_QUESTION_TIMEOUT),
        },
        Err(e) => {
            log::warn!("Failed to load app config for user question timeout: {e}");
            Some(USER_QUESTION_TIMEOUT)
        }
    }
}

/// 工作区绑定值的**唯一校验实现**（I38h §1）：写入校验与读取校验必须共用这一份，
/// 否则会出现「保存时说合法、执行时说越界」。
///
/// 返回 `None` 覆盖三种情形：空串（未配置）、脏值（已绑定但当前不可用）、
/// 非 UTF-8 路径（无法作为 wire / DB 文本承载，按不可用处理）。
/// 前两者在 DB 中可区分（空串 vs 非空），在执行期不可区分（都无可用工作区）。
///
/// 存储值保持 `std::fs::canonicalize` 的原样（Windows 上可能带 `\\?\` 前缀）：
/// 该值是绑定比较与解析的唯一键，不做展示层美化；`\\?\` 的剥离只发生在展示侧。
///
/// 不用 `to_string_lossy`：lossy 会把非 UTF-8 路径改写成另一个字符串，
/// 导致「校验的是 A、存下来的是 B」，绑定比较随之失效（I44 §10.3）。
pub(crate) fn validate_workspace_root(raw: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    let path = std::path::Path::new(raw);
    if !path.is_absolute() {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    if !canonical.is_dir() {
        return None;
    }
    canonical.to_str().map(str::to_string)
}

/// 工作区根目录的**唯一解析入口**（I38h §3）：4 处消费点全部调这一份。
///
/// 只读 `conversations.workspace_root`；不读 `AppConf`、不读目录簿、不读环境变量、
/// **绝不退回进程 cwd**。读取失败 fail-closed 到「未配置」，并留 warn 供排查。
pub fn resolve_workspace(conn: &rusqlite::Connection, conversation_id: &str) -> Option<String> {
    match assistant_storage::workspaces::get_conversation_workspace(conn, conversation_id) {
        Ok(Some(raw)) => {
            let resolved = validate_workspace_root(&raw);
            if resolved.is_none() && !raw.trim().is_empty() {
                log::debug!(
                    "workspace binding for conversation {conversation_id} is unusable: {raw}"
                );
            }
            resolved
        }
        Ok(None) => None,
        Err(e) => {
            log::warn!("Failed to read workspace binding for conversation {conversation_id}: {e}");
            None
        }
    }
}

/// 装配工具基础设施，并把所有实例 `manage` 进 Tauri 状态。
///
/// 顺序固定，后一步依赖前一步的产物；网关只认 `ApprovalPrompter` trait，
/// 计时装饰器对内层透明。命令层只按类型取用已装配的实例，不重复构造。
///
/// 注册门禁（I38c §3 / §7.1）：生产放开 `fs_link` + 只读组 + 修改组 + 交互组 + 危险组。
/// 修改组（P5-R2）随 P6 审批 UI 启用（I38d C9.5a）；危险组 `delete`（P5-R3）的
/// 三项门禁（P2 控制层可用、前端审批卡片上线、A2 审批三态验收）已于 2026-11-09
/// 全部满足，故此处接入。
pub fn init(handle: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let registry = Arc::new(ToolRegistry::new());
    os::register_os_tools(&registry);

    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    os::register_readonly_tools(&registry, receipts.clone());
    // P5-R2：修改组（write / edit / mkdir / move）在 P6 审批 UI 上线后启用。
    os::register_write_tools(&registry, receipts.clone());
    // P5-R3：危险组（delete）。I38c §7.1 三项门禁已满足，2026-11-09 接入。
    os::register_dangerous_tools(&registry, receipts.clone());
    handle.manage(receipts);

    // C8 交互组：`ask_user` 只面向 Agent（AGENT_ONLY），等待由工具自管超时，
    // 超时值每次现读全局配置（`None` → 180s、`0` → 不限时）。
    let question_pending = PendingQuestionRegistry::new();
    handle.manage(question_pending.clone());
    os::register_interactive_tools(
        &registry,
        Arc::new(TauriQuestionPrompter::new(handle.clone(), question_pending)),
        Arc::new(user_question_timeout_from_conf),
    );

    // C4.18：P2 遗留端口的生产装配。
    let grants: Arc<dyn PermissionGrantStore> =
        Arc::new(DbPermissionGrantStore::new(handle.clone()));
    let audit: Arc<dyn PermissionAuditStore> =
        Arc::new(DbPermissionAuditStore::new(handle.clone()));
    let waits = ApprovalWaitRegistry::new();
    let pending = PendingApprovalRegistry::new();
    handle.manage(pending.clone());
    let prompter: Arc<dyn ApprovalPrompter> = Arc::new(TimingApprovalPrompter::with_registry(
        Arc::new(TauriApprovalPrompter::new(handle.clone(), pending)),
        waits.clone(),
    ));
    let gateway = Arc::new(ToolGateway::with_timeout_source(
        grants,
        audit,
        prompter,
        Arc::new(approval_timeout_from_conf),
    ));

    // 落盘根目录必须是稳定路径：它会写进 `<persisted-output path="…">`，
    // 跨进程重放一致性依赖它（§4.5），不得用临时目录。
    let results_dir = crate::app_dirs::AppDirs::resolve()?.tool_results();
    spawn_result_cleanup(results_dir.clone());
    let storage: Arc<dyn ToolResultStorage> = Arc::new(FileToolResultStorage::new(results_dir));
    let pipeline = Arc::new(ToolPipeline::new(registry.clone(), gateway, storage));

    // 工作区根目录按 I38h §4 #1 按会话解析：每次执行现算，切换绑定后下一轮立即生效。
    // 这里只登记「共享基础设施 + 默认策略」的基底实例；每轮实际使用的执行器
    // 由 `with_policy` 按会话有效工具配置派生（C7），避免全局单例缓存策略。
    let workspace_handle = handle.clone();
    let workspace_source: crate::adapters::WorkspaceRootSource =
        Arc::new(move |conversation_id: &str| {
            let state = workspace_handle.state::<crate::setup::DbState>();
            let resolved = match state.0.lock() {
                Ok(conn) => resolve_workspace(&conn, conversation_id),
                Err(e) => {
                    log::warn!("Failed to lock db for workspace resolve: {e}");
                    None
                }
            };
            resolved
        });
    let tool_exec = Arc::new(PipelineToolExecution::with_workspace_source(
        pipeline,
        waits,
        workspace_source,
    ));
    handle.manage(tool_exec);

    handle.manage(registry);
    log::info!("OS tool registry initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_storage::conversations::{insert_conversation, ConversationRow};

    /// 测试用临时目录：不引入新依赖，退出时自清理。
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("assistant-ws-{}-{}", std::process::id(), tag));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn conn() -> (rusqlite::Connection, TempDir) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let tag = format!("db{}", SEQ.fetch_add(1, Ordering::Relaxed));
        let dir = TempDir::new(&tag);
        let conn = assistant_storage::db::init_db(&dir.path().join("workspace.db")).unwrap();
        (conn, dir)
    }

    fn conversation(id: &str, workspace_root: &str) -> ConversationRow {
        ConversationRow {
            id: id.to_string(),
            title: "t".to_string(),
            title_source: "placeholder".to_string(),
            provider_id: None,
            model_id: "m".to_string(),
            system_prompt: None,
            is_pinned: false,
            sort_order: 0,
            generation: 0,
            created_at: 0,
            updated_at: 0,
            is_full: false,
            active_branch_id: None,
            mode_id: None,
            tool_config_json: None,
            workspace_root: workspace_root.to_string(),
        }
    }

    #[test]
    fn validate_rejects_empty_relative_and_missing() {
        assert_eq!(validate_workspace_root(""), None);
        assert_eq!(validate_workspace_root("   "), None);
        assert_eq!(validate_workspace_root("relative/dir"), None);
        assert_eq!(validate_workspace_root("C:/definitely/not/here"), None);

        let dir = TempDir::new("file");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert_eq!(
            validate_workspace_root(file.to_str().unwrap()),
            None,
            "文件不是目录"
        );
    }

    #[test]
    fn validate_returns_canonical_for_real_directory() {
        let dir = TempDir::new("canonical");
        let raw = dir.path().to_str().unwrap().to_string();
        let canonical = dir.path().canonicalize().unwrap();
        assert_eq!(
            validate_workspace_root(&raw).as_deref(),
            Some(canonical.to_str().unwrap())
        );
    }

    #[test]
    fn resolve_unconfigured_is_none() {
        let (conn, _db) = conn();
        insert_conversation(&conn, &conversation("c1", "")).unwrap();
        assert_eq!(resolve_workspace(&conn, "c1"), None);
    }

    #[test]
    fn resolve_valid_binding_is_canonical() {
        let (conn, _db) = conn();
        let dir = TempDir::new("valid");
        let canonical = dir.path().canonicalize().unwrap();
        insert_conversation(&conn, &conversation("c1", dir.path().to_str().unwrap())).unwrap();
        assert_eq!(
            resolve_workspace(&conn, "c1").as_deref(),
            Some(canonical.to_str().unwrap())
        );
    }

    #[test]
    fn resolve_dirty_binding_fails_closed_without_falling_back_to_cwd() {
        let (conn, _db) = conn();
        insert_conversation(&conn, &conversation("c1", "relative/dir")).unwrap();
        assert_eq!(resolve_workspace(&conn, "c1"), None);

        let dir = TempDir::new("gone");
        let gone = dir.path().join("gone");
        insert_conversation(&conn, &conversation("c2", gone.to_str().unwrap())).unwrap();
        assert_eq!(resolve_workspace(&conn, "c2"), None, "目录已不存在");
    }

    #[test]
    fn resolve_missing_conversation_is_none() {
        let (conn, _db) = conn();
        assert_eq!(resolve_workspace(&conn, "missing"), None);
    }

    #[test]
    fn resolve_does_not_read_workspaces_book() {
        let (conn, _db) = conn();
        let dir = TempDir::new("book");
        let path = dir.path().to_str().unwrap().to_string();
        assistant_storage::workspaces::upsert(&conn, &path, None).unwrap();
        insert_conversation(&conn, &conversation("c1", "")).unwrap();
        assert_eq!(
            resolve_workspace(&conn, "c1"),
            None,
            "目录簿只是展示缓存，不参与解析"
        );
    }
}
