use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use assistant_tools::gateway::{
    redact_secrets, request_with_timeout, ApprovalContext, ApprovalDecision,
    ApprovalExtensionSeverity, ApprovalKey, ApprovalPrompter, ApprovalReason, ApprovalRequest,
    ApprovalScope, ApprovalWait, AuditEntry, AuditOutcome, ChildGrantStore, GrantScope,
    InMemoryAuditStore, InMemoryGrantStore, PermissionAuditStore, PermissionGrantStore,
    ScriptedApprovalPrompter, ToolGateway,
};
use assistant_tools::os::agent_read::AgentReadTool;
use assistant_tools::os::path::{canonicalize_lenient, ensure_within_root};
use assistant_tools::storage::{
    FileToolResultStorage, InMemoryReadReceiptStore, PersistToolResult, ToolResultStorage,
};
use assistant_tools::{
    policy_from_agent_mode, ApprovalLevel, ApprovalPreview, BehaviorHints, GatewayDecision,
    PermissionConstraintLayer, PermissionEffect, PermissionIntent, PermissionRule,
    ResolvedPermissionPolicy, SandboxEnforcement, Tool, ToolCallSource, ToolResult, ToolTier,
    ToolUseContext,
};
use serde_json::{json, Value};
use tempfile::tempdir;

mod support;

use support::link::file_link;

fn ctx() -> ToolUseContext {
    ToolUseContext::default()
}

/// 跨平台绝对路径夹具：路径键要求可归一的绝对路径。
///
/// 写死 `C:/ws/...` 在 Unix 上是相对路径，`canonical_path_for_key` 归一失败
/// 会返回空键，于是授权不落库、缓存永不命中，用例在 Linux/macOS 上必挂。
fn ws_path(name: &str) -> String {
    std::env::temp_dir()
        .join("assistant-gateway-tests")
        .join(name)
        .display()
        .to_string()
}

fn path_key(tool: &str, action: &str, resource: &str) -> Option<ApprovalKey> {
    ApprovalKey::from_intent(
        tool,
        &PermissionIntent::new(action, resource),
        resource,
        &Value::Null,
        &ctx(),
        ToolTier::Os,
        None,
    )
}

fn command_key(
    resource: &str,
    input: &Value,
    context: &ToolUseContext,
    tier: ToolTier,
) -> Option<ApprovalKey> {
    command_key_with_enforcement(resource, input, context, tier, None)
}

fn command_key_with_enforcement(
    resource: &str,
    input: &Value,
    context: &ToolUseContext,
    tier: ToolTier,
    enforcement: Option<SandboxEnforcement>,
) -> Option<ApprovalKey> {
    ApprovalKey::from_intent(
        "bash",
        &PermissionIntent::new("execute", resource),
        resource,
        input,
        context,
        tier,
        enforcement,
    )
}

fn approval_request(timeout: Duration) -> ApprovalRequest {
    ApprovalRequest {
        tool_call_id: "call-1".to_string(),
        conversation_id: Some("conv-1".to_string()),
        turn_id: Some("turn-1".to_string()),
        iteration: Some(1),
        tool_name: "fs_edit".to_string(),
        arguments: json!({ "path": "a.txt" }),
        intents: vec![PermissionIntent::new("edit", "a.txt")],
        timeout,
        context: ApprovalContext::minimal(ToolTier::Os, Default::default(), Vec::new()),
    }
}

#[test]
fn path_key_normalizes_dotdot() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    let file = sub.join("a.txt");
    std::fs::write(&file, b"x").unwrap();

    let direct = path_key("fs_read", "read", file.to_str().unwrap()).unwrap();
    let dotted = path_key(
        "fs_read",
        "read",
        sub.join("..").join("sub").join("a.txt").to_str().unwrap(),
    )
    .unwrap();

    assert_eq!(direct, dotted);
}

#[test]
fn path_key_resolves_symlink_target() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("real.txt");
    std::fs::write(&target, b"x").unwrap();
    let link = dir.path().join("link.txt");

    if !file_link(&target, &link) {
        eprintln!("symlink unavailable on this host; skipping");
        return;
    }

    let real = path_key("fs_read", "read", target.to_str().unwrap()).unwrap();
    let via_link = path_key("fs_read", "read", link.to_str().unwrap()).unwrap();

    assert_eq!(real, via_link);
    match &real.scope {
        ApprovalScope::Path { canonical } => {
            assert!(
                !canonical.starts_with(r"\\?\"),
                "verbatim prefix must be stripped: {}",
                canonical
            );
            assert!(
                canonical.ends_with("real.txt"),
                "resolved target: {}",
                canonical
            );
        }
        other => panic!("expected path scope, got {:?}", other),
    }
}

#[test]
fn path_key_rejects_empty_resource() {
    assert!(path_key("fs_read", "read", "   ").is_none());
}

#[cfg(windows)]
#[test]
fn path_key_rejects_target_with_unpaired_surrogate() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let dir = tempdir().unwrap();
    let mut wide: Vec<u16> = "bad".encode_utf16().collect();
    wide.push(0xD800);
    wide.extend(".txt".encode_utf16());
    let target = dir.path().join(OsString::from_wide(&wide));

    if std::fs::write(&target, b"x").is_err() {
        eprintln!("cannot create non-UTF-8 file name on this volume; skipping");
        return;
    }

    let link = dir.path().join("link.txt");
    if !file_link(&target, &link) {
        eprintln!("symlink unavailable on this host; skipping");
        return;
    }

    assert!(
        path_key("fs_read", "read", link.to_str().unwrap()).is_none(),
        "non-UTF-8 canonical target must not produce a reusable key"
    );
}

#[cfg(unix)]
#[test]
fn path_key_rejects_target_with_invalid_utf8() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = tempdir().unwrap();
    let target = dir.path().join(OsStr::from_bytes(b"bad\xff.txt"));

    if std::fs::write(&target, b"x").is_err() {
        eprintln!("cannot create non-UTF-8 file name; skipping");
        return;
    }

    let link = dir.path().join("link.txt");
    if !file_link(&target, &link) {
        eprintln!("symlink unavailable on this host; skipping");
        return;
    }

    assert!(
        path_key("fs_read", "read", link.to_str().unwrap()).is_none(),
        "non-UTF-8 canonical target must not produce a reusable key"
    );
}

#[test]
fn command_key_requires_full_argv_match() {
    let short = command_key("git status", &Value::Null, &ctx(), ToolTier::Os).unwrap();
    let long = command_key("git status --short", &Value::Null, &ctx(), ToolTier::Os).unwrap();

    assert_ne!(short, long);
    assert_ne!(short.scope, long.scope);
}

#[test]
fn command_key_includes_cwd_and_tier() {
    let dir = tempdir().unwrap();
    let other = tempdir().unwrap();
    let first = ToolUseContext {
        workspace_root: Some(dir.path().to_string_lossy().into_owned()),
        ..Default::default()
    };
    let second = ToolUseContext {
        workspace_root: Some(other.path().to_string_lossy().into_owned()),
        ..Default::default()
    };

    let a = command_key("git status", &Value::Null, &first, ToolTier::Os).unwrap();
    let b = command_key("git status", &Value::Null, &second, ToolTier::Os).unwrap();
    let c = command_key("git status", &Value::Null, &first, ToolTier::Sandbox).unwrap();

    assert_ne!(a, b, "cwd must be part of the key");
    assert_ne!(a, c, "sandbox tier must be part of the key");
}

#[test]
fn command_key_rejects_unbalanced_quotes() {
    assert!(command_key("git \"status", &Value::Null, &ctx(), ToolTier::Os).is_none());
}

#[test]
fn command_key_splits_quoted_arguments() {
    let key = command_key("git commit -m 'a b c'", &Value::Null, &ctx(), ToolTier::Os).unwrap();
    match key.scope {
        ApprovalScope::Command { argv, .. } => {
            assert_eq!(argv, vec!["git", "commit", "-m", "a b c"]);
        }
        other => panic!("expected command scope, got {:?}", other),
    }
}

#[test]
fn endpoint_key_normalizes_host_case() {
    let key = ApprovalKey::from_intent(
        "http_fetch",
        &PermissionIntent::new("network", "https://Example.COM:8443/path"),
        "https://Example.COM:8443/path",
        &json!({ "method": "post" }),
        &ctx(),
        ToolTier::External,
        None,
    )
    .unwrap();

    assert_eq!(
        key.scope,
        ApprovalScope::Endpoint {
            host: "example.com".to_string(),
            method: "POST".to_string(),
        }
    );
}

#[test]
fn opaque_key_uses_resource_fingerprint() {
    let key = ApprovalKey::from_intent(
        "mcp_tool",
        &PermissionIntent::new("invoke", "mcp:filesystem"),
        "mcp:filesystem",
        &Value::Null,
        &ctx(),
        ToolTier::External,
        None,
    )
    .unwrap();

    assert_eq!(
        key.scope,
        ApprovalScope::Opaque {
            fingerprint: "mcp:filesystem".to_string(),
        }
    );
}

#[tokio::test]
async fn empty_key_never_hits_the_cache() {
    let store = InMemoryGrantStore::default();
    let empty = path_key("fs_read", "read", "   ");
    assert!(empty.is_none());
    assert!(store.is_empty());

    let real = path_key("fs_read", "read", &ws_path("a.txt")).unwrap();
    store
        .save_grant(&real, GrantScope::Global, None)
        .await
        .unwrap();
    assert!(store.is_granted(&real, None).await);
    assert_eq!(store.len(), 1);
}

fn mcp_key(resource: &str, tool: &str, input: &Value) -> Option<ApprovalKey> {
    ApprovalKey::from_intent(
        tool,
        &PermissionIntent::new("mcp", resource),
        resource,
        input,
        &ctx(),
        ToolTier::External,
        None,
    )
}

fn mcp_scope(key: &ApprovalKey) -> &ApprovalScope {
    match &key.scope {
        scope @ ApprovalScope::Mcp { .. } => scope,
        other => panic!("expected mcp scope, got {:?}", other),
    }
}

#[test]
fn mcp_key_carries_all_four_fields() {
    let key = mcp_key(
        "mcp:srv-1:cfg-abc",
        "mcp__srv-1__search",
        &json!({ "query": "rust" }),
    )
    .unwrap();

    let ApprovalScope::Mcp {
        server_id,
        tool,
        args_fingerprint,
        config_hash,
    } = mcp_scope(&key).clone()
    else {
        unreachable!()
    };

    assert_eq!(server_id, "srv-1");
    assert_eq!(tool, "search");
    assert_eq!(config_hash, "cfg-abc");
    assert_eq!(args_fingerprint.len(), 64);
    assert!(args_fingerprint.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert_eq!(key.tool, "mcp__srv-1__search");
}

#[test]
fn mcp_key_requires_object_input() {
    assert!(mcp_key("mcp:srv-1:cfg-abc", "mcp__srv-1__search", &Value::Null).is_none());
    assert!(mcp_key("mcp:srv-1:cfg-abc", "mcp__srv-1__search", &json!([1, 2])).is_none());
}

#[test]
fn mcp_args_fingerprint_is_canonical() {
    let left = mcp_key(
        "mcp:srv-1:cfg-abc",
        "mcp__srv-1__search",
        &json!({ "b": 1, "a": { "y": 2, "x": 3 } }),
    )
    .unwrap();
    let right = mcp_key(
        "mcp:srv-1:cfg-abc",
        "mcp__srv-1__search",
        &json!({ "a": { "x": 3, "y": 2 }, "b": 1 }),
    )
    .unwrap();
    assert_eq!(left, right);

    let different = mcp_key(
        "mcp:srv-1:cfg-abc",
        "mcp__srv-1__search",
        &json!({ "a": { "x": 3, "y": 2 }, "b": 2 }),
    )
    .unwrap();
    assert_ne!(left, different);
}

#[test]
fn mcp_array_order_changes_fingerprint() {
    let left = mcp_key(
        "mcp:srv-1:cfg-abc",
        "mcp__srv-1__search",
        &json!({ "items": [1, 2] }),
    )
    .unwrap();
    let right = mcp_key(
        "mcp:srv-1:cfg-abc",
        "mcp__srv-1__search",
        &json!({ "items": [2, 1] }),
    )
    .unwrap();
    assert_ne!(left, right);
}

#[test]
fn mcp_key_differs_on_config_hash() {
    let args = json!({ "query": "rust" });
    let old = mcp_key("mcp:srv-1:cfg-old", "mcp__srv-1__search", &args).unwrap();
    let new = mcp_key("mcp:srv-1:cfg-new", "mcp__srv-1__search", &args).unwrap();
    assert_ne!(old, new);
}

#[test]
fn mcp_key_differs_on_tool() {
    let args = json!({ "query": "rust" });
    let search = mcp_key("mcp:srv-1:cfg-abc", "mcp__srv-1__search", &args).unwrap();
    let fetch = mcp_key("mcp:srv-1:cfg-abc", "mcp__srv-1__fetch", &args).unwrap();
    assert_ne!(search, fetch);
}

#[test]
fn mcp_resource_with_extra_colon_is_empty_key() {
    assert!(mcp_key("mcp:srv-1:cfg-abc:extra", "mcp__srv-1__search", &json!({})).is_none());
    assert!(mcp_key("srv-1:cfg-abc", "mcp__srv-1__search", &json!({})).is_none());
    assert!(mcp_key("mcp::cfg-abc", "mcp__srv-1__search", &json!({})).is_none());
}

#[test]
fn mcp_tool_name_prefix_mismatch_is_empty_key() {
    assert!(mcp_key("mcp:srv-1:cfg-abc", "mcp__srv-2__search", &json!({})).is_none());
    assert!(mcp_key("mcp:srv-1:cfg-abc", "search", &json!({})).is_none());
}

/// 永不返回的审批替身：模拟「无人应答」。
///
/// `tokio::time::timeout` 先轮询内部 future、再查截止时间；若用 `with_delay(200ms)`
/// 配 20ms 超时，并行负载下任务被抢占过久会让两个计时器同时就绪，从而返回 `Decided`。
/// 永不就绪的 future 让超时判定与调度无关。
struct NeverApproval;

#[async_trait::async_trait]
impl ApprovalPrompter for NeverApproval {
    async fn request(&self, _request: ApprovalRequest) -> ApprovalDecision {
        std::future::pending().await
    }
}

#[tokio::test]
async fn approval_timeout_is_denied() {
    let decision =
        request_with_timeout(&NeverApproval, approval_request(Duration::from_millis(20))).await;

    match decision {
        ApprovalWait::TimedOut { timeout } => {
            assert_eq!(
                timeout,
                Duration::from_millis(20),
                "超时结局必须带上实际等待上限"
            );
        }
        other => panic!("timeout must deny, got {:?}", other),
    }
}

#[tokio::test]
async fn approval_within_timeout_is_approved() {
    let prompter = ScriptedApprovalPrompter::new(vec![ApprovalDecision::Approve]);
    let decision = request_with_timeout(&prompter, approval_request(Duration::from_secs(5))).await;

    assert_eq!(decision, ApprovalWait::Decided(ApprovalDecision::Approve));
    assert_eq!(prompter.request_count(), 1);
}

#[tokio::test]
async fn always_approve_only_writes_the_current_key() {
    let store = InMemoryGrantStore::default();
    let granted = path_key("fs_edit", "edit", &ws_path("a.txt")).unwrap();
    store
        .save_grant(&granted, GrantScope::Global, None)
        .await
        .unwrap();

    assert!(store.is_granted(&granted, None).await);

    let similar = path_key("fs_edit", "edit", &ws_path("a.txt.bak")).unwrap();
    assert!(!store.is_granted(&similar, None).await);

    let other_action = path_key("fs_read", "read", &ws_path("a.txt")).unwrap();
    assert!(!store.is_granted(&other_action, None).await);

    let normalized = path_key("fs_edit", "edit", &ws_path("./a.txt")).unwrap();
    assert_eq!(normalized, granted);
    assert!(store.is_granted(&normalized, None).await);
}

#[tokio::test]
async fn child_grant_store_reads_parent_but_writes_local() {
    let parent_concrete = Arc::new(InMemoryGrantStore::default());
    let parent: Arc<dyn PermissionGrantStore> = parent_concrete.clone();

    let parent_key = path_key("fs_read", "read", &ws_path("parent.txt")).unwrap();
    parent
        .save_grant(&parent_key, GrantScope::Global, None)
        .await
        .unwrap();

    let child = ChildGrantStore::new(parent.clone());
    assert!(child.is_granted(&parent_key, None).await);

    let child_key = path_key("fs_edit", "edit", &ws_path("child.txt")).unwrap();
    child
        .save_grant(&child_key, GrantScope::Global, None)
        .await
        .unwrap();

    assert!(child.is_granted(&child_key, None).await);
    assert!(!parent_concrete.is_granted(&child_key, None).await);
    assert_eq!(child.local().len(), 1);
    assert_eq!(parent_concrete.len(), 1);
}

#[tokio::test]
async fn in_memory_audit_store_records_entries_in_order() {
    let store = InMemoryAuditStore::default();
    let entry = AuditEntry {
        tool: "fs_edit".to_string(),
        action: "edit".to_string(),
        resource: "a.txt".to_string(),
        outcome: AuditOutcome::Denied,
        at_millis: 1700000000000,
        conversation_id: None,
        turn_id: None,
        iteration: None,
        source: None,
    };
    store.log_decision(entry.clone()).await.unwrap();

    assert_eq!(store.entries(), vec![entry]);
}

#[tokio::test]
async fn file_storage_persists_result_and_reports_bytes() {
    let dir = tempdir().unwrap();
    let storage = FileToolResultStorage::new(dir.path());

    let persisted = storage
        .persist(PersistToolResult {
            tool_call_id: "call/1:abc".to_string(),
            tool_name: "bash".to_string(),
            session_id: None,
            content: "hello world".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(persisted.bytes, 11);
    assert!(persisted.path.starts_with(dir.path().to_str().unwrap()));
    let name = Path::new(&persisted.path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        name.ends_with("-bash-call_1_abc.txt"),
        "unexpected name: {name}"
    );
    let written = std::fs::read_to_string(&persisted.path).unwrap();
    assert_eq!(written, "hello world");
}

#[tokio::test]
async fn scripted_prompter_falls_back_to_deny_when_exhausted() {
    let prompter = ScriptedApprovalPrompter::new(vec![ApprovalDecision::Approve]);

    assert_eq!(
        prompter
            .request(approval_request(Duration::from_secs(5)))
            .await,
        ApprovalDecision::Approve
    );

    let second = prompter
        .request(approval_request(Duration::from_secs(5)))
        .await;
    match second {
        ApprovalDecision::Deny { reason } => {
            assert_eq!(reason.as_deref(), Some("script exhausted"));
        }
        other => panic!("exhausted script must deny, got {:?}", other),
    }
    assert_eq!(prompter.request_count(), 2);
}

struct FakeTool {
    tier: ToolTier,
    intents: Vec<PermissionIntent>,
    intents_error: Option<String>,
    enforcement: Option<SandboxEnforcement>,
}

impl FakeTool {
    fn new(intents: Vec<PermissionIntent>) -> Self {
        Self {
            tier: ToolTier::Os,
            intents,
            intents_error: None,
            enforcement: None,
        }
    }

    fn with_enforcement(mut self, enforcement: SandboxEnforcement) -> Self {
        self.enforcement = Some(enforcement);
        self
    }
}

#[async_trait::async_trait]
impl Tool for FakeTool {
    fn id(&self) -> &str {
        "fake_tool"
    }

    async fn description(&self) -> Result<String, String> {
        Ok("fake tool for gateway tests".to_string())
    }

    fn input_schema(&self) -> Value {
        json!({ "type": "object" })
    }

    fn tier(&self) -> ToolTier {
        self.tier
    }

    fn sandbox_enforcement(&self) -> Option<SandboxEnforcement> {
        self.enforcement
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        match self.intents_error.as_ref() {
            Some(error) => Err(error.clone()),
            None => Ok(self.intents.clone()),
        }
    }

    async fn execute(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<ToolResult, String> {
        Ok(ToolResult::ok(Value::Null))
    }
}

fn gateway_with(
    prompter: Arc<dyn ApprovalPrompter>,
) -> (
    ToolGateway,
    Arc<InMemoryGrantStore>,
    Arc<InMemoryAuditStore>,
) {
    let grants = Arc::new(InMemoryGrantStore::default());
    let audit = Arc::new(InMemoryAuditStore::default());
    let gateway = ToolGateway::new(
        grants.clone(),
        audit.clone(),
        prompter,
        Duration::from_secs(5),
    );
    (gateway, grants, audit)
}

fn allow_all() -> ResolvedPermissionPolicy {
    ResolvedPermissionPolicy::new(
        vec![PermissionRule::new("*", "*", PermissionEffect::Allow)],
        Vec::new(),
    )
}

fn deny_all() -> ResolvedPermissionPolicy {
    ResolvedPermissionPolicy::new(
        vec![PermissionRule::new("*", "*", PermissionEffect::Deny)],
        Vec::new(),
    )
}

#[tokio::test]
async fn gateway_allows_without_asking_when_policy_allows() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![]));
    let (gateway, _, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![PermissionIntent::new("read", ws_path("a.txt"))]);

    let decision = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &allow_all())
        .await
        .unwrap();

    assert_eq!(decision, GatewayDecision::Allowed);
    assert_eq!(prompter.request_count(), 0, "Allow must not prompt");
    let entries = audit.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, AuditOutcome::Allowed);
}

#[tokio::test]
async fn gateway_denies_immediately_without_asking() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::Approve,
    ]));
    let (gateway, _, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![PermissionIntent::new("delete", ws_path("a.txt"))]);

    let decision = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &deny_all())
        .await
        .unwrap();

    assert_eq!(
        decision,
        GatewayDecision::Denied {
            action: "delete".to_string(),
            resource: ws_path("a.txt"),
        }
    );
    assert_eq!(prompter.request_count(), 0, "Deny must not prompt");
    assert_eq!(audit.entries()[0].outcome, AuditOutcome::Denied);
}

#[tokio::test]
async fn gateway_readonly_tool_without_intents_is_audited_as_bypassed() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![]));
    let (gateway, _, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![]);

    let decision = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &deny_all())
        .await
        .unwrap();

    assert_eq!(decision, GatewayDecision::Allowed);
    assert_eq!(prompter.request_count(), 0);
    let entries = audit.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, AuditOutcome::Bypassed);
    assert_eq!(entries[0].action, "none");
    assert_eq!(entries[0].resource, "intent:none");
    assert_eq!(
        entries[0].source,
        Some(ToolCallSource::Agent),
        "轮内派发要标明来源"
    );
}

/// I71 §4.4 W2：来源只做归因，不参与决策——同样的 deny_all 策略下
/// Agent 与 ToolBox 两个入口得到同一决策，仅审计行来源不同。
#[tokio::test]
async fn audit_source_is_attribution_only_and_never_changes_the_decision() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![]));
    let (gateway, _, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![]);

    let agent_decision = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &deny_all())
        .await
        .unwrap();
    let toolbox_context = ToolUseContext::for_toolbox(None);
    let toolbox_decision = gateway
        .evaluate(&tool, &Value::Null, &toolbox_context, &deny_all())
        .await
        .unwrap();

    assert_eq!(agent_decision, GatewayDecision::Allowed);
    assert_eq!(toolbox_decision, agent_decision);

    let entries = audit.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].source, Some(ToolCallSource::Agent));
    assert_eq!(entries[1].source, Some(ToolCallSource::Toolbox));
}

#[tokio::test]
async fn gateway_intent_error_is_audited_before_returning() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![]));
    let (gateway, _, audit) = gateway_with(prompter.clone());
    let mut tool = FakeTool::new(vec![]);
    tool.intents_error = Some("bad input".to_string());

    let error = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &allow_all())
        .await
        .unwrap_err();

    assert_eq!(error, "bad input");
    let entries = audit.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, AuditOutcome::IntentError);
    assert_eq!(entries[0].action, "none");
    assert_eq!(
        entries[0].resource, "intent:error",
        "错误文本不得写进 resource"
    );
    assert_eq!(prompter.request_count(), 0);
}

#[tokio::test]
async fn gateway_decision_branches_each_audit_exactly_once() {
    async fn run(
        prompter: Arc<dyn ApprovalPrompter>,
        tool: &FakeTool,
        policy: &ResolvedPermissionPolicy,
        timeout: Duration,
    ) -> (GatewayDecision, Arc<InMemoryAuditStore>) {
        let grants = Arc::new(InMemoryGrantStore::default());
        let audit = Arc::new(InMemoryAuditStore::default());
        let gateway = ToolGateway::new(grants, audit.clone(), prompter, timeout);
        let decision = gateway
            .evaluate(tool, &Value::Null, &ctx(), policy)
            .await
            .unwrap();
        (decision, audit)
    }

    // tier 上限拒绝：动作与资源固定为 `*` / `tier:<tier>`。
    let ceiling = allow_all().with_constraint_layer(PermissionConstraintLayer::new(vec![
        PermissionRule::new("*", "tier:os", PermissionEffect::Deny),
    ]));
    let (decision, audit) = run(
        Arc::new(ScriptedApprovalPrompter::new(vec![])),
        &FakeTool::new(vec![PermissionIntent::new("read", ws_path("a.txt"))]),
        &ceiling,
        Duration::from_secs(5),
    )
    .await;
    assert!(matches!(decision, GatewayDecision::Denied { .. }));
    assert_eq!(audit.entries().len(), 1);
    assert_eq!(audit.entries()[0].outcome, AuditOutcome::Denied);
    assert_eq!(audit.entries()[0].action, "*");
    assert_eq!(audit.entries()[0].resource, "tier:os");

    // 策略允许 / 策略拒绝。
    for (policy, expected) in [
        (allow_all(), AuditOutcome::Allowed),
        (deny_all(), AuditOutcome::Denied),
    ] {
        let (_, audit) = run(
            Arc::new(ScriptedApprovalPrompter::new(vec![])),
            &FakeTool::new(vec![PermissionIntent::new("read", ws_path("a.txt"))]),
            &policy,
            Duration::from_secs(5),
        )
        .await;
        let entries = audit.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].outcome, expected);
        assert!(!entries[0].action.is_empty());
        assert!(!entries[0].resource.is_empty());
    }

    // 缓存命中：放行原因与策略允许必须分开记。
    let (grants, audit, gateway) = {
        let grants = Arc::new(InMemoryGrantStore::default());
        let audit = Arc::new(InMemoryAuditStore::default());
        let gateway = ToolGateway::new(
            grants.clone(),
            audit.clone(),
            Arc::new(ScriptedApprovalPrompter::new(vec![])),
            Duration::from_secs(5),
        );
        (grants, audit, gateway)
    };
    let key = path_key("fake_tool", "read", &ws_path("a.txt")).unwrap();
    grants
        .save_grant(&key, GrantScope::Global, None)
        .await
        .unwrap();
    let tool = FakeTool::new(vec![PermissionIntent::new("read", ws_path("a.txt"))]);
    // 策略必须是 `Allow`：`Deny` 先于授权命中求值（§6.4），此处只想验证
    // 「命中缓存时审计原因与策略 `Allow` 分开记」。
    let decision = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &allow_all())
        .await
        .unwrap();
    assert_eq!(decision, GatewayDecision::Allowed);
    assert_eq!(audit.entries().len(), 1);
    assert_eq!(audit.entries()[0].outcome, AuditOutcome::GrantHit);

    // 审批四分支：Approve / AllowSession / AllowAlways / Deny。
    for (choice, expected) in [
        (ApprovalDecision::Approve, AuditOutcome::Approved),
        (
            ApprovalDecision::AllowSession,
            AuditOutcome::SessionApproved,
        ),
        (ApprovalDecision::AllowAlways, AuditOutcome::AlwaysApproved),
        (
            ApprovalDecision::Deny {
                reason: Some("no".to_string()),
            },
            AuditOutcome::Rejected,
        ),
    ] {
        let (_, audit) = run(
            Arc::new(ScriptedApprovalPrompter::new(vec![choice])),
            &FakeTool::new(vec![PermissionIntent::new("edit", ws_path("a.txt"))]),
            &ResolvedPermissionPolicy::default_global(),
            Duration::from_secs(5),
        )
        .await;
        let entries = audit.entries();
        assert_eq!(entries.len(), 1, "each decision audits exactly one entry");
        assert_eq!(entries[0].outcome, expected);
        assert!(!entries[0].action.is_empty());
        assert!(!entries[0].resource.is_empty());
    }

    // 超时：审计为 TimedOut，不是 Rejected。
    let (decision, audit) = run(
        Arc::new(
            ScriptedApprovalPrompter::new(vec![ApprovalDecision::Approve])
                .with_delay(Duration::from_millis(200)),
        ),
        &FakeTool::new(vec![PermissionIntent::new("edit", ws_path("a.txt"))]),
        &ResolvedPermissionPolicy::default_global(),
        Duration::from_millis(20),
    )
    .await;
    assert!(matches!(decision, GatewayDecision::ApprovalTimedOut { .. }));
    assert_eq!(audit.entries().len(), 1);
    assert_eq!(audit.entries()[0].outcome, AuditOutcome::TimedOut);

    // 空 intent 短路：`bypassed`，与 `allowed` 区分。
    let (decision, audit) = run(
        Arc::new(ScriptedApprovalPrompter::new(vec![])),
        &FakeTool::new(vec![]),
        &deny_all(),
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(decision, GatewayDecision::Allowed);
    assert_eq!(audit.entries().len(), 1);
    assert_eq!(audit.entries()[0].outcome, AuditOutcome::Bypassed);

    // intent 求值失败：先落 `intentError` 再返回错误。
    let mut broken = FakeTool::new(vec![]);
    broken.intents_error = Some("boom".to_string());
    let (grants, audit, gateway) = {
        let grants = Arc::new(InMemoryGrantStore::default());
        let audit = Arc::new(InMemoryAuditStore::default());
        let gateway = ToolGateway::new(
            grants.clone(),
            audit.clone(),
            Arc::new(ScriptedApprovalPrompter::new(vec![])),
            Duration::from_secs(5),
        );
        (grants, audit, gateway)
    };
    let _ = grants;
    assert!(gateway
        .evaluate(&broken, &Value::Null, &ctx(), &allow_all())
        .await
        .is_err());
    assert_eq!(audit.entries().len(), 1);
    assert_eq!(audit.entries()[0].outcome, AuditOutcome::IntentError);
}

#[tokio::test]
async fn gateway_merges_multiple_asks_into_one_request() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::Approve,
    ]));
    let (gateway, _, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![
        PermissionIntent::new("edit", ws_path("a.txt")),
        PermissionIntent::new("edit", ws_path("b.txt")),
    ]);

    let decision = gateway
        .evaluate(
            &tool,
            &Value::Null,
            &ctx(),
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();

    assert_eq!(decision, GatewayDecision::Allowed);
    assert_eq!(prompter.request_count(), 1, "asks must be merged");
    let requests = prompter.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].intents.len(), 1, "same action merged");
    assert_eq!(requests[0].intents[0].resources.len(), 2);
    assert_eq!(audit.entries().len(), 2);
    assert!(audit
        .entries()
        .iter()
        .all(|entry| entry.outcome == AuditOutcome::Approved));
}

#[tokio::test]
async fn gateway_always_approve_persists_key_and_skips_second_prompt() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::AllowAlways,
    ]));
    let (gateway, grants, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![PermissionIntent::new("edit", ws_path("a.txt"))]);
    let policy = ResolvedPermissionPolicy::default_global();

    let first = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &policy)
        .await
        .unwrap();
    assert_eq!(first, GatewayDecision::Allowed);
    assert_eq!(grants.len(), 1);

    let second = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &policy)
        .await
        .unwrap();
    assert_eq!(second, GatewayDecision::Allowed);
    assert_eq!(
        prompter.request_count(),
        1,
        "granted key must skip prompting"
    );

    let entries = audit.entries();
    assert_eq!(entries.len(), 2, "both decisions must be audited");
    assert_eq!(entries[0].outcome, AuditOutcome::AlwaysApproved);
    assert_eq!(
        entries[1].outcome,
        AuditOutcome::GrantHit,
        "cache hit must be audited as GrantHit, not silently skipped"
    );
}

#[tokio::test]
async fn gateway_always_approve_does_not_cover_a_different_resource() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::AllowAlways,
        ApprovalDecision::Deny { reason: None },
    ]));
    let (gateway, grants, _) = gateway_with(prompter.clone());
    let policy = ResolvedPermissionPolicy::default_global();

    gateway
        .evaluate(
            &FakeTool::new(vec![PermissionIntent::new("edit", ws_path("a.txt"))]),
            &Value::Null,
            &ctx(),
            &policy,
        )
        .await
        .unwrap();
    assert_eq!(grants.len(), 1);

    let second = gateway
        .evaluate(
            &FakeTool::new(vec![PermissionIntent::new("edit", ws_path("b.txt"))]),
            &Value::Null,
            &ctx(),
            &policy,
        )
        .await
        .unwrap();

    match second {
        GatewayDecision::RejectedByUser { .. } => {}
        other => panic!("similar resource must not reuse grant, got {:?}", other),
    }
    assert_eq!(prompter.request_count(), 2);
}

#[tokio::test]
async fn gateway_rejected_by_user_carries_reason() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::Deny {
            reason: Some("user cancelled".to_string()),
        },
    ]));
    let (gateway, grants, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![PermissionIntent::new("write", ws_path("a.txt"))]);

    let decision = gateway
        .evaluate(
            &tool,
            &Value::Null,
            &ctx(),
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();

    assert_eq!(
        decision,
        GatewayDecision::RejectedByUser {
            reason: Some("user cancelled".to_string()),
        }
    );
    assert!(grants.is_empty(), "rejection must not persist a grant");
    assert_eq!(audit.entries()[0].outcome, AuditOutcome::Rejected);
}

#[tokio::test]
async fn gateway_timeout_is_not_user_rejection() {
    let prompter = Arc::new(
        ScriptedApprovalPrompter::new(vec![ApprovalDecision::Approve])
            .with_delay(Duration::from_millis(300)),
    );
    let grants = Arc::new(InMemoryGrantStore::default());
    let audit = Arc::new(InMemoryAuditStore::default());
    let gateway = ToolGateway::new(
        grants.clone(),
        audit.clone(),
        prompter,
        Duration::from_millis(20),
    );
    let tool = FakeTool::new(vec![PermissionIntent::new("edit", ws_path("a.txt"))]);

    let decision = gateway
        .evaluate(
            &tool,
            &Value::Null,
            &ctx(),
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();

    match decision {
        GatewayDecision::ApprovalTimedOut { timeout } => {
            assert_eq!(timeout, Duration::from_millis(20));
        }
        other => panic!(
            "timeout must not be reported as user rejection, got {:?}",
            other
        ),
    }
    assert!(grants.is_empty(), "timeout must not persist a grant");
    assert_eq!(
        audit.entries()[0].outcome,
        AuditOutcome::TimedOut,
        "超时必须在审计里与用户拒绝分开"
    );
}

#[tokio::test]
async fn gateway_empty_key_still_asks_and_never_persists() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::AllowAlways,
    ]));
    let (gateway, grants, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![PermissionIntent::new("edit", "   ")]);

    let decision = gateway
        .evaluate(
            &tool,
            &Value::Null,
            &ctx(),
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();

    assert_eq!(decision, GatewayDecision::Allowed);
    assert_eq!(prompter.request_count(), 1, "empty key must still prompt");
    assert!(
        grants.is_empty(),
        "AllowAlways on an empty key must not persist anything"
    );
    let entries = audit.entries();
    assert_eq!(
        entries.len(),
        1,
        "empty key must be audited: it is the only proof a human was asked"
    );
    assert_eq!(entries[0].outcome, AuditOutcome::AlwaysApproved);
}

/// 审批卡片信息装配（I38d §3.2.1）：L0 零成本、L2 只在 `Ask` 路径付出 IO。
struct PreviewingTool {
    intents: Vec<PermissionIntent>,
    preview: Option<ApprovalPreview>,
    preview_calls: Arc<Mutex<usize>>,
    sandbox: Option<SandboxEnforcement>,
}

impl PreviewingTool {
    fn new(intents: Vec<PermissionIntent>, preview: Option<ApprovalPreview>) -> Self {
        Self {
            intents,
            preview,
            preview_calls: Arc::new(Mutex::new(0)),
            sandbox: None,
        }
    }

    /// 让替身扮演 `shell` 一类的沙箱工具（A14 需要 `sandbox_enforcement() = Some`）。
    fn with_sandbox(mut self, enforcement: SandboxEnforcement) -> Self {
        self.sandbox = Some(enforcement);
        self
    }

    fn preview_calls(&self) -> usize {
        self.preview_calls.lock().map(|c| *c).unwrap_or(0)
    }
}

#[async_trait::async_trait]
impl Tool for PreviewingTool {
    fn id(&self) -> &str {
        "previewing_tool"
    }

    async fn description(&self) -> Result<String, String> {
        Ok("tool with approval preview".to_string())
    }

    fn input_schema(&self) -> Value {
        json!({ "type": "object" })
    }

    fn tier(&self) -> ToolTier {
        ToolTier::Os
    }

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints {
            read_only: false,
            destructive: true,
            open_world: false,
        }
    }

    fn sandbox_enforcement(&self) -> Option<SandboxEnforcement> {
        self.sandbox
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        Ok(self.intents.clone())
    }

    async fn preview_for_approval(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Option<ApprovalPreview> {
        if let Ok(mut calls) = self.preview_calls.lock() {
            *calls += 1;
        }
        self.preview.clone()
    }

    async fn execute(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<ToolResult, String> {
        Ok(ToolResult::ok(Value::Null))
    }
}

/// 捕获审批请求的替身：断言卡片拿到了什么，而不是只看最终决策。
struct CapturingPrompter {
    seen: Arc<Mutex<Vec<ApprovalRequest>>>,
}

#[async_trait::async_trait]
impl ApprovalPrompter for CapturingPrompter {
    async fn request(&self, request: ApprovalRequest) -> ApprovalDecision {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request);
        }
        ApprovalDecision::Approve
    }
}

#[tokio::test]
async fn gateway_ask_path_carries_l0_l1_l2_context() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let prompter = Arc::new(CapturingPrompter { seen: seen.clone() });
    let (gateway, _, _) = gateway_with(prompter);
    let tool = PreviewingTool::new(
        vec![PermissionIntent::new("delete", ws_path("tmp/cache"))],
        Some(ApprovalPreview {
            summary: "删除 128 个条目（3.4 MB）".to_string(),
            truncated: true,
            estimated: false,
        }),
    );

    let decision = gateway
        .evaluate(
            &tool,
            &Value::Null,
            &ctx(),
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();

    assert_eq!(decision, GatewayDecision::Allowed);
    assert_eq!(tool.preview_calls(), 1, "Ask 路径必须预演一次");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let context = &seen[0].context;
    assert_eq!(context.tier, ToolTier::Os, "L0 层级来自工具元数据");
    assert!(context.behavior.destructive, "L0 行为提示必须透传");
    assert_eq!(
        context.reasons,
        vec![ApprovalReason::DefaultPolicy {
            rule: "* *".to_string()
        }],
        "L1 必须说明命中哪条策略"
    );
    let preview = context.preview.as_ref().expect("L2 预演必须透传");
    assert_eq!(preview.summary, "删除 128 个条目（3.4 MB）");
    assert!(preview.truncated);
    assert!(!preview.estimated);
    assert!(
        context.extensions.is_empty(),
        "未配置工作区时没有工作区结论，也不得伪造扩展项"
    );
}

/// A17：审批卡片的「工作区内/外」与执行期边界同源，结论必须一致。
#[tokio::test]
async fn workspace_extension_agrees_with_the_execution_boundary() {
    let root = tempdir().unwrap();
    let inside = root.path().join("inside.txt");
    std::fs::write(&inside, b"x").unwrap();
    let other = tempdir().unwrap();
    let outside = other.path().join("outside.txt");
    // `ensure_within_root` 的契约是「输入已归一」（macOS 上 `/var` → `/private/var`）。
    let inside = canonicalize_lenient(&inside).expect("canonical inside");
    let outside = canonicalize_lenient(&outside).expect("canonical outside");

    let seen = Arc::new(Mutex::new(Vec::new()));
    let prompter = Arc::new(CapturingPrompter { seen: seen.clone() });
    let (gateway, _, _) = gateway_with(prompter);
    // 工作区结论来自权限边界，只有 Agent 链路形态才有；ToolBox 形态不产生该结论。
    let context = ToolUseContext {
        workspace_root: Some(root.path().display().to_string()),
        ..Default::default()
    };

    for resource in [inside.display().to_string(), outside.display().to_string()] {
        let tool = PreviewingTool::new(vec![PermissionIntent::new("edit", resource)], None);
        gateway
            .evaluate(
                &tool,
                &Value::Null,
                &context,
                &ResolvedPermissionPolicy::default_global(),
            )
            .await
            .unwrap();
    }

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);

    let inside_extension = seen[0]
        .context
        .extensions
        .iter()
        .find(|extension| extension.kind == "workspace")
        .expect("inside target must carry a workspace conclusion");
    assert!(
        ensure_within_root(&inside, root.path()).is_ok(),
        "执行期边界必须放行区内目标"
    );
    assert_eq!(inside_extension.value, "inside the workspace root");
    assert_eq!(inside_extension.severity, ApprovalExtensionSeverity::Info);

    let outside_extension = seen[1]
        .context
        .extensions
        .iter()
        .find(|extension| extension.kind == "workspace")
        .expect("outside target must carry a workspace conclusion");
    assert!(
        ensure_within_root(&outside, root.path()).is_err(),
        "执行期边界必须拒绝区外目标"
    );
    assert_eq!(outside_extension.value, "outside the workspace root");
    assert_eq!(
        outside_extension.severity,
        ApprovalExtensionSeverity::Caution
    );
}

#[tokio::test]
async fn output_scope_extension_marks_the_session_output_namespace() {
    let output = tempdir().unwrap();
    std::fs::write(output.path().join("run.log"), b"log").unwrap();

    let seen = Arc::new(Mutex::new(Vec::new()));
    let prompter = Arc::new(CapturingPrompter { seen: seen.clone() });
    let (gateway, _, _) = gateway_with(prompter);

    // `readToolOutput` 恒 `Ask`（不受审批级别影响），因此一定走到卡片构造。
    let context = ToolUseContext {
        workspace_root: Some(output.path().display().to_string()),
        tool_output_root: Some(output.path().to_path_buf()),
        ..Default::default()
    };
    let tool = PreviewingTool::new(
        vec![PermissionIntent::new(
            "readToolOutput",
            output.path().display().to_string(),
        )],
        None,
    );
    let decision = gateway
        .evaluate(
            &tool,
            &Value::Null,
            &context,
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();
    assert_eq!(decision, GatewayDecision::Allowed, "用户批准后网关放行");

    let seen = seen.lock().unwrap();
    let extension = seen[0]
        .context
        .extensions
        .iter()
        .find(|extension| extension.kind == "output.scope")
        .expect("output namespace must carry a read-scope conclusion");
    assert_eq!(
        extension.value,
        "this session's tool output directory (read-only)"
    );
    assert_eq!(extension.severity, ApprovalExtensionSeverity::Info);
}

#[tokio::test]
async fn output_scope_extension_is_absent_without_an_output_root() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let prompter = Arc::new(CapturingPrompter { seen: seen.clone() });
    let (gateway, _, _) = gateway_with(prompter);
    let tool = PreviewingTool::new(
        vec![PermissionIntent::new(
            "readToolOutput",
            ws_path("tmp/run.log"),
        )],
        None,
    );
    gateway
        .evaluate(
            &tool,
            &Value::Null,
            &ctx(),
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();

    let seen = seen.lock().unwrap();
    assert!(
        seen[0]
            .context
            .extensions
            .iter()
            .all(|extension| extension.kind != "output.scope"),
        "no output root means no read-scope conclusion"
    );
}

/// A14：`shell` 一类的沙箱工具在 `Ask` 时卡片带两条扩展，且只写末段目录名。
///
/// 断言的是「卡片说了什么」，不是决策：用户据此判断这条命令会动到哪。
#[tokio::test]
async fn sandbox_extensions_describe_enforcement_and_writable_root() {
    let root = tempdir().unwrap();
    let canonical = canonicalize_lenient(root.path()).expect("canonical workspace");
    let base = canonical
        .file_name()
        .expect("tempdir has a last segment")
        .to_string_lossy()
        .into_owned();

    let seen = Arc::new(Mutex::new(Vec::new()));
    let prompter = Arc::new(CapturingPrompter { seen: seen.clone() });
    let (gateway, _, _) = gateway_with(prompter);
    let tool = PreviewingTool::new(
        vec![PermissionIntent::new("execute", "rm -rf build")],
        Some(ApprovalPreview {
            summary: "执行 1 条命令 · 破坏性 · 隔离：完全".to_string(),
            truncated: false,
            estimated: true,
        }),
    )
    .with_sandbox(SandboxEnforcement::Full);
    let context = ToolUseContext {
        workspace_root: Some(canonical.display().to_string()),
        ..Default::default()
    };

    gateway
        .evaluate(
            &tool,
            &Value::Null,
            &context,
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();

    let seen = seen.lock().unwrap();
    let extensions = &seen[0].context.extensions;
    let enforcement = extensions
        .iter()
        .find(|extension| extension.kind == "sandbox.enforcement")
        .expect("shell card must state the isolation strength");
    assert_eq!(enforcement.label, "隔离强度");
    assert_eq!(enforcement.value, "完全隔离");
    assert_eq!(enforcement.severity, ApprovalExtensionSeverity::Info);

    let writable = extensions
        .iter()
        .find(|extension| extension.kind == "sandbox.root")
        .expect("shell card must state the writable root");
    assert_eq!(writable.label, "可写根");
    assert_eq!(writable.value, base, "卡片只写末段目录名，不带绝对路径");
    assert!(
        !writable.value.contains(&canonical.display().to_string()),
        "absolute path must never reach the card"
    );
    assert_eq!(writable.severity, ApprovalExtensionSeverity::Info);

    let summary = seen[0]
        .context
        .preview
        .as_ref()
        .expect("L2 preview must be carried")
        .summary
        .clone();
    assert!(
        summary.contains("破坏性"),
        "类别由 summary 携带，不另立扩展"
    );
    assert_eq!(
        extensions
            .iter()
            .filter(|extension| extension.kind.starts_with("sandbox."))
            .count(),
        2,
        "exactly two sandbox extensions: no third entry for the command class"
    );
}

/// A14（另一半）：部分隔离降为 `Caution`，非沙箱工具一条都不产出。
#[tokio::test]
async fn sandbox_extensions_are_absent_for_non_sandbox_tools_and_caution_when_partial() {
    let root = tempdir().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let prompter = Arc::new(CapturingPrompter { seen: seen.clone() });
    let (gateway, _, _) = gateway_with(prompter);
    let context = ToolUseContext {
        workspace_root: Some(root.path().display().to_string()),
        ..Default::default()
    };

    let partial = PreviewingTool::new(vec![PermissionIntent::new("execute", "echo hi")], None)
        .with_sandbox(SandboxEnforcement::Partial);
    let plain = PreviewingTool::new(vec![PermissionIntent::new("edit", "a.txt")], None);

    for tool in [&partial as &dyn Tool, &plain as &dyn Tool] {
        gateway
            .evaluate(
                tool,
                &Value::Null,
                &context,
                &ResolvedPermissionPolicy::default_global(),
            )
            .await
            .unwrap();
    }

    let seen = seen.lock().unwrap();
    let partial_enforcement = seen[0]
        .context
        .extensions
        .iter()
        .find(|extension| extension.kind == "sandbox.enforcement")
        .expect("partial isolation must still be stated");
    assert_eq!(partial_enforcement.value, "部分隔离");
    assert_eq!(
        partial_enforcement.severity,
        ApprovalExtensionSeverity::Caution,
        "partial isolation is a warning, not a reassurance"
    );
    assert!(
        seen[1]
            .context
            .extensions
            .iter()
            .all(|extension| !extension.kind.starts_with("sandbox.")),
        "a tool without a sandbox must not claim a boundary"
    );
}

/// A16：`resource` 会整条落库，密钥必须在落库前打掉，而执行侧仍拿原值。
///
/// 两条断言缺一不可：只测打码会漏掉「把执行也改坏」，只测执行会漏掉「密钥进了明文库」。
#[tokio::test]
async fn audit_redacts_secrets_while_the_approved_command_stays_intact() {
    let command = "set TOKEN=abc123 && deploy --password hunter2 --region cn";
    let seen = Arc::new(Mutex::new(Vec::new()));
    let prompter = Arc::new(CapturingPrompter { seen: seen.clone() });
    let (gateway, _, audit) = gateway_with(prompter);
    let tool = PreviewingTool::new(vec![PermissionIntent::new("execute", command)], None);

    gateway
        .evaluate(
            &tool,
            &Value::Null,
            &ctx(),
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();

    let entries = audit.entries();
    let entry = entries
        .iter()
        .find(|entry| entry.action == "execute")
        .expect("execute decision must be audited");
    assert_eq!(
        entry.resource, "set TOKEN=<redacted> && deploy --password <redacted> --region cn",
        "secrets must be redacted before the row is written"
    );
    assert!(!entry.resource.contains("abc123"), "env secret leaked");
    assert!(!entry.resource.contains("hunter2"), "flag secret leaked");

    let seen = seen.lock().unwrap();
    assert_eq!(
        seen[0].intents[0].resources,
        vec![command.to_string()],
        "the approval card and the executed command keep the original value"
    );
}

#[test]
fn redact_secrets_covers_the_documented_shapes_only() {
    // `-p` 是端口，打码会把审计记录毁成不可读（§10.3 明确排除）。
    assert_eq!(
        redact_secrets("docker run -p 8080:80 img"),
        "docker run -p 8080:80 img"
    );
    assert_eq!(redact_secrets("ssh -p 2222 host"), "ssh -p 2222 host");
    // 内联赋值形式不能把下一个 token 一起吞掉。
    assert_eq!(
        redact_secrets("curl --token=abc https://x"),
        "curl --token=<redacted> https://x"
    );
    // 开关形式只打码后一个 token，其余参数保持可读。
    assert_eq!(
        redact_secrets("tool --api-key sk-1 --verbose"),
        "tool --api-key <redacted> --verbose"
    );
    // 词表外的名字不动，避免把普通参数打成一堆 `<redacted>`。
    assert_eq!(redact_secrets("set COLOR=red"), "set COLOR=red");
    // 空值不产生 `NAME=<redacted>`，否则会把「显式清空」记成「有密钥」。
    assert_eq!(redact_secrets("set TOKEN="), "set TOKEN=");
    // 大小写不敏感：`token` / `Password` 同样命中。
    assert_eq!(
        redact_secrets("set token=x db_password=y"),
        "set token=<redacted> db_password=<redacted>"
    );
}

#[tokio::test]
async fn read_tool_output_grant_covers_the_whole_session_namespace() {
    let workspace = tempdir().unwrap();
    let output = tempdir().unwrap();
    std::fs::write(output.path().join("a.log"), b"first").unwrap();
    std::fs::write(output.path().join("b.log"), b"second").unwrap();

    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::AllowSession,
    ]));
    let (gateway, grants, audit) = gateway_with(prompter.clone());
    let tool = AgentReadTool::new(Arc::new(InMemoryReadReceiptStore::new()));
    let policy = ResolvedPermissionPolicy::default_global();
    let context = ToolUseContext {
        workspace_root: Some(workspace.path().display().to_string()),
        session_id: Some("conv-1".to_string()),
        tool_output_root: Some(output.path().to_path_buf()),
        ..Default::default()
    };

    for name in ["a.log", "b.log"] {
        let decision = gateway
            .evaluate(
                &tool,
                &json!({ "path": output.path().join(name).display().to_string() }),
                &context,
                &policy,
            )
            .await
            .unwrap();
        assert_eq!(decision, GatewayDecision::Allowed, "用户批准后放行");
    }

    assert_eq!(
        prompter.request_count(),
        1,
        "resource 取命名空间目录：同域第二个文件不再弹框"
    );
    assert_eq!(grants.len(), 1, "一次授权只落一条键");
    assert_eq!(
        audit.entries()[1].outcome,
        AuditOutcome::GrantHit,
        "第二次读取必须记 GrantHit，而不是静默放行"
    );
}

#[tokio::test]
async fn read_tool_output_stays_ask_under_every_approval_level() {
    let output = tempdir().unwrap();
    let workspace = tempdir().unwrap();
    std::fs::write(output.path().join("run.log"), b"log").unwrap();
    let context = ToolUseContext {
        workspace_root: Some(workspace.path().display().to_string()),
        session_id: Some("conv-1".to_string()),
        tool_output_root: Some(output.path().to_path_buf()),
        ..Default::default()
    };
    let input = json!({ "path": output.path().join("run.log").display().to_string() });
    let tool = AgentReadTool::new(Arc::new(InMemoryReadReceiptStore::new()));

    // `dangerous_only` 与 `none` 两档都不得放宽输出域读取（I52 §5.11.4(4)）。
    for level in [ApprovalLevel::DangerousOnly, ApprovalLevel::None] {
        let policy = policy_from_agent_mode(ToolTier::External, level);
        let intents = tool.permission_intents(&input, &context).unwrap();
        let effect = policy.evaluate(&intents[0].action, &intents[0].resources[0]);
        assert_eq!(
            effect,
            PermissionEffect::Ask,
            "输出域读取在 {:?} 档必须仍为 Ask",
            level
        );
    }
}

#[tokio::test]
async fn read_tool_output_request_hides_the_absolute_namespace_path() {
    let workspace = tempdir().unwrap();
    let output = tempdir().unwrap();
    std::fs::write(output.path().join("run.log"), b"log").unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let prompter = Arc::new(CapturingPrompter { seen: seen.clone() });
    let (gateway, _, _) = gateway_with(prompter);
    let tool = AgentReadTool::new(Arc::new(InMemoryReadReceiptStore::new()));
    let context = ToolUseContext {
        workspace_root: Some(workspace.path().display().to_string()),
        session_id: Some("conv-1".to_string()),
        tool_output_root: Some(output.path().to_path_buf()),
        ..Default::default()
    };
    let input = json!({ "path": output.path().join("run.log").display().to_string() });
    gateway
        .evaluate(
            &tool,
            &input,
            &context,
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();

    let seen = seen.lock().unwrap();
    let intent = &seen[0].intents[0];
    assert_eq!(intent.action, "readToolOutput");
    let resource = &intent.resources[0];
    assert!(
        resource.starts_with('…'),
        "卡片资源名必须脱敏，实际: {resource}"
    );
    assert!(
        !resource.contains(&output.path().display().to_string()),
        "本机绝对路径不得进入审批卡: {resource}"
    );
}

/// C8：`readToolOutput` 的三态回归——拒绝后不放行、不落授权、审计记一次 `Denied`。
/// 与 Ask（恒问）/ GrantHit（同域第二次）两条既有用例合起来覆盖新增动作的三条分支。
#[tokio::test]
async fn read_tool_output_denied_by_the_user_is_not_granted() {
    let workspace = tempdir().unwrap();
    let output = tempdir().unwrap();
    std::fs::write(output.path().join("run.log"), b"log").unwrap();

    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::Deny {
            reason: Some("not this time".to_string()),
        },
    ]));
    let (gateway, grants, audit) = gateway_with(prompter);
    let tool = AgentReadTool::new(Arc::new(InMemoryReadReceiptStore::new()));
    let context = ToolUseContext {
        workspace_root: Some(workspace.path().display().to_string()),
        session_id: Some("conv-1".to_string()),
        tool_output_root: Some(output.path().to_path_buf()),
        ..Default::default()
    };
    let input = json!({ "path": output.path().join("run.log").display().to_string() });

    let decision = gateway
        .evaluate(
            &tool,
            &input,
            &context,
            &ResolvedPermissionPolicy::default_global(),
        )
        .await
        .unwrap();
    assert_eq!(
        decision,
        GatewayDecision::RejectedByUser {
            reason: Some("not this time".to_string()),
        }
    );
    assert!(
        grants.is_empty(),
        "a rejected read must not persist a grant"
    );
    let entries = audit.entries();
    assert_eq!(entries.len(), 1, "exactly one audit row per decision");
    assert_eq!(entries[0].action, "readToolOutput");
    assert_eq!(
        entries[0].outcome,
        AuditOutcome::Rejected,
        "user rejection is `Rejected`, distinct from policy `Denied`"
    );
}

#[tokio::test]
async fn gateway_allow_path_never_pays_the_preview_cost() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let prompter = Arc::new(CapturingPrompter { seen });
    let (gateway, _, _) = gateway_with(prompter);
    let tool = PreviewingTool::new(
        vec![PermissionIntent::new("delete", ws_path("tmp/cache"))],
        Some(ApprovalPreview {
            summary: "不该被调用".to_string(),
            truncated: false,
            estimated: false,
        }),
    );

    let decision = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &allow_all())
        .await
        .unwrap();

    assert_eq!(decision, GatewayDecision::Allowed);
    assert_eq!(tool.preview_calls(), 0, "Allow 路径不得为卡片付出 IO");
}

fn session_ctx() -> ToolUseContext {
    ToolUseContext {
        session_id: Some("conv-1".to_string()),
        ..Default::default()
    }
}

/// A7：`sandbox_tier` 对命令键是常量，隔离强度降级后旧授权不得复用。
#[tokio::test]
async fn command_grant_is_not_reused_after_enforcement_downgrade() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::AllowSession,
        ApprovalDecision::Deny { reason: None },
    ]));
    let (gateway, grants, _) = gateway_with(prompter.clone());
    let context = session_ctx();
    let policy = ResolvedPermissionPolicy::default_global();

    let full = FakeTool::new(vec![PermissionIntent::new("execute", "git status")])
        .with_enforcement(SandboxEnforcement::Full);
    let first = gateway
        .evaluate(&full, &Value::Null, &context, &policy)
        .await
        .unwrap();
    assert_eq!(first, GatewayDecision::Allowed);
    assert_eq!(grants.len(), 1, "session grant must persist");

    let partial = FakeTool::new(vec![PermissionIntent::new("execute", "git status")])
        .with_enforcement(SandboxEnforcement::Partial);
    let second = gateway
        .evaluate(&partial, &Value::Null, &context, &policy)
        .await
        .unwrap();
    match second {
        GatewayDecision::RejectedByUser { .. } => {}
        other => panic!("enforcement downgrade must re-ask, got {:?}", other),
    }
    assert_eq!(
        prompter.request_count(),
        2,
        "强边界下的授权不得在弱边界上复用"
    );
}

/// A8：策略 `Deny` 先于授权命中，历史授权不能覆盖策略改判。
#[tokio::test]
async fn policy_deny_wins_over_existing_grant() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![]));
    let (gateway, grants, audit) = gateway_with(prompter.clone());
    let context = session_ctx();

    let key = command_key_with_enforcement(
        "git status",
        &Value::Null,
        &context,
        ToolTier::Os,
        Some(SandboxEnforcement::Full),
    )
    .unwrap();
    grants
        .save_grant(&key, GrantScope::Session, Some("conv-1"))
        .await
        .unwrap();
    assert!(grants.is_granted(&key, Some("conv-1")).await);

    let tool = FakeTool::new(vec![PermissionIntent::new("execute", "git status")])
        .with_enforcement(SandboxEnforcement::Full);
    let decision = gateway
        .evaluate(&tool, &Value::Null, &context, &deny_all())
        .await
        .unwrap();

    match decision {
        GatewayDecision::Denied { .. } => {}
        other => panic!("policy Deny must win over a grant, got {:?}", other),
    }
    assert_eq!(prompter.request_count(), 0, "Deny must not prompt");
    assert_eq!(audit.entries()[0].outcome, AuditOutcome::Denied);
}

#[test]
fn command_key_carries_enforcement_and_decodes_legacy_rows() {
    let full = command_key_with_enforcement(
        "git status",
        &Value::Null,
        &ctx(),
        ToolTier::Os,
        Some(SandboxEnforcement::Full),
    )
    .unwrap();
    let partial = command_key_with_enforcement(
        "git status",
        &Value::Null,
        &ctx(),
        ToolTier::Os,
        Some(SandboxEnforcement::Partial),
    )
    .unwrap();
    let legacy =
        command_key_with_enforcement("git status", &Value::Null, &ctx(), ToolTier::Os, None)
            .unwrap();

    assert_ne!(full, partial, "enforcement must be part of the key");
    assert_ne!(full, legacy, "enforced key must not equal the legacy key");

    let json = serde_json::to_value(&full).unwrap();
    assert_eq!(json["scope"]["enforcement"], json!("full"));
    let decoded: ApprovalKey = serde_json::from_value(json).unwrap();
    assert_eq!(decoded, full, "round-trip must preserve enforcement");

    let legacy_json = serde_json::to_value(&legacy).unwrap();
    assert!(
        legacy_json["scope"]["enforcement"].is_null(),
        "键恒在、值可为 null"
    );
    let mut stripped = legacy_json;
    stripped["scope"]
        .as_object_mut()
        .unwrap()
        .remove("enforcement");
    let decoded: ApprovalKey = serde_json::from_value(stripped).unwrap();
    assert_eq!(decoded, legacy, "旧 grant_key 行必须解出 None");
}
