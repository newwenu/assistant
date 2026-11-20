// 工具基础设施的生产装配：注册表 → 分组注册 → 授权/审计 → 审批 → 网关 → 流水线 → 执行器端口。

use std::sync::Arc;
use tauri::Manager;

use assistant_config::app_conf::AppConf;
use assistant_tools::{
    cleanup_expired_results, mcp::McpServerManager, os, sandbox, ApprovalPrompter,
    FileToolResultStorage, InMemoryReadReceiptStore, PermissionAuditStore, PermissionGrantStore,
    ReadReceiptStore, SandboxEnforcement, SandboxHostPaths, SandboxProbe, ToolGateway,
    ToolPipeline, ToolRegistry, ToolResultStorage, ToolTier,
};

use crate::adapters::{
    ApprovalWaitRegistry, DbPermissionAuditStore, DbPermissionGrantStore, PendingApprovalRegistry,
    PendingQuestionRegistry, PipelineToolExecution, TauriApprovalPrompter, TauriQuestionPrompter,
    TimingApprovalPrompter,
};

/// 结果保留策略的启动快照：天数 + 生效时刻（秒）。
///
/// 两处消费方（启动清理与条目策略自述）必须用**同一份**快照：各自读配置会得到
/// 不同的 `effective_since`，条目展示的截止时刻与真实清理时刻就对不上（I61 §3.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResultRetention {
    days: u32,
    effective_since: i64,
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// 读一次配置得到保留策略；**fail-closed**：读不到就按 `0`（不清理），
/// 绝不回退到常量天数——宁可少删，不可多删（I61 G8）。
fn load_result_retention() -> ResultRetention {
    let days = match AppConf::load() {
        Ok(conf) => conf.result_retention_days,
        Err(e) => {
            log::warn!("Failed to load app config for result retention, disable cleanup: {e}");
            0
        }
    };
    ResultRetention {
        days,
        effective_since: now_seconds(),
    }
}

/// 启动时按 `mtime` 年龄清理落盘目录（I46 compact §2.9 D9，门禁 10）。
///
/// 放到独立线程：清理是纯本地 I/O，不应阻塞启动；失败只 warn、绝不抛出。
/// 保留天数每次启动读一次配置（`0` → 禁用），运行期改设置下次重启生效。
fn spawn_result_cleanup(root: std::path::PathBuf, retention: ResultRetention) {
    std::thread::spawn(move || {
        let report = cleanup_expired_results(&root, retention.days);
        log::info!(
            "tool result cleanup: removed {} expired file(s), {} failure(s) (retention {} days)",
            report.removed,
            report.failed,
            retention.days
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

/// 沙箱读拒绝的固定列表（I53 §5.4）：只由本函数构造，不读用户配置、不受模型输入影响。
///
/// 三类来源：凭据目录、本应用写入的受管目录、平台凭据存储目录。
/// 不存在的项**不加入**，且**禁止**创建——探测时点只读系统能力（I53 §6）。
fn sandbox_host_paths(
    dirs: &crate::app_dirs::AppDirs,
    home: Option<&std::path::Path>,
) -> SandboxHostPaths {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(home) = home {
        for name in [".ssh", ".aws", ".gnupg", ".netrc"] {
            candidates.push(home.join(name));
        }
    }
    candidates.extend([
        dirs.data().to_path_buf(),
        dirs.local().to_path_buf(),
        dirs.database(),
        dirs.attachments(),
        dirs.tool_results(),
        dirs.mascot_assets(),
        dirs.logs(),
        dirs.crashes(),
    ]);
    // 系统凭据目录：路径由 `dirs` 给出，不硬编码盘符或用户名。
    #[cfg(windows)]
    if let Some(roaming) = dirs::data_dir() {
        candidates.push(roaming.join("Microsoft").join("Credentials"));
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = home {
        candidates.push(home.join("Library").join("Keychains"));
    }
    candidates.retain(|path| path.exists());
    SandboxHostPaths {
        sensitive_read_deny: candidates,
    }
}

/// 注册期门禁（I53 §4.4 规则 3 / A-S4）：`enforcement == None` 一律不注册 `shell`。
///
/// 判据只读探测结论，不看工具白名单与用户配置：白名单里的 `shell` 名字自然落空，
/// 模型也看不到一个「存在但必然报错」的工具。
fn shell_registration_allowed(probe: &SandboxProbe) -> bool {
    probe.enforcement != SandboxEnforcement::None
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
    // 受管目录只解析一次：沙箱读拒绝列表与落盘根目录共用这一份 `AppDirs`。
    let dirs = crate::app_dirs::AppDirs::resolve()?;

    // I53 C4（§4.4）：探测只在注册期做一次，结论决定 `shell` 是否存在。
    // 结论缓存进进程内 `OnceLock`，运行期只读缓存，不再探测（A-S8）。
    let host = sandbox_host_paths(&dirs, dirs::home_dir().as_deref());
    let probe = sandbox::detect(&host);
    log::info!(
        "sandbox probe: kind={} enforcement={} missing={:?} detail={:?}",
        probe.kind.as_str(),
        probe.enforcement.as_str(),
        probe.missing,
        probe.detail
    );
    if !shell_registration_allowed(probe) {
        log::warn!(
            "sandbox unavailable (detail={:?}): shell tool not registered",
            probe.detail
        );
    }

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
    // ToolBox 直调（`os_tool_exec`）不走网关，但必须落到同一个审计存储：
    // 命令层按类型取用这一份实例，禁止另建第二套。
    handle.manage(audit.clone());
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
    let results_dir = dirs.tool_results();
    let retention = load_result_retention();
    spawn_result_cleanup(results_dir.clone(), retention);
    let storage: Arc<dyn ToolResultStorage> = Arc::new(
        FileToolResultStorage::new(results_dir)
            .with_retention(retention.days, retention.effective_since),
    );
    // C5：`shell` 的注册点必须晚于 registry 与 storage（两者都是它的构造输入），
    // 门禁判据仍在注册期探测结论上：`enforcement == none` 时整组不注册。
    if shell_registration_allowed(probe) {
        os::register_shell_tools(&registry, storage.clone(), probe);
    }
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
    // 升级通道的层级上限同样按会话求值（I58 §3.8 ③）：读会话落库的工具装配，
    // 与 `resolve_available_tools` 的白名单过滤同源；读不到就 fail-closed 到沙箱层级，
    // 绝不因为「读不到配置」而放宽可升级范围。
    let tier_handle = handle.clone();
    let max_tool_tier_source: crate::adapters::MaxToolTierSource =
        Arc::new(move |conversation_id: &str| {
            let state = tier_handle.state::<crate::setup::DbState>();
            let raw = match state.0.lock() {
                Ok(conn) => {
                    assistant_storage::conversations::get_conversation_by_id(&conn, conversation_id)
                        .ok()
                        .flatten()
                        .and_then(|row| row.tool_config_json)
                }
                Err(e) => {
                    log::warn!("Failed to lock db for tool tier resolve: {e}");
                    None
                }
            };
            raw.as_deref()
                .and_then(|raw| {
                    serde_json::from_str::<crate::protocol::ConversationToolConfigDto>(raw).ok()
                })
                .map(|config| ToolTier::from(config.max_tool_tier))
                .unwrap_or(ToolTier::Sandbox)
        });
    let tool_exec = Arc::new(
        PipelineToolExecution::with_workspace_source(pipeline, waits, workspace_source)
            // 行索引缓存根：与结果落盘同为可重建产物，由装配层给出（I58 §3.5）。
            .with_index_root(dirs.indexes())
            // 升级通道：注册表 + 层级来源，`request_tool(detail = "enable")` 的唯一出口（I58 §3.8）。
            .with_escalation(registry.clone(), max_tool_tier_source),
    );
    handle.manage(tool_exec);

    handle.manage(registry);
    log::info!("OS tool registry initialized");

    // P7-C4：MCP 状态机只管注册表与生命周期，进程细节在宿主侧传输层。
    // 先 manage 再拉起：启动序列要用同一份实例，避免两套状态。
    let mcp_manager = Arc::new(McpServerManager::new(
        handle.state::<Arc<ToolRegistry>>().inner().clone(),
        Arc::new(crate::mcp::transport::StdioTransport::new()),
    ));
    handle.manage(mcp_manager);
    crate::mcp::config::start_enabled_servers(handle);
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

    /// A-S4（注册侧）：探测结论为 `None` 时注册表内不得出现 `shell`。
    #[test]
    fn registry_has_no_shell_when_probe_reports_none() {
        let registry = Arc::new(ToolRegistry::new());
        os::register_os_tools(&registry);

        let probe = SandboxProbe::unavailable(
            assistant_tools::SandboxKind::WindowsRestrictedToken,
            "no_write_boundary",
        );
        assert!(!shell_registration_allowed(&probe));
        assert!(
            registry.find("shell").is_none() && registry.find_by_name("shell").is_none(),
            "注册期结论为 None 时 shell 必须不存在"
        );
    }

    #[test]
    fn shell_registration_gate_follows_enforcement() {
        let partial = SandboxProbe {
            kind: assistant_tools::SandboxKind::WindowsRestrictedToken,
            enforcement: SandboxEnforcement::Partial,
            missing: vec![assistant_tools::SandboxDimension::FsRead],
            detail: None,
        };
        assert!(shell_registration_allowed(&partial), "partial 仍注册");

        let full = SandboxProbe {
            kind: assistant_tools::SandboxKind::Landlock,
            enforcement: SandboxEnforcement::Full,
            missing: Vec::new(),
            detail: None,
        };
        assert!(shell_registration_allowed(&full));
    }

    #[test]
    fn sandbox_host_paths_skip_missing_entries_without_creating_them() {
        let dir = TempDir::new("host");
        let data = dir.path().join("data");
        let local = dir.path().join("local");
        let home = dir.path().join("home");
        let dirs = crate::app_dirs::AppDirs::with_bases(data.clone(), local.clone());
        std::fs::create_dir_all(dirs.database()).unwrap();
        std::fs::create_dir_all(home.join(".ssh")).unwrap();

        let paths = sandbox_host_paths(&dirs, Some(&home));
        assert!(paths.sensitive_read_deny.contains(&dirs.database()));
        assert!(paths.sensitive_read_deny.contains(&home.join(".ssh")));
        assert!(
            !paths.sensitive_read_deny.contains(&home.join(".aws")),
            "不存在的项不加入"
        );
        assert!(!local.exists(), "探测期不得创建任何目录");
        assert!(
            paths.sensitive_read_deny.iter().all(|path| path.exists()),
            "列表内每一项都必须已存在"
        );
    }
}
