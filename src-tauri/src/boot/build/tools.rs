// 参考: docs/store/I75-startup-layering-spec.md §9 #8 — 工具运行时装配
//
// 装配函数化：本文件只做构造与注入，不再把 `ToolRegistry` / `PipelineToolExecution`
// `manage` 进 Tauri 状态；命令层与适配层一律经 `boot::tools` / `boot::tool_exec` 取用（C3）。
// 仍留在 Tauri 状态的子实例（审批等待表、审计存储、MCP 管理器）在 P3 并入 `access.rs`。

use std::sync::Arc;

use tauri::Manager;

use crate::adapters::{
    ApprovalWaitRegistry, DbPermissionAuditStore, DbPermissionGrantStore, PendingApprovalRegistry,
    PendingQuestionRegistry, PipelineToolExecution, TauriApprovalPrompter, TauriQuestionPrompter,
    TimingApprovalPrompter,
};
use crate::boot::ctx::StartupCtx;
use crate::boot::resources::{CapErr, CapValue};
use assistant_tools::{
    mcp::McpServerManager, os, sandbox, ApprovalPrompter, FileToolResultStorage,
    FileToolScratchStore, InMemoryReadReceiptStore, PermissionAuditStore, PermissionGrantStore,
    ReadReceiptStore, SandboxEnforcement, SandboxHostPaths, SandboxProbe, ToolGateway,
    ToolPipeline, ToolRegistry, ToolResultStorage, ToolScratchStore, ToolTier,
};

/// 沙箱读拒绝的固定列表（I53 §5.4 / I73 §3.8 W3）：只由本函数构造，不读用户配置、不受模型输入影响。
///
/// 三类来源：凭据目录、本应用写入的受管目录、平台凭据存储目录。
/// 「本应用写入的受管目录」按登记表**逐项枚举**，不再整树拒绝 `data()` / `local()`：
/// 整树拒绝会把会话临时区（同为受管目录，但必须是沙箱可写根）一并罩住，
/// 触发 `writable_root_overlaps_sensitive` 直接判负。逐项枚举反而保留默认保护——
/// 登记表新增条目自动进入拒绝列表，无需维护第二份清单。
/// 不存在的项**不加入**，且**禁止**创建——探测时点只读系统能力（I53 §6）。
pub(crate) fn sandbox_host_paths(
    dirs: &crate::app_dirs::AppDirs,
    home: Option<&std::path::Path>,
) -> SandboxHostPaths {
    use crate::app_dirs::registry::{CoreEntry, MANAGED_ROOTS};

    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(home) = home {
        for name in [".ssh", ".aws", ".gnupg", ".netrc"] {
            candidates.push(home.join(name));
        }
    }
    // 受管目录逐项枚举：`ToolScratch` 是沙箱可写根，排除在拒读列表之外。
    for root in MANAGED_ROOTS {
        if root.entry == CoreEntry::ToolScratch {
            continue;
        }
        candidates.push(dirs.managed_path_of(root.entry));
    }
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
pub(crate) fn shell_registration_allowed(probe: &SandboxProbe) -> bool {
    probe.enforcement != SandboxEnforcement::None
}

/// 装配工具基础设施：本函数只返回 `ToolRegistry`（落进 `tool_runtime` 槽），
/// 执行器端口经 `ctx.inject` 登记为 `tool_exec` 槽，不再各自 `manage`。
///
/// 顺序固定，后一步依赖前一步的产物；网关只认 `ApprovalPrompter` trait，
/// 计时装饰器对内层透明。命令层只按类型取用已装配的实例，不重复构造。
///
/// 注册门禁（I38c §3 / §7.1）：生产放开 `fs_link` + 只读组 + 修改组 + 交互组 + 危险组。
/// 修改组（P5-R2）随 P6 审批 UI 启用（I38d C9.5a）；危险组 `delete`（P5-R3）的
/// 三项门禁（P2 控制层可用、前端审批卡片上线、A2 审批三态验收）已于 2026-11-09
/// 全部满足，故此处接入。
pub fn build(ctx: &StartupCtx<'_>) -> Result<CapValue, CapErr> {
    let dirs = ctx.dirs()?;
    let handle = ctx.app;
    // 受管目录由 `dirs` 表项唯一解析后传入（N15）：沙箱读拒绝列表与落盘根目录共用这一份。

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
    os::register_bridge_tools(&registry);

    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    os::register_readonly_tools(&registry, receipts.clone());
    // P5-R2：修改组（write / edit / mkdir / move）在 P6 审批 UI 上线后启用。
    os::register_write_tools(&registry, receipts.clone());
    // P5-R3：危险组（delete）。I38c §7.1 三项门禁已满足，2026-11-09 接入。
    os::register_dangerous_tools(&registry, receipts.clone());

    // C8 交互组：`ask_user` 只面向 Agent（AGENT_ONLY），等待由工具自管超时，
    // 超时值每次现读全局配置（`None` → 180s、`0` → 不限时）。
    let question_pending = PendingQuestionRegistry::new();
    os::register_interactive_tools(
        &registry,
        Arc::new(TauriQuestionPrompter::new(
            handle.clone(),
            question_pending.clone(),
        )),
        Arc::new(crate::tool_setup::user_question_timeout_from_conf),
    );
    ctx.inject("pending_question", Arc::new(question_pending));

    // C4.18：P2 遗留端口的生产装配。
    let grants: Arc<dyn PermissionGrantStore> =
        Arc::new(DbPermissionGrantStore::new(handle.clone()));
    let audit_store = Arc::new(DbPermissionAuditStore::new(handle.clone()));
    let audit: Arc<dyn PermissionAuditStore> = audit_store.clone();
    // ToolBox 直调（`os_tool_exec`）不走网关，但必须落到同一个审计存储：
    // 命令层经 `access::permission_audit` 取用这一份实例，禁止另建第二套。
    ctx.inject("permission_audit", audit_store);
    let waits = ApprovalWaitRegistry::new();
    let pending = PendingApprovalRegistry::new();
    let prompter: Arc<dyn ApprovalPrompter> = Arc::new(TimingApprovalPrompter::with_registry(
        Arc::new(TauriApprovalPrompter::new(handle.clone(), pending.clone())),
        waits.clone(),
    ));
    ctx.inject("pending_approval", Arc::new(pending));
    let gateway = Arc::new(ToolGateway::with_timeout_source(
        grants,
        audit,
        prompter,
        Arc::new(crate::tool_setup::approval_timeout_from_conf),
    ));

    // 落盘根目录必须是稳定路径：它会写进 `<persisted-output path="…">`，
    // 跨进程重放一致性依赖它（§4.5），不得用临时目录。
    let results_dir = dirs.tool_results();
    let retention = crate::tool_setup::load_result_retention();
    let storage: Arc<dyn ToolResultStorage> = Arc::new(
        FileToolResultStorage::new(results_dir)
            .with_retention(retention.days, retention.effective_since),
    );
    // 临时区所有者（I77 §3.2 / C5）：装配期创建**唯一**实例，同时交给 shell 工具与
    // 执行器；两侧各 `new` 一份会让建目录与观测落到不同基根。
    let scratch: Arc<dyn ToolScratchStore> =
        Arc::new(FileToolScratchStore::new(dirs.tool_scratch()));
    // C5：`shell` 的注册点必须晚于 registry 与 storage（两者都是它的构造输入），
    // 门禁判据仍在注册期探测结论上：`enforcement == none` 时整组不注册。
    if shell_registration_allowed(probe) {
        os::register_shell_tools(&registry, storage.clone(), scratch.clone(), probe);
    }
    let pipeline = Arc::new(ToolPipeline::new(registry.clone(), gateway, storage));

    // 工作区根目录按 I38h §4 #1 按会话解析：每次执行现算，切换绑定后下一轮立即生效。
    // 这里只登记「共享基础设施 + 默认策略」的基底实例；每轮实际使用的执行器
    // 由 `with_policy` 按会话有效工具配置派生（C7），避免全局单例缓存策略。
    let workspace_handle = handle.clone();
    let workspace_source: crate::adapters::WorkspaceRootSource =
        Arc::new(move |conversation_id: &str| {
            let state = workspace_handle.state::<crate::db_state::DbState>();
            let resolved = match state.0.lock() {
                Ok(conn) => crate::tool_setup::resolve_workspace(&conn, conversation_id),
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
            let state = tier_handle.state::<crate::db_state::DbState>();
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
            // shell 临时区所有者：与注册处共享同一实例，执行器只把它搬进上下文（I77 C4）。
            .with_tool_scratch(scratch)
            // 升级通道：注册表 + 层级来源，`request_tool(detail = "enable")` 的唯一出口（I58 §3.8）。
            .with_escalation(registry.clone(), max_tool_tier_source),
    );
    // 执行器端口占独立槽：`tool_runtime` 只返回注册表，两者都由 `ensure` 幂等装配。
    ctx.inject("tool_exec", tool_exec);

    // P7-C4：MCP 状态机只管注册表与生命周期，进程细节在宿主侧传输层。
    // 经槽共享同一份实例：命令层走 `access::mcp_manager`，`mcp_start` 走 `ctx.get`，
    // 不 `manage` 以保证失败重试时不会重复注册（I6）。
    let mcp_manager = Arc::new(McpServerManager::new(
        registry.clone(),
        Arc::new(crate::mcp::transport::StdioTransport::new()),
    ));
    ctx.inject("mcp_manager", mcp_manager);
    // 探测结论上面已按 `info` 落一行；这里补的是阶段行没有的上下文（工具数），
    // 只在 dev 档可见，release 档静默。
    log::debug!(
        "OS tool registry initialized: {} tool(s)",
        registry.tool_ids().len()
    );
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_tools::SandboxKind;

    /// 测试用临时目录：不引入新依赖，退出时自清理。
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "assistant-tools-{}-{}",
                std::process::id(),
                tag
            ));
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

    /// A-S4（注册侧）：探测结论为 `None` 时注册表内不得出现 `shell`。
    #[test]
    fn registry_has_no_shell_when_probe_reports_none() {
        let registry = Arc::new(ToolRegistry::new());
        os::register_bridge_tools(&registry);

        let probe =
            SandboxProbe::unavailable(SandboxKind::WindowsRestrictedToken, "no_write_boundary");
        assert!(!shell_registration_allowed(&probe));
        assert!(
            registry.find("shell").is_none() && registry.find_by_name("shell").is_none(),
            "注册期结论为 None 时 shell 必须不存在"
        );
    }

    #[test]
    fn shell_registration_gate_follows_enforcement() {
        let partial = SandboxProbe {
            kind: SandboxKind::WindowsRestrictedToken,
            enforcement: SandboxEnforcement::Partial,
            missing: vec![assistant_tools::SandboxDimension::FsRead],
            detail: None,
        };
        assert!(shell_registration_allowed(&partial), "partial 仍注册");

        let full = SandboxProbe {
            kind: SandboxKind::Landlock,
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
