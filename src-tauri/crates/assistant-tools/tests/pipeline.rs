use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use assistant_tools::gateway::{
    InMemoryAuditStore, InMemoryGrantStore, ScriptedApprovalPrompter, ToolGateway,
};
use assistant_tools::pipeline::{
    InMemoryEventSink, PipelineEvent, PipelineStage, ToolCall, ToolPipeline,
};
use assistant_tools::storage::{
    PersistToolResult, PersistedRef, ToolResultStorage, PER_ROUND_LIMIT_CHARS,
    PER_TOOL_LIMIT_CHARS, PREVIEW_CHARS,
};
use assistant_tools::{
    ApprovalDecision, AuditOutcome, PermissionEffect, PermissionIntent, PermissionRule,
    ResolvedPermissionPolicy, Tool, ToolErrorKind, ToolRegistry, ToolResult, ToolTier,
    ToolUseContext, ValidationResult,
};
use async_trait::async_trait;
use serde_json::{json, Value};

fn ctx() -> ToolUseContext {
    ToolUseContext::default()
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

fn deny_all() -> ResolvedPermissionPolicy {
    ResolvedPermissionPolicy::new(
        vec![PermissionRule::new("*", "*", PermissionEffect::Deny)],
        Vec::new(),
    )
}

#[derive(Clone)]
struct ProbeSpec {
    name: &'static str,
    intents: Vec<PermissionIntent>,
    validate: ValidationResult,
    concurrent: bool,
    delay: Duration,
    result: ToolResult,
}

impl ProbeSpec {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            intents: Vec::new(),
            validate: ValidationResult::ok(),
            concurrent: false,
            delay: Duration::ZERO,
            result: ToolResult::ok(json!({ "ok": true })),
        }
    }

    fn concurrent(mut self) -> Self {
        self.concurrent = true;
        self
    }

    fn with_intents(mut self, intents: Vec<PermissionIntent>) -> Self {
        self.intents = intents;
        self
    }

    fn with_validation(mut self, validate: ValidationResult) -> Self {
        self.validate = validate;
        self
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    fn with_result(mut self, result: ToolResult) -> Self {
        self.result = result;
        self
    }
}

struct ProbeTool {
    spec: ProbeSpec,
    executed: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for ProbeTool {
    fn id(&self) -> &str {
        self.spec.name
    }

    async fn description(&self) -> Result<String, String> {
        Ok("probe tool".to_string())
    }

    fn input_schema(&self) -> Value {
        json!({ "type": "object" })
    }

    fn tier(&self) -> ToolTier {
        ToolTier::Os
    }

    fn validate_input(&self, _input: &Value, _context: &ToolUseContext) -> ValidationResult {
        self.spec.validate.clone()
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        self.spec.concurrent
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        Ok(self.spec.intents.clone())
    }

    async fn execute(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<ToolResult, String> {
        self.executed.fetch_add(1, Ordering::SeqCst);
        if !self.spec.delay.is_zero() {
            tokio::time::sleep(self.spec.delay).await;
        }
        Ok(self.spec.result.clone())
    }
}

#[derive(Debug, Default)]
struct RecordingStorage {
    persisted: Mutex<Vec<PersistToolResult>>,
}

impl RecordingStorage {
    fn calls(&self) -> Vec<PersistToolResult> {
        self.persisted.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl ToolResultStorage for RecordingStorage {
    async fn persist(&self, req: PersistToolResult) -> Result<PersistedRef, String> {
        let bytes = req.content.len();
        let path = format!("store/{}.txt", req.tool_call_id);
        self.persisted.lock().unwrap().push(req);
        Ok(PersistedRef { path, bytes })
    }
}

struct Harness {
    pipeline: ToolPipeline,
    registry: Arc<ToolRegistry>,
    audit: Arc<InMemoryAuditStore>,
    prompter: Arc<ScriptedApprovalPrompter>,
    storage: Arc<RecordingStorage>,
    executed: Arc<AtomicUsize>,
}

fn harness(decisions: Vec<ApprovalDecision>) -> Harness {
    harness_with_timeout(decisions, Duration::from_secs(5))
}

fn harness_with_timeout(decisions: Vec<ApprovalDecision>, timeout: Duration) -> Harness {
    let registry = Arc::new(ToolRegistry::new());
    let grants = Arc::new(InMemoryGrantStore::default());
    let audit = Arc::new(InMemoryAuditStore::default());
    let prompter = Arc::new(ScriptedApprovalPrompter::new(decisions));
    let gateway = Arc::new(ToolGateway::new(
        grants,
        audit.clone(),
        prompter.clone(),
        timeout,
    ));
    let storage = Arc::new(RecordingStorage::default());
    let pipeline = ToolPipeline::new(registry.clone(), gateway, storage.clone());
    Harness {
        pipeline,
        registry,
        audit,
        prompter,
        storage,
        executed: Arc::new(AtomicUsize::new(0)),
    }
}

impl Harness {
    fn add(&self, spec: ProbeSpec) {
        self.registry.register(Arc::new(ProbeTool {
            spec,
            executed: self.executed.clone(),
        }));
    }
}

fn call(id: &str, name: &str) -> ToolCall {
    ToolCall::new(id, name, json!({}))
}

/// 取出 `Finished` 事件里下发给 wire 的正文（与 tool 行 `content` 同源）。
fn finished_text(sink: &InMemoryEventSink) -> String {
    match &sink.events()[1] {
        PipelineEvent::Finished { assistant_text, .. } => assistant_text.clone(),
        other => panic!("expected Finished event, got {other:?}"),
    }
}

#[tokio::test]
async fn unknown_tool_fails_at_find_stage() {
    let h = harness(vec![]);
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(&[call("c1", "nope")], &ctx(), &allow_all(), &sink)
        .await;

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].stage, PipelineStage::Find);
    assert!(!outcomes[0].result.ok);
    assert_eq!(outcomes[0].result.error_kind, Some(ToolErrorKind::Internal));
    assert!(outcomes[0]
        .result
        .error
        .as_deref()
        .unwrap()
        .contains("unknown tool"));
    assert_eq!(h.executed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn invalid_input_returns_business_and_skips_gateway() {
    let h = harness(vec![ApprovalDecision::Approve]);
    h.add(
        ProbeSpec::new("probe")
            .with_intents(vec![PermissionIntent::new("edit", "a.txt")])
            .with_validation(ValidationResult::invalid("bad path")),
    );
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(&[call("c1", "probe")], &ctx(), &ask_all(), &sink)
        .await;

    assert_eq!(outcomes[0].stage, PipelineStage::Validate);
    assert!(!outcomes[0].blocks_input_rewrite);
    assert_eq!(outcomes[0].result.error_kind, Some(ToolErrorKind::Business));
    assert_eq!(
        h.prompter.request_count(),
        0,
        "validation must run before gateway: no approval for invalid input"
    );
    assert!(
        h.audit.entries().len() == 1,
        "validation failure must be audited as ValidateRejected"
    );
    assert_eq!(h.audit.entries()[0].outcome, AuditOutcome::ValidateRejected);
    assert_eq!(h.audit.entries()[0].action, "none");
    assert_eq!(h.audit.entries()[0].resource, "validate:rejected");
    assert_eq!(h.executed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn blocked_validation_never_reaches_gateway() {
    let h = harness(vec![ApprovalDecision::Approve]);
    h.add(
        ProbeSpec::new("probe")
            .with_intents(vec![PermissionIntent::new("edit", "a.txt")])
            .with_validation(ValidationResult::blocked("not allowed").with_error_code("E_BLOCK")),
    );
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(&[call("c1", "probe")], &ctx(), &ask_all(), &sink)
        .await;

    assert_eq!(outcomes[0].stage, PipelineStage::Validate);
    assert!(outcomes[0].blocks_input_rewrite);
    assert_eq!(outcomes[0].result.error_kind, Some(ToolErrorKind::Business));
    assert!(outcomes[0]
        .result
        .error
        .as_deref()
        .unwrap()
        .contains("E_BLOCK"));
    assert_eq!(h.prompter.request_count(), 0);
    assert_eq!(h.audit.entries().len(), 1);
    assert_eq!(h.audit.entries()[0].outcome, AuditOutcome::ValidateRejected);
}

#[tokio::test]
async fn gateway_denial_stops_before_execution() {
    let h = harness(vec![]);
    h.add(ProbeSpec::new("probe").with_intents(vec![PermissionIntent::new("edit", "a.txt")]));
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(&[call("c1", "probe")], &ctx(), &deny_all(), &sink)
        .await;

    assert_eq!(outcomes[0].stage, PipelineStage::Gateway);
    assert_eq!(outcomes[0].result.error_kind, Some(ToolErrorKind::Denied));
    assert_eq!(
        h.executed.load(Ordering::SeqCst),
        0,
        "denied call must not execute"
    );
    assert_eq!(h.prompter.request_count(), 0);
}

#[tokio::test]
async fn approval_timeout_is_tagged_timeout_not_denied() {
    let registry = Arc::new(ToolRegistry::new());
    let grants = Arc::new(InMemoryGrantStore::default());
    let audit = Arc::new(InMemoryAuditStore::default());
    let prompter = Arc::new(
        ScriptedApprovalPrompter::new(vec![ApprovalDecision::Approve])
            .with_delay(Duration::from_millis(200)),
    );
    let gateway = Arc::new(ToolGateway::new(
        grants,
        audit.clone(),
        prompter.clone(),
        Duration::from_millis(20),
    ));
    let pipeline = ToolPipeline::new(
        registry.clone(),
        gateway,
        Arc::new(RecordingStorage::default()),
    );
    let executed = Arc::new(AtomicUsize::new(0));
    registry.register(Arc::new(ProbeTool {
        spec: ProbeSpec::new("probe").with_intents(vec![PermissionIntent::new("edit", "a.txt")]),
        executed: executed.clone(),
    }));
    let sink = InMemoryEventSink::new();

    let outcomes = pipeline
        .execute(&[call("c1", "probe")], &ctx(), &ask_all(), &sink)
        .await;

    assert_eq!(outcomes[0].stage, PipelineStage::Gateway);
    assert_eq!(
        outcomes[0].result.error_kind,
        Some(ToolErrorKind::Timeout),
        "超时必须打 Timeout 标，而不是 Denied"
    );
    let error = outcomes[0].result.error.as_deref().unwrap();
    assert!(
        error.contains("did not respond"),
        "超时文案必须说明「无人应答」：{error}"
    );
    assert!(
        !error.contains("rejected by user"),
        "超时不得伪装成用户拒绝：{error}"
    );
    assert_eq!(executed.load(Ordering::SeqCst), 0, "超时调用不得执行");
    assert_eq!(
        audit.entries()[0].outcome,
        AuditOutcome::TimedOut,
        "超时审计必须与用户拒绝分开"
    );
}

#[tokio::test]
async fn rejected_by_user_is_tagged_denied() {
    let h = harness(vec![ApprovalDecision::Deny {
        reason: Some("nope".to_string()),
    }]);
    h.add(ProbeSpec::new("probe").with_intents(vec![PermissionIntent::new("edit", "a.txt")]));
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(&[call("c1", "probe")], &ctx(), &ask_all(), &sink)
        .await;

    assert_eq!(outcomes[0].stage, PipelineStage::Gateway);
    assert_eq!(outcomes[0].result.error_kind, Some(ToolErrorKind::Denied));
    assert!(outcomes[0]
        .result
        .error
        .as_deref()
        .unwrap()
        .contains("nope"));
    assert_eq!(h.executed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn approved_call_runs_and_reports_events_in_order() {
    let h = harness(vec![ApprovalDecision::Approve]);
    h.add(ProbeSpec::new("probe").with_intents(vec![PermissionIntent::new("edit", "a.txt")]));
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(&[call("c1", "probe")], &ctx(), &ask_all(), &sink)
        .await;

    assert_eq!(outcomes[0].stage, PipelineStage::Execute);
    assert!(outcomes[0].result.ok);
    assert_eq!(h.executed.load(Ordering::SeqCst), 1);
    assert!(outcomes[0].persisted.is_none(), "small result stays inline");

    let events = sink.events();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], PipelineEvent::Started { .. }));
    assert!(matches!(events[1], PipelineEvent::Finished { .. }));
}

#[tokio::test]
async fn failed_call_reports_failed_event() {
    let h = harness(vec![]);
    h.add(ProbeSpec::new("probe").with_result(ToolResult::err("boom")));
    let sink = InMemoryEventSink::new();

    h.pipeline
        .execute(&[call("c1", "probe")], &ctx(), &allow_all(), &sink)
        .await;

    let events = sink.events();
    assert_eq!(events.len(), 2);
    match &events[1] {
        PipelineEvent::Failed {
            error, error_kind, ..
        } => {
            assert_eq!(error, "boom");
            assert_eq!(*error_kind, Some(ToolErrorKind::Business));
        }
        other => panic!("expected Failed event, got {:?}", other),
    }
}

#[tokio::test]
async fn large_result_is_persisted_with_preview() {
    let big = "x".repeat(PER_TOOL_LIMIT_CHARS + 1);
    let h = harness(vec![]);
    h.add(
        ProbeSpec::new("probe").with_result(ToolResult::ok_with_assistant(
            json!({ "big": true }),
            big.clone(),
        )),
    );
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(&[call("c1", "probe")], &ctx(), &allow_all(), &sink)
        .await;

    let persisted = outcomes[0]
        .persisted
        .as_ref()
        .expect("big result must persist");
    assert_eq!(h.storage.calls().len(), 1);
    assert_eq!(h.storage.calls()[0].content, big);
    let prefix = format!("<persisted-output path=\"{}\">\n", persisted.path);
    assert!(outcomes[0].assistant_text.starts_with(&prefix));
    let marker = format!(
        "\n[… {} chars omitted; full output in the file above …]\n",
        big.chars().count() - PREVIEW_CHARS
    );
    assert_eq!(
        outcomes[0].assistant_text.chars().count(),
        prefix.chars().count() + PREVIEW_CHARS + marker.chars().count(),
        "persisted output keeps head + tail preview plus one omission marker"
    );
    let reported = finished_text(&sink);
    assert_eq!(
        reported, outcomes[0].assistant_text,
        "门禁 1：Finished.result 与落库 tool 行正文同源"
    );
}

/// 门禁 1（写入侧）：`Finished.result` 与落库 tool 行正文逐字节相等。
#[tokio::test]
async fn finished_result_matches_tool_row_text() {
    let text = "line one\nline two".to_string();
    let h = harness(vec![]);
    h.add(
        ProbeSpec::new("probe").with_result(ToolResult::ok_with_assistant(
            json!({ "n": 1 }),
            text.clone(),
        )),
    );
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(&[call("c1", "probe")], &ctx(), &allow_all(), &sink)
        .await;

    assert_eq!(finished_text(&sink), text);
    assert_eq!(finished_text(&sink), outcomes[0].assistant_text);
    assert!(h.storage.calls().is_empty(), "small result stays inline");
}

#[tokio::test]
async fn assistant_text_falls_back_to_compact_data() {
    let h = harness(vec![]);
    h.add(ProbeSpec::new("probe").with_result(ToolResult::ok(json!({ "a": 1 }))));
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(&[call("c1", "probe")], &ctx(), &allow_all(), &sink)
        .await;

    assert_eq!(outcomes[0].assistant_text, r#"{"a":1}"#);
}

#[tokio::test]
async fn partition_matches_spec_example() {
    let h = harness(vec![]);
    h.add(ProbeSpec::new("read").concurrent());
    h.add(ProbeSpec::new("grep").concurrent());
    h.add(ProbeSpec::new("write"));
    h.add(ProbeSpec::new("bash"));

    let calls = vec![
        call("c1", "read"),
        call("c2", "read"),
        call("c3", "write"),
        call("c4", "grep"),
        call("c5", "grep"),
        call("c6", "bash"),
    ];

    let batches = h.pipeline.partition_tool_batches(&calls);
    assert_eq!(batches, vec![vec![0, 1], vec![2], vec![3, 4], vec![5]]);
}

#[tokio::test]
async fn partition_treats_unknown_tool_as_serial() {
    let h = harness(vec![]);
    h.add(ProbeSpec::new("read").concurrent());

    let calls = vec![
        call("c1", "read"),
        call("c2", "missing"),
        call("c3", "read"),
    ];
    let batches = h.pipeline.partition_tool_batches(&calls);

    assert_eq!(batches, vec![vec![0], vec![1], vec![2]]);
}

#[tokio::test]
async fn concurrent_batch_preserves_original_order() {
    let h = harness(vec![]);
    h.add(
        ProbeSpec::new("slow")
            .concurrent()
            .with_delay(Duration::from_millis(40))
            .with_result(ToolResult::ok(json!({ "who": "slow" }))),
    );
    h.add(
        ProbeSpec::new("fast")
            .concurrent()
            .with_result(ToolResult::ok(json!({ "who": "fast" }))),
    );
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(
            &[call("c1", "slow"), call("c2", "fast")],
            &ctx(),
            &allow_all(),
            &sink,
        )
        .await;

    assert_eq!(outcomes.len(), 2);
    assert_eq!(outcomes[0].tool_call_id, "c1");
    assert_eq!(outcomes[1].tool_call_id, "c2");
    assert_eq!(outcomes[0].result.data.as_ref().unwrap()["who"], "slow");

    let events = sink.events();
    assert_eq!(events.len(), 4);
    assert!(matches!(events[0], PipelineEvent::Started { .. }));
    assert!(matches!(events[1], PipelineEvent::Started { .. }));
    assert!(matches!(events[2], PipelineEvent::Finished { .. }));
    assert!(matches!(events[3], PipelineEvent::Finished { .. }));
}

#[tokio::test]
async fn serial_tool_breaks_the_concurrent_batch() {
    let h = harness(vec![]);
    h.add(
        ProbeSpec::new("writer")
            .with_delay(Duration::from_millis(20))
            .with_result(ToolResult::ok(json!({ "who": "writer" }))),
    );
    h.add(
        ProbeSpec::new("reader")
            .concurrent()
            .with_delay(Duration::from_millis(20))
            .with_result(ToolResult::ok(json!({ "who": "reader" }))),
    );
    let sink = InMemoryEventSink::new();

    let outcomes = h
        .pipeline
        .execute(
            &[call("c1", "writer"), call("c2", "reader")],
            &ctx(),
            &allow_all(),
            &sink,
        )
        .await;

    assert_eq!(outcomes.len(), 2);
    assert_eq!(outcomes[0].tool_call_id, "c1");
    assert_eq!(outcomes[1].tool_call_id, "c2");
    assert_eq!(
        h.pipeline
            .partition_tool_batches(&[call("c1", "writer"), call("c2", "reader")])
            .len(),
        2
    );
}

#[tokio::test]
async fn round_budget_persists_only_the_overflowing_result() {
    let per_result_chars = 45_000;
    let text = "q".repeat(per_result_chars);
    let h = harness(vec![]);
    h.add(
        ProbeSpec::new("probe")
            .concurrent()
            .with_result(ToolResult::ok_with_assistant(json!({ "big": true }), text)),
    );
    let sink = InMemoryEventSink::new();
    let calls: Vec<ToolCall> = (0..5)
        .map(|index| ToolCall::new(format!("c{}", index), "probe", json!({})))
        .collect();

    let outcomes = h
        .pipeline
        .execute(&calls, &ctx(), &allow_all(), &sink)
        .await;

    let round_total_chars = per_result_chars * calls.len();
    assert!(round_total_chars > PER_ROUND_LIMIT_CHARS);
    let persisted: Vec<usize> = outcomes
        .iter()
        .enumerate()
        .filter(|(_, outcome)| outcome.persisted.is_some())
        .map(|(index, _)| index)
        .collect();

    assert_eq!(
        persisted,
        vec![4],
        "each result is below the per-tool limit, so only the round overflow is persisted"
    );
    assert!(outcomes[0].persisted.is_none());
    assert!(outcomes[4]
        .assistant_text
        .starts_with("<persisted-output path="));
}

#[tokio::test]
async fn pipeline_source_has_no_tauri_dependency() {
    let source = include_str!("../src/pipeline.rs");
    for forbidden in ["tauri", "AppHandle", "Window"] {
        assert!(
            !source.contains(forbidden),
            "pipeline must stay UI-agnostic: found '{}'",
            forbidden
        );
    }
}

/// V1 冒烟 e2e：生产注册表 → 真实 `ToolPipeline` → 真实 `os` 工具 → 真实 tempdir。
///
/// 只锁这条接缝：`read` / `write` 各一次，走真实注册表（含真实权限意图与审批网关）、
/// 真实执行与真实落盘。批处理、方言、重试等分支已由组件级测试覆盖，不在此重复。
///
/// 读的目标用预置文件而非刚写出的文件，与本用例要锁的接缝（注册表 → 管线 → 真实 IO）保持正交。
#[tokio::test]
async fn production_os_tools_round_trip_through_the_pipeline() {
    use assistant_tools::storage::InMemoryReadReceiptStore;
    use assistant_tools::{os, ReadReceiptStore};

    let dir = tempfile::tempdir().unwrap();
    let seed = dir.path().join("seed.txt");
    std::fs::write(&seed, "beta\n").unwrap();

    let registry = Arc::new(ToolRegistry::new());
    let receipts: Arc<dyn ReadReceiptStore> = Arc::new(InMemoryReadReceiptStore::new());
    os::register_os_tools(&registry);
    os::register_readonly_tools(&registry, receipts.clone());
    os::register_write_tools(&registry, receipts);

    let audit = Arc::new(InMemoryAuditStore::default());
    let gateway = Arc::new(ToolGateway::new(
        Arc::new(InMemoryGrantStore::default()),
        audit.clone(),
        Arc::new(ScriptedApprovalPrompter::new(vec![])),
        Duration::from_secs(5),
    ));
    let storage = Arc::new(RecordingStorage::default());
    let pipeline = ToolPipeline::new(registry, gateway, storage.clone());
    let sink = InMemoryEventSink::new();

    let context = ToolUseContext {
        workspace_root: Some(dir.path().display().to_string()),
        session_id: Some("smoke".to_string()),
        turn_id: Some("turn-1".to_string()),
        iteration: Some(1),
        tool_call_id: None,
        ..Default::default()
    };

    let outcomes = pipeline
        .execute(
            &[ToolCall::new(
                "call-1",
                "read",
                json!({ "path": "seed.txt" }),
            )],
            &context,
            &allow_all(),
            &sink,
        )
        .await;
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].result.ok, "{:?}", outcomes[0].result.error);
    assert_eq!(outcomes[0].stage, PipelineStage::Execute);
    assert!(
        outcomes[0].assistant_text.contains("beta"),
        "the read tool must return the real file content: {}",
        outcomes[0].assistant_text
    );

    let target = dir.path().join("smoke.txt");
    let outcomes = pipeline
        .execute(
            &[ToolCall::new(
                "call-2",
                "write",
                json!({ "path": "smoke.txt", "content": "alpha\n" }),
            )],
            &context,
            &allow_all(),
            &sink,
        )
        .await;
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].result.ok, "{:?}", outcomes[0].result.error);
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "alpha\n",
        "the write tool must hit the real file system"
    );

    // `read` 现在声明 `read` intent（I50 T3.1），因此走策略求值并记 `Allowed`；
    // 这里只锁「两个工具都走完真实网关并留下审计」。
    let entries = audit.entries();
    assert!(
        entries.iter().any(|entry| entry.tool == "write"),
        "the write call must pass through the real gateway audit: {entries:?}"
    );
    let read_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.tool == "read")
        .collect();
    assert!(
        !read_entries.is_empty(),
        "the read call must be audited: {entries:?}"
    );
    assert!(
        read_entries
            .iter()
            .all(|entry| entry.outcome == AuditOutcome::Allowed && entry.action == "read"),
        "read declares a read intent, so it must be audited as Allowed: {entries:?}"
    );
    assert_eq!(
        storage.calls().len(),
        0,
        "small results must not be persisted"
    );
}
