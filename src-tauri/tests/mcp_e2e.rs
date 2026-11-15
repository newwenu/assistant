//! P7-C5 端到端：真实子进程 + 宿主链路。
//!
//! 夹具是 `tests/fixtures/dev_mcp_server.rs`（纯标准库 stdio server，由
//! `[[bin]]` 编译成真实进程）。测试通过 `CARGO_BIN_EXE_dev_mcp_server` 拿到
//! 绝对路径，因此不依赖解释器、不猜 `target` 布局，缺产物时直接编译失败而非静默跳过。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use assistant_app::adapters::resolve_available_tools;
use assistant_app::mcp::transport::StdioTransport;
use assistant_tools::mcp::{
    McpCallOutcome, McpCaller, McpServerConfig, McpServerManager, McpServerStatus, McpTransport,
};
use assistant_tools::pipeline::{InMemoryEventSink, ToolCall, ToolOutcome, ToolPipeline};
use assistant_tools::storage::FileToolResultStorage;
use assistant_tools::{
    ApprovalDecision, InMemoryAuditStore, InMemoryGrantStore, PermissionEffect, PermissionRule,
    ResolvedPermissionPolicy, ScriptedApprovalPrompter, ToolErrorKind, ToolGateway, ToolRegistry,
    ToolTier, ToolUseContext,
};
use serde_json::{json, Value};
use tempfile::TempDir;

const SERVER_ID: &str = "dev_mcp";

struct Fixture {
    server: &'static str,
    work: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let work = tempfile::tempdir().expect("e2e work dir must be creatable");
        Self {
            server: env!("CARGO_BIN_EXE_dev_mcp_server"),
            work,
        }
    }

    fn config(&self, config_hash: &str, call_timeout: Duration) -> McpServerConfig {
        McpServerConfig {
            id: SERVER_ID.to_string(),
            name: "dev-mcp".to_string(),
            command: self.server.to_string(),
            args: vec![
                "--work-dir".to_string(),
                self.work.path().display().to_string(),
            ],
            cwd: None,
            env: BTreeMap::new(),
            config_hash: config_hash.to_string(),
            call_timeout: Some(call_timeout),
        }
    }

    fn manager(&self) -> (Arc<ToolRegistry>, McpServerManager) {
        let registry = Arc::new(ToolRegistry::new());
        let transport: Arc<dyn McpTransport> = Arc::new(StdioTransport::new());
        let manager = McpServerManager::new(Arc::clone(&registry), transport);
        (registry, manager)
    }

    fn work_path(&self) -> PathBuf {
        self.work.path().to_path_buf()
    }

    fn work_file(&self, name: &str) -> PathBuf {
        self.work.path().join(name)
    }
}

fn allow_all() -> ResolvedPermissionPolicy {
    ResolvedPermissionPolicy::new(
        vec![PermissionRule::new("*", "*", PermissionEffect::Allow)],
        Vec::new(),
    )
}

fn ask_all() -> ResolvedPermissionPolicy {
    ResolvedPermissionPolicy::new(
        vec![PermissionRule::new("*", "*", PermissionEffect::Ask)],
        Vec::new(),
    )
}

fn pipeline(
    registry: &Arc<ToolRegistry>,
    prompter: Arc<ScriptedApprovalPrompter>,
    storage_root: PathBuf,
) -> (ToolPipeline, Arc<InMemoryAuditStore>) {
    let audit = Arc::new(InMemoryAuditStore::default());
    let gateway = Arc::new(ToolGateway::new(
        Arc::new(InMemoryGrantStore::default()),
        audit.clone(),
        prompter,
        Duration::from_secs(5),
    ));
    let storage: Arc<dyn assistant_tools::ToolResultStorage> =
        Arc::new(FileToolResultStorage::new(storage_root));
    (
        ToolPipeline::new(Arc::clone(registry), gateway, storage),
        audit,
    )
}

fn session_context() -> ToolUseContext {
    ToolUseContext {
        session_id: Some("conv-e2e".to_string()),
        turn_id: Some("turn-1".to_string()),
        iteration: Some(1),
        ..ToolUseContext::default()
    }
}

async fn run(
    pipeline: &ToolPipeline,
    call: ToolCall,
    context: &ToolUseContext,
    policy: &ResolvedPermissionPolicy,
) -> ToolOutcome {
    let sink = InMemoryEventSink::new();
    let mut outcomes = pipeline.execute(&[call], context, policy, &sink).await;
    assert_eq!(outcomes.len(), 1, "one call must yield exactly one outcome");
    outcomes.remove(0)
}

/// C5：握手成功、工具按规范化名注册，禁用后工具与进程一并回收。
#[tokio::test]
async fn handshake_registers_normalized_tools_and_disable_removes_them() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();

    let status = manager
        .enable(fixture.config("hash-1", Duration::from_secs(5)))
        .await;
    assert_eq!(status, McpServerStatus::Ready, "handshake must reach Ready");

    let snapshot = manager.snapshot(SERVER_ID).await.expect("snapshot exists");
    let mut ids = snapshot.tool_ids.clone();
    ids.sort();
    assert_eq!(
        ids,
        vec![
            "mcp__dev_mcp__dev_append",
            "mcp__dev_mcp__dev_bad_frame",
            "mcp__dev_mcp__dev_crash",
            "mcp__dev_mcp__dev_echo",
            "mcp__dev_mcp__dev_fail",
            "mcp__dev_mcp__dev_flood",
            "mcp__dev_mcp__dev_sleep",
        ]
    );
    for id in &ids {
        assert!(
            registry.find_by_name(id).is_some(),
            "tool {id} must be resolvable by model-facing name"
        );
    }

    manager.disable(SERVER_ID).await;
    assert_eq!(
        manager.snapshot(SERVER_ID).await.unwrap().status,
        McpServerStatus::Stopped
    );
    for id in &ids {
        assert!(
            registry.find_by_name(id).is_none(),
            "disable must unregister {id}"
        );
    }
}

/// C5：只读工具走完整 `find → validate → gateway → execute → persist` 通路。
#[tokio::test]
async fn a_read_only_tool_round_trips_through_the_pipeline() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();
    assert_eq!(
        manager
            .enable(fixture.config("hash-1", Duration::from_secs(5)))
            .await,
        McpServerStatus::Ready
    );

    let prompter = Arc::new(ScriptedApprovalPrompter::new(Vec::new()));
    let (pipeline, _audit) = pipeline(&registry, prompter, fixture.work_path());
    let outcome = run(
        &pipeline,
        ToolCall::new(
            "call-echo",
            "mcp__dev_mcp__dev_echo",
            json!({ "message": "ping", "n": 7 }),
        ),
        &session_context(),
        &allow_all(),
    )
    .await;

    assert!(outcome.result.ok, "echo must succeed: {:?}", outcome.result);
    let text = outcome
        .result
        .result_for_assistant
        .clone()
        .unwrap_or_default();
    assert!(
        text.contains("\"message\""),
        "server payload must be relayed: {text}"
    );
    manager.disable(SERVER_ID).await;
}

/// C5：`isError: true` 归入 `Business`，且结果可回填模型纠错。
#[tokio::test]
async fn a_business_failure_is_classified_as_business() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();
    assert_eq!(
        manager
            .enable(fixture.config("hash-1", Duration::from_secs(5)))
            .await,
        McpServerStatus::Ready
    );

    let prompter = Arc::new(ScriptedApprovalPrompter::new(Vec::new()));
    let (pipeline, _audit) = pipeline(&registry, prompter, fixture.work_path());
    let outcome = run(
        &pipeline,
        ToolCall::new(
            "call-fail",
            "mcp__dev_mcp__dev_fail",
            json!({ "reason": "boom" }),
        ),
        &session_context(),
        &allow_all(),
    )
    .await;

    assert!(!outcome.result.ok);
    assert_eq!(outcome.result.error_kind, Some(ToolErrorKind::Business));
    let error = outcome.result.error.clone().unwrap_or_default();
    assert!(
        error.contains("boom"),
        "error text must carry server reason: {error}"
    );
    manager.disable(SERVER_ID).await;
}

/// C5：慢调用触发本地超时，归入 `Timeout`，且 server 进程仍可用。
#[tokio::test]
async fn a_slow_call_times_out_without_killing_the_server() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();
    assert_eq!(
        manager
            .enable(fixture.config("hash-1", Duration::from_millis(200)))
            .await,
        McpServerStatus::Ready
    );

    let prompter = Arc::new(ScriptedApprovalPrompter::new(Vec::new()));
    let (pipeline, _audit) = pipeline(&registry, prompter, fixture.work_path());
    let outcome = run(
        &pipeline,
        ToolCall::new("call-slow", "mcp__dev_mcp__dev_sleep", json!({ "ms": 800 })),
        &session_context(),
        &allow_all(),
    )
    .await;
    assert_eq!(outcome.result.error_kind, Some(ToolErrorKind::Timeout));

    let after = run(
        &pipeline,
        ToolCall::new("call-after-timeout", "mcp__dev_mcp__dev_echo", json!({})),
        &session_context(),
        &allow_all(),
    )
    .await;
    assert!(
        after.result.ok,
        "timeout must not take the process down: {:?}",
        after.result
    );
    manager.disable(SERVER_ID).await;
}

/// C5：turn 取消只中止本次请求，进程与工具保留。
#[tokio::test]
async fn cancelling_a_call_keeps_the_process_alive() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();
    assert_eq!(
        manager
            .enable(fixture.config("hash-1", Duration::from_secs(5)))
            .await,
        McpServerStatus::Ready
    );

    let prompter = Arc::new(ScriptedApprovalPrompter::new(Vec::new()));
    let (pipeline, _audit) = pipeline(&registry, prompter, fixture.work_path());

    let token = tokio_util::sync::CancellationToken::new();
    let cancel_handle = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(120)).await;
        cancel_handle.cancel();
    });

    let mut context = session_context();
    context.cancel = Some(token);
    let outcome = run(
        &pipeline,
        ToolCall::new(
            "call-cancel",
            "mcp__dev_mcp__dev_sleep",
            json!({ "ms": 1500 }),
        ),
        &context,
        &allow_all(),
    )
    .await;
    assert_eq!(outcome.result.error_kind, Some(ToolErrorKind::Cancelled));

    let after = run(
        &pipeline,
        ToolCall::new("call-after-cancel", "mcp__dev_mcp__dev_echo", json!({})),
        &session_context(),
        &allow_all(),
    )
    .await;
    assert!(
        after.result.ok,
        "cancel must not take the process down: {:?}",
        after.result
    );
    manager.disable(SERVER_ID).await;
}

/// C5：子进程猝死不波及宿主，后续调用归入 `Internal`。
#[tokio::test]
async fn a_crashed_server_is_isolated_as_internal() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();
    assert_eq!(
        manager
            .enable(fixture.config("hash-1", Duration::from_secs(5)))
            .await,
        McpServerStatus::Ready
    );

    let prompter = Arc::new(ScriptedApprovalPrompter::new(Vec::new()));
    let (pipeline, _audit) = pipeline(&registry, prompter, fixture.work_path());
    let outcome = run(
        &pipeline,
        ToolCall::new("call-crash", "mcp__dev_mcp__dev_crash", json!({})),
        &session_context(),
        &allow_all(),
    )
    .await;
    assert_eq!(outcome.result.error_kind, Some(ToolErrorKind::Internal));

    let after = run(
        &pipeline,
        ToolCall::new("call-after-crash", "mcp__dev_mcp__dev_echo", json!({})),
        &session_context(),
        &allow_all(),
    )
    .await;
    assert_eq!(after.result.error_kind, Some(ToolErrorKind::Internal));
    assert!(
        after
            .result
            .error
            .unwrap_or_default()
            .contains("unavailable"),
        "dead connection must report unavailable"
    );

    manager.disable(SERVER_ID).await;
    assert_eq!(
        manager.snapshot(SERVER_ID).await.unwrap().status,
        McpServerStatus::Stopped
    );
}

/// C5：大结果不被 MCP 层截断，由 P2 预算落盘并回填引用。
#[tokio::test]
async fn a_large_result_is_persisted_by_the_round_budget() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();
    assert_eq!(
        manager
            .enable(fixture.config("hash-1", Duration::from_secs(5)))
            .await,
        McpServerStatus::Ready
    );

    let prompter = Arc::new(ScriptedApprovalPrompter::new(Vec::new()));
    let (pipeline, _audit) = pipeline(&registry, prompter, fixture.work_path());
    let bytes = 60_000;
    let outcome = run(
        &pipeline,
        ToolCall::new(
            "call-flood",
            "mcp__dev_mcp__dev_flood",
            json!({ "bytes": bytes }),
        ),
        &session_context(),
        &allow_all(),
    )
    .await;

    assert!(outcome.result.ok);
    let persisted = outcome
        .persisted
        .clone()
        .expect("large result must be persisted");
    let stored = std::fs::read_to_string(&persisted.path).expect("persisted file is readable");
    assert_eq!(
        stored.chars().count(),
        bytes,
        "MCP layer must not truncate: persisted payload keeps full length"
    );
    assert!(
        outcome.assistant_text.contains(&persisted.path),
        "assistant text must reference the persisted file"
    );
    manager.disable(SERVER_ID).await;
}

/// C5：非法帧只丢自己，连接继续工作。
#[tokio::test]
async fn a_malformed_frame_does_not_break_the_connection() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();
    assert_eq!(
        manager
            .enable(fixture.config("hash-1", Duration::from_secs(5)))
            .await,
        McpServerStatus::Ready
    );

    let prompter = Arc::new(ScriptedApprovalPrompter::new(Vec::new()));
    let (pipeline, _audit) = pipeline(&registry, prompter, fixture.work_path());
    let outcome = run(
        &pipeline,
        ToolCall::new("call-bad-frame", "mcp__dev_mcp__dev_bad_frame", json!({})),
        &session_context(),
        &allow_all(),
    )
    .await;
    assert!(
        outcome.result.ok,
        "the answering frame must still be delivered"
    );

    let after = run(
        &pipeline,
        ToolCall::new("call-after-bad-frame", "mcp__dev_mcp__dev_echo", json!({})),
        &session_context(),
        &allow_all(),
    )
    .await;
    assert!(after.result.ok, "connection must survive a malformed frame");
    manager.disable(SERVER_ID).await;
}

/// C5：`AllowSession` 命中同键不再询问；参数或 `config_hash` 变化后重新询问。
#[tokio::test]
async fn session_grants_are_bound_to_args_and_config_hash() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();
    assert_eq!(
        manager
            .enable(fixture.config("hash-1", Duration::from_secs(5)))
            .await,
        McpServerStatus::Ready
    );

    let prompter = Arc::new(ScriptedApprovalPrompter::new(vec![
        ApprovalDecision::AllowSession,
        ApprovalDecision::AllowSession,
        ApprovalDecision::AllowSession,
    ]));
    let (pipeline, _audit) = pipeline(&registry, prompter.clone(), fixture.work_path());

    let call = |id: &str, line: &str| {
        ToolCall::new(
            id,
            "mcp__dev_mcp__dev_append",
            json!({ "file": "grants.txt", "line": line }),
        )
    };

    let first = run(
        &pipeline,
        call("call-1", "one"),
        &session_context(),
        &ask_all(),
    )
    .await;
    assert!(first.result.ok, "{:?}", first.result);
    assert_eq!(prompter.requests().len(), 1, "first call must ask once");

    let second = run(
        &pipeline,
        call("call-2", "one"),
        &session_context(),
        &ask_all(),
    )
    .await;
    assert!(second.result.ok);
    assert_eq!(
        prompter.requests().len(),
        1,
        "identical args must reuse the session grant"
    );

    let third = run(
        &pipeline,
        call("call-3", "two"),
        &session_context(),
        &ask_all(),
    )
    .await;
    assert!(third.result.ok);
    assert_eq!(
        prompter.requests().len(),
        2,
        "changed arguments must invalidate the grant"
    );

    let file = fixture.work_file("grants.txt");
    let content = std::fs::read_to_string(&file).expect("append file exists");
    assert_eq!(
        content.lines().count(),
        3,
        "all three calls must reach the server"
    );

    assert_eq!(
        manager
            .enable(fixture.config("hash-2", Duration::from_secs(5)))
            .await,
        McpServerStatus::Ready
    );
    let fourth = run(
        &pipeline,
        call("call-4", "two"),
        &session_context(),
        &ask_all(),
    )
    .await;
    assert!(fourth.result.ok);
    assert_eq!(
        prompter.requests().len(),
        3,
        "config_hash change must invalidate the grant"
    );

    manager.disable(SERVER_ID).await;
}

/// C5：tier 上限、白名单与受众三重门同时生效。
#[tokio::test]
async fn external_tools_are_gated_by_tier_whitelist_and_audience() {
    let fixture = Fixture::new();
    let (registry, manager) = fixture.manager();
    assert_eq!(
        manager
            .enable(fixture.config("hash-1", Duration::from_secs(5)))
            .await,
        McpServerStatus::Ready
    );

    let ids = manager.snapshot(SERVER_ID).await.unwrap().tool_ids;
    let available = resolve_available_tools(&registry, &ids, &[], ToolTier::External).await;
    assert_eq!(
        available.len(),
        7,
        "max_tier=external must expose all MCP tools"
    );

    let below_ceiling = resolve_available_tools(&registry, &ids, &[], ToolTier::Os).await;
    assert!(
        below_ceiling.is_empty(),
        "external tools must not leak below the tier ceiling"
    );

    let deny_by_default = resolve_available_tools(&registry, &[], &[], ToolTier::External).await;
    assert!(
        deny_by_default.is_empty(),
        "empty whitelist exposes nothing"
    );

    for id in &ids {
        assert!(
            registry.find_for_toolbox(id).is_none(),
            "AGENT_ONLY tools must be unreachable from ToolBox: {id}"
        );
    }

    manager.disable(SERVER_ID).await;
}

/// C5：未知工具走 server 的 `-32601` 通道，归入 `ToolNotFound` → `Business`。
#[tokio::test]
async fn an_unknown_tool_on_the_server_is_a_business_failure() {
    let fixture = Fixture::new();
    let transport: Arc<dyn McpTransport> = Arc::new(StdioTransport::new());
    let connection = transport
        .connect(&fixture.config("hash-1", Duration::from_secs(5)))
        .await
        .expect("spawn must succeed");
    let defs = connection
        .handshake()
        .await
        .expect("handshake must succeed");
    assert_eq!(defs.len(), 7);

    let caller: Arc<dyn McpCaller> = connection.caller();
    let outcome = caller.call_tool("no_such_tool", &Value::Null, None).await;
    assert_eq!(outcome, McpCallOutcome::ToolNotFound);

    connection.shutdown().await;
}
