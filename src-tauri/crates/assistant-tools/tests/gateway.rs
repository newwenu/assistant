use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use assistant_tools::gateway::{
    request_with_timeout, ApprovalDecision, ApprovalKey, ApprovalPrompter, ApprovalRequest,
    ApprovalScope, AuditEntry, AuditOutcome, ChildGrantStore, InMemoryAuditStore,
    InMemoryGrantStore, PermissionAuditStore, PermissionGrantStore, ScriptedApprovalPrompter,
    ToolGateway,
};
use assistant_tools::storage::{FileToolResultStorage, PersistToolResult, ToolResultStorage};
use assistant_tools::{
    GatewayDecision, PermissionEffect, PermissionIntent, PermissionRule, ResolvedPermissionPolicy,
    Tool, ToolResult, ToolTier, ToolUseContext,
};
use serde_json::{json, Value};
use tempfile::tempdir;

fn ctx() -> ToolUseContext {
    ToolUseContext::default()
}

fn path_key(tool: &str, action: &str, resource: &str) -> Option<ApprovalKey> {
    ApprovalKey::from_intent(
        tool,
        &PermissionIntent::new(action, resource),
        resource,
        &Value::Null,
        &ctx(),
        ToolTier::Os,
    )
}

fn command_key(
    resource: &str,
    input: &Value,
    context: &ToolUseContext,
    tier: ToolTier,
) -> Option<ApprovalKey> {
    ApprovalKey::from_intent(
        "bash",
        &PermissionIntent::new("execute", resource),
        resource,
        input,
        context,
        tier,
    )
}

fn approval_request(timeout: Duration) -> ApprovalRequest {
    ApprovalRequest {
        tool_call_id: "call-1".to_string(),
        tool_name: "fs_edit".to_string(),
        arguments: json!({ "path": "a.txt" }),
        intents: vec![PermissionIntent::new("edit", "a.txt")],
        timeout,
    }
}

#[cfg(unix)]
fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
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

    if !try_symlink_file(&target, &link) {
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
    if !try_symlink_file(&target, &link) {
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
    if !try_symlink_file(&target, &link) {
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

    let real = path_key("fs_read", "read", "C:/tmp/a.txt").unwrap();
    store.save_grant(&real).await.unwrap();
    assert!(store.is_granted(&real).await);
    assert_eq!(store.len(), 1);
}

#[tokio::test]
async fn approval_timeout_is_denied() {
    let prompter = ScriptedApprovalPrompter::new(vec![ApprovalDecision::Approve])
        .with_delay(Duration::from_millis(200));

    let decision =
        request_with_timeout(&prompter, approval_request(Duration::from_millis(20))).await;

    match decision {
        ApprovalDecision::Deny { reason } => {
            assert_eq!(reason.as_deref(), Some("approval timed out"));
        }
        other => panic!("timeout must deny, got {:?}", other),
    }
}

#[tokio::test]
async fn approval_within_timeout_is_approved() {
    let prompter = ScriptedApprovalPrompter::new(vec![ApprovalDecision::Approve]);
    let decision = request_with_timeout(&prompter, approval_request(Duration::from_secs(5))).await;

    assert_eq!(decision, ApprovalDecision::Approve);
    assert_eq!(prompter.request_count(), 1);
}

#[tokio::test]
async fn always_approve_only_writes_the_current_key() {
    let store = InMemoryGrantStore::default();
    let granted = path_key("fs_edit", "edit", "C:/tmp/a.txt").unwrap();
    store.save_grant(&granted).await.unwrap();

    assert!(store.is_granted(&granted).await);

    let similar = path_key("fs_edit", "edit", "C:/tmp/a.txt.bak").unwrap();
    assert!(!store.is_granted(&similar).await);

    let other_action = path_key("fs_read", "read", "C:/tmp/a.txt").unwrap();
    assert!(!store.is_granted(&other_action).await);

    let normalized = path_key("fs_edit", "edit", "C:/tmp/./a.txt").unwrap();
    assert_eq!(normalized, granted);
    assert!(store.is_granted(&normalized).await);
}

#[tokio::test]
async fn child_grant_store_reads_parent_but_writes_local() {
    let parent_concrete = Arc::new(InMemoryGrantStore::default());
    let parent: Arc<dyn PermissionGrantStore> = parent_concrete.clone();

    let parent_key = path_key("fs_read", "read", "C:/tmp/parent.txt").unwrap();
    parent.save_grant(&parent_key).await.unwrap();

    let child = ChildGrantStore::new(parent.clone());
    assert!(child.is_granted(&parent_key).await);

    let child_key = path_key("fs_edit", "edit", "C:/tmp/child.txt").unwrap();
    child.save_grant(&child_key).await.unwrap();

    assert!(child.is_granted(&child_key).await);
    assert!(!parent_concrete.is_granted(&child_key).await);
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
    assert_eq!(name, "call_1_abc.txt");
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
    name: String,
    tier: ToolTier,
    intents: Vec<PermissionIntent>,
}

impl FakeTool {
    fn new(intents: Vec<PermissionIntent>) -> Self {
        Self {
            name: "fake_tool".to_string(),
            tier: ToolTier::Os,
            intents,
        }
    }
}

#[async_trait::async_trait]
impl Tool for FakeTool {
    fn name(&self) -> &str {
        &self.name
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

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        Ok(self.intents.clone())
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
    let tool = FakeTool::new(vec![PermissionIntent::new("read", "C:/ws/a.txt")]);

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
    let tool = FakeTool::new(vec![PermissionIntent::new("delete", "C:/ws/a.txt")]);

    let decision = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &deny_all())
        .await
        .unwrap();

    assert_eq!(
        decision,
        GatewayDecision::Denied {
            action: "delete".to_string(),
            resource: "C:/ws/a.txt".to_string(),
        }
    );
    assert_eq!(prompter.request_count(), 0, "Deny must not prompt");
    assert_eq!(audit.entries()[0].outcome, AuditOutcome::Denied);
}

#[tokio::test]
async fn gateway_readonly_tool_without_intents_is_allowed() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![]));
    let (gateway, _, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![]);

    let decision = gateway
        .evaluate(&tool, &Value::Null, &ctx(), &deny_all())
        .await
        .unwrap();

    assert_eq!(decision, GatewayDecision::Allowed);
    assert_eq!(prompter.request_count(), 0);
    assert!(audit.entries().is_empty());
}

#[tokio::test]
async fn gateway_merges_multiple_asks_into_one_request() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::Approve,
    ]));
    let (gateway, _, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![
        PermissionIntent::new("edit", "C:/ws/a.txt"),
        PermissionIntent::new("edit", "C:/ws/b.txt"),
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
        ApprovalDecision::AlwaysApprove,
    ]));
    let (gateway, grants, audit) = gateway_with(prompter.clone());
    let tool = FakeTool::new(vec![PermissionIntent::new("edit", "C:/ws/a.txt")]);
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
        ApprovalDecision::AlwaysApprove,
        ApprovalDecision::Deny { reason: None },
    ]));
    let (gateway, grants, _) = gateway_with(prompter.clone());
    let policy = ResolvedPermissionPolicy::default_global();

    gateway
        .evaluate(
            &FakeTool::new(vec![PermissionIntent::new("edit", "C:/ws/a.txt")]),
            &Value::Null,
            &ctx(),
            &policy,
        )
        .await
        .unwrap();
    assert_eq!(grants.len(), 1);

    let second = gateway
        .evaluate(
            &FakeTool::new(vec![PermissionIntent::new("edit", "C:/ws/b.txt")]),
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
    let tool = FakeTool::new(vec![PermissionIntent::new("write", "C:/ws/a.txt")]);

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
async fn gateway_timeout_becomes_rejected_by_user() {
    let prompter = Arc::new(
        ScriptedApprovalPrompter::new(vec![ApprovalDecision::Approve])
            .with_delay(Duration::from_millis(300)),
    );
    let grants = Arc::new(InMemoryGrantStore::default());
    let audit = Arc::new(InMemoryAuditStore::default());
    let gateway = ToolGateway::new(grants.clone(), audit, prompter, Duration::from_millis(20));
    let tool = FakeTool::new(vec![PermissionIntent::new("edit", "C:/ws/a.txt")]);

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
        GatewayDecision::RejectedByUser { reason } => {
            assert_eq!(reason.as_deref(), Some("approval timed out"));
        }
        other => panic!("timeout must not allow, got {:?}", other),
    }
    assert!(grants.is_empty(), "timeout must not persist a grant");
}

#[tokio::test]
async fn gateway_empty_key_still_asks_and_never_persists() {
    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::AlwaysApprove,
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
        "AlwaysApprove on an empty key must not persist anything"
    );
    let entries = audit.entries();
    assert_eq!(
        entries.len(),
        1,
        "empty key must be audited: it is the only proof a human was asked"
    );
    assert_eq!(entries[0].outcome, AuditOutcome::AlwaysApproved);
}
