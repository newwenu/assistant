use std::sync::{Arc, Mutex};

use assistant_tools::gateway::{
    InMemoryAuditStore, InMemoryGrantStore, ScriptedApprovalPrompter, ToolGateway,
};
use assistant_tools::os;
use assistant_tools::os::agent_request_tool::{MANUAL_MAX_CHARS, MANUAL_TOTAL_CHARS};
use assistant_tools::pipeline::{InMemoryEventSink, PipelineEvent, ToolCall, ToolPipeline};
use assistant_tools::storage::{
    InMemoryReadReceiptStore, PersistToolResult, PersistedRef, ReadReceiptStore, ToolResultSink,
    ToolResultStorage,
};
use assistant_tools::{
    ApprovalDecision, EscalationCandidate, EscalationOutcome, EscalationRejectReason,
    EscalationRejection, PermissionEffect, PermissionRule, ResolvedPermissionPolicy, Tool,
    ToolAudience, ToolCatalog, ToolEscalationPort, ToolRegistry, ToolResult, ToolTier,
    ToolUseContext, ValidationResult,
};
use async_trait::async_trait;
use serde_json::{json, Value};

fn receipts() -> Arc<dyn ReadReceiptStore> {
    Arc::new(InMemoryReadReceiptStore::new())
}

/// `fs_link`（TOOLBOX_ONLY）+ 只读组：目录应只看见只读组那五个。
fn readonly_registry() -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    os::register_bridge_tools(&registry);
    os::register_readonly_tools(&registry, receipts());
    registry
}

fn allow_all() -> ResolvedPermissionPolicy {
    ResolvedPermissionPolicy::new(
        vec![PermissionRule::new("*", "*", PermissionEffect::Allow)],
        Vec::new(),
    )
}

fn context(available: Option<Vec<&str>>, catalog: Option<Arc<ToolRegistry>>) -> ToolUseContext {
    ToolUseContext {
        available_tool_names: available
            .map(|names| names.into_iter().map(str::to_string).collect()),
        tool_catalog: catalog.map(|registry| registry as Arc<dyn ToolCatalog>),
        ..Default::default()
    }
}

async fn request(input: Value, context: &ToolUseContext) -> ToolResult {
    os::agent_request_tool::AgentRequestTool::new()
        .execute(&input, context)
        .await
        .expect("request_tool must never fail through the Err channel")
}

fn listed(result: &ToolResult) -> Vec<String> {
    result.data.as_ref().expect("data")["listed"]
        .as_array()
        .expect("listed array")
        .iter()
        .map(|value| value.as_str().expect("name").to_string())
        .collect()
}

fn source(result: &ToolResult) -> String {
    result.data.as_ref().expect("data")["source"]
        .as_str()
        .expect("source")
        .to_string()
}

fn assistant_text(result: &ToolResult) -> String {
    result.result_for_assistant.clone().expect("assistant text")
}

/// 手册回退路径（`manual()` 缺省）用的探针：`description` 超长，用来验证裁剪。
struct DescribedTool {
    name: &'static str,
    description_chars: usize,
}

#[async_trait]
impl Tool for DescribedTool {
    fn id(&self) -> &str {
        self.name
    }

    async fn description(&self) -> Result<String, String> {
        Ok("x".repeat(self.description_chars))
    }

    fn input_schema(&self) -> Value {
        json!({ "type": "object" })
    }

    fn tier(&self) -> ToolTier {
        ToolTier::Os
    }

    fn audience(&self) -> ToolAudience {
        ToolAudience::AGENT_ONLY
    }

    fn validate_input(&self, _input: &Value, _context: &ToolUseContext) -> ValidationResult {
        ValidationResult::ok()
    }

    async fn execute(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<ToolResult, String> {
        Ok(ToolResult::ok(json!({})))
    }
}

/// G1：`Tool::manual()` 有默认实现，因此既有工具零改动即可进目录；
/// 缺省手册回退 `description()`，不会出现空条目。
#[tokio::test]
async fn the_default_manual_keeps_every_registered_tool_in_the_catalog() {
    let registry = readonly_registry();
    let entries = registry.entries().await;

    let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["glob", "grep", "ls", "read", "request_tool", "super_read"],
        "the catalog lists exactly the agent-facing tools, sorted by id"
    );
    assert!(
        !ids.contains(&"fs_link"),
        "TOOLBOX_ONLY tools must stay out of the catalog: {:?}",
        ids
    );
    for entry in &entries {
        assert_eq!(
            entry.name, entry.id,
            "no tool overrides `name()`, so the model-visible name equals the id"
        );
        assert!(
            !entry.manual.trim().is_empty(),
            "'{}' has neither a manual nor a description",
            entry.id
        );
    }
}

/// G2：清单以本轮真实工具集为准，而不是注册表全集。
#[tokio::test]
async fn the_list_follows_the_turn_tool_set() {
    let registry = readonly_registry();
    let context = context(Some(vec!["read", "request_tool"]), Some(registry));

    let result = request(json!({}), &context).await;
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(listed(&result), vec!["read", "request_tool"]);
    assert_eq!(source(&result), "turn");
    assert_eq!(result.data.as_ref().expect("data")["available"], 2);
    assert_eq!(result.data.as_ref().expect("data")["manuals"], 0);

    let text = assistant_text(&result);
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines.contains(&"- read") && lines.contains(&"- request_tool"),
        "{text}"
    );
    assert!(
        !lines.contains(&"- ls"),
        "a tool outside the turn set must not be listed: {text}"
    );
    assert!(
        !text.contains("falls back to the registry"),
        "the turn path must not carry the fallback warning: {text}"
    );
}

/// G4：只读——调用前后注册表事实（目录）逐项不变，本轮工具集也不被改写。
#[tokio::test]
async fn the_call_does_not_change_the_tool_set() {
    let registry = readonly_registry();
    let before = registry.entries().await;
    let context = context(Some(vec!["read"]), Some(registry.clone()));

    let result = request(json!({ "detail": "manual" }), &context).await;
    assert!(result.ok, "{:?}", result.error);

    let after = registry.entries().await;
    assert_eq!(
        before, after,
        "request_tool must never mutate the registry or the turn's tool set"
    );
    assert_eq!(
        context.available_tool_names.as_deref(),
        Some(&["read".to_string()][..]),
        "the injected turn tool set must stay untouched"
    );
}

/// G3：没有本轮事实时回退注册表，并**显式**说明回退（清单可能含本轮未启用的工具）。
#[tokio::test]
async fn the_list_falls_back_to_the_registry_and_says_so() {
    let registry = readonly_registry();
    let context = context(None, Some(registry));

    let result = request(json!({}), &context).await;
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(
        listed(&result),
        vec!["glob", "grep", "ls", "read", "request_tool", "super_read"],
        "the fallback list is the registry's agent-facing set, sorted by id"
    );
    assert_eq!(source(&result), "registry");

    let text = assistant_text(&result);
    assert!(text.contains("source: registry"), "{text}");
    assert!(
        text.contains("falls back to the registry") && text.contains("not enabled in this turn"),
        "the fallback must be labelled, otherwise the model trusts a wrong list: {text}"
    );
}

/// `detail = "manual"` 按 `query` 过滤，且过滤对 `id` / `name` 大小写不敏感。
#[tokio::test]
async fn manual_detail_returns_only_the_matching_manuals() {
    let registry = readonly_registry();
    let context = context(Some(vec!["read", "ls", "request_tool"]), Some(registry));

    let result = request(json!({ "detail": "manual", "query": "RE" }), &context).await;
    assert!(result.ok, "{:?}", result.error);
    let text = assistant_text(&result);
    assert!(text.contains("## read\n"), "{text}");
    assert!(text.contains("## request_tool\n"), "{text}");
    assert!(
        !text.contains("## ls\n"),
        "query must filter the manuals: {text}"
    );
    assert_eq!(result.data.as_ref().expect("data")["manuals"], 2);
}

/// `query` 缺省 = 返回全部手册；`detail` 缺省 = 只返回清单。
#[tokio::test]
async fn manual_detail_without_a_query_returns_every_manual() {
    let registry = readonly_registry();
    let context = context(Some(vec!["ls", "read"]), Some(registry));

    let result = request(json!({ "detail": "manual" }), &context).await;
    let text = assistant_text(&result);
    assert!(text.contains("## ls\n"), "{text}");
    assert!(text.contains("## read\n"), "{text}");
    assert_eq!(result.data.as_ref().expect("data")["manuals"], 2);
}

/// G5：每工具 ≤ 2000 字符、总计 ≤ 8000 字符，超额时给出可识别的截断标记。
#[tokio::test]
async fn manuals_are_capped_per_tool_and_in_total() {
    let registry = Arc::new(ToolRegistry::new());
    for name in ["aa", "bb", "cc", "dd", "ee", "ff"] {
        registry.register(Arc::new(DescribedTool {
            name,
            description_chars: MANUAL_MAX_CHARS + 500,
        }));
    }
    let context = context(None, Some(registry));

    let result = request(json!({ "detail": "manual" }), &context).await;
    let text = assistant_text(&result);

    // 尾标记不属于任何手册，先剥离再逐块计量。
    let body_text = match text.split_once("\n\n[catalog: showing") {
        Some((head, _)) => head,
        None => text.as_str(),
    };
    let blocks: Vec<&str> = body_text.split("\n\n## ").skip(1).collect();
    assert!(!blocks.is_empty(), "{text}");
    let mut total = 0usize;
    for block in &blocks {
        let body = block.split_once('\n').map(|(_, body)| body).unwrap_or("");
        let chars = body.chars().count();
        assert!(
            chars <= MANUAL_MAX_CHARS,
            "a single manual exceeded the hard cap: {chars} chars"
        );
        assert!(body.ends_with("(manual truncated)"), "{body}");
        total += chars;
    }
    assert!(
        total <= MANUAL_TOTAL_CHARS,
        "the manuals total exceeded the cap: {total} chars"
    );
    assert!(
        text.contains("[catalog: showing 3 of 6]"),
        "truncation must be announced in the footer form the edit gate recognizes: {text}"
    );
    assert_eq!(result.data.as_ref().expect("data")["manuals"], 3);
}

/// 没有目录（ToolBox 直调、或未注入）时是业务错误，不是内部错误。
#[tokio::test]
async fn without_a_catalog_the_call_is_a_business_error() {
    let context = context(Some(vec!["read"]), None);
    let result = request(json!({}), &context).await;

    assert!(!result.ok);
    assert_eq!(
        result.error_kind,
        Some(assistant_tools::ToolErrorKind::Business),
        "the model must be able to correct this, so it is not an internal error"
    );
    assert!(result
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("catalog is unavailable"));
}

/// `detail` 只接受封闭集合，非法值在 `validate_input` 阶段就被拦下。
#[test]
fn an_unknown_detail_is_rejected_at_validation() {
    let tool = os::agent_request_tool::AgentRequestTool::new();
    let context = ToolUseContext::default();
    let validation = tool.validate_input(&json!({ "detail": "everything" }), &context);
    assert!(!validation.ok);
    let message = validation.message.as_deref().unwrap_or_default();
    assert!(
        message.contains("\"enable\""),
        "the closed set must list every accepted value: {message}"
    );
}

#[derive(Debug, Default)]
struct NullStorage;

#[async_trait]
impl ToolResultStorage for NullStorage {
    async fn persist(&self, _request: PersistToolResult) -> Result<PersistedRef, String> {
        Ok(PersistedRef {
            path: "store/null.txt".to_string(),
            bytes: 0,
        })
    }

    async fn open_sink(
        &self,
        _request: PersistToolResult,
    ) -> Result<Box<dyn ToolResultSink>, String> {
        Err("null storage does not stream".to_string())
    }
}

/// 目录由 Pipeline 注入：生产路径上工具自己拿不到注册表，唯一来源是上下文。
#[tokio::test]
async fn the_pipeline_injects_the_catalog() {
    let registry = readonly_registry();
    let gateway = Arc::new(ToolGateway::new(
        Arc::new(InMemoryGrantStore::default()),
        Arc::new(InMemoryAuditStore::default()),
        Arc::new(ScriptedApprovalPrompter::new(Vec::<ApprovalDecision>::new())),
        std::time::Duration::from_secs(5),
    ));
    let pipeline = ToolPipeline::new(registry.clone(), gateway, Arc::new(NullStorage));
    let sink = InMemoryEventSink::new();

    // 调用方不注入目录，Pipeline 必须自己补上。
    let outcomes = pipeline
        .execute(
            &[ToolCall::new("c1", "request_tool", json!({}))],
            &context(None, None),
            &allow_all(),
            &sink,
        )
        .await;

    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].result.ok, "{:?}", outcomes[0].result.error);
    let text = match sink
        .events()
        .into_iter()
        .find(|event| matches!(event, PipelineEvent::Finished { .. }))
    {
        Some(PipelineEvent::Finished { assistant_text, .. }) => assistant_text,
        other => panic!("expected Finished event, got {other:?}"),
    };
    assert!(
        text.contains("Available tools (6, source: registry)"),
        "the injected catalog must be the registry itself: {text}"
    );
}

// ---------------------------------------------------------------------------
// 升级通道（I58 §3.8 / §10 G21·G23）
// ---------------------------------------------------------------------------

/// 脚本化端口：可升级段与申请结果都由用例给定，只记录被调用的入参。
struct ScriptedEscalationPort {
    candidates: Vec<EscalationCandidate>,
    outcome: EscalationOutcome,
    requested: Mutex<Vec<Vec<String>>>,
}

impl ScriptedEscalationPort {
    fn new(candidates: Vec<EscalationCandidate>, outcome: EscalationOutcome) -> Self {
        Self {
            candidates,
            outcome,
            requested: Mutex::new(Vec::new()),
        }
    }

    fn requested(&self) -> Vec<Vec<String>> {
        match self.requested.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[async_trait]
impl ToolEscalationPort for ScriptedEscalationPort {
    async fn candidates(&self, _available: &[String]) -> Vec<EscalationCandidate> {
        self.candidates.clone()
    }

    async fn request(&self, requested: &[String], _available: &[String]) -> EscalationOutcome {
        match self.requested.lock() {
            Ok(mut guard) => guard.push(requested.to_vec()),
            Err(poisoned) => poisoned.into_inner().push(requested.to_vec()),
        }
        self.outcome.clone()
    }
}

fn candidate(id: &str, brief: &str) -> EscalationCandidate {
    EscalationCandidate {
        id: id.to_string(),
        name: id.to_string(),
        brief: brief.to_string(),
    }
}

fn context_with_port(
    available: Vec<&str>,
    catalog: Arc<ToolRegistry>,
    port: Arc<dyn ToolEscalationPort>,
) -> ToolUseContext {
    ToolUseContext {
        available_tool_names: Some(available.into_iter().map(str::to_string).collect()),
        tool_catalog: Some(catalog as Arc<dyn ToolCatalog>),
        escalation: Some(port),
        ..Default::default()
    }
}

fn escalatable(result: &ToolResult) -> Vec<String> {
    result
        .data
        .as_ref()
        .expect("data")
        .get("escalatable")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|value| value.as_str().expect("name").to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn enabled(result: &ToolResult) -> Vec<String> {
    result.data.as_ref().expect("data")["enabled"]
        .as_array()
        .expect("enabled array")
        .iter()
        .map(|value| value.as_str().expect("name").to_string())
        .collect()
}

fn rejected(result: &ToolResult) -> Vec<(String, String)> {
    result.data.as_ref().expect("data")["rejected"]
        .as_array()
        .expect("rejected array")
        .iter()
        .map(|value| {
            (
                value["id"].as_str().expect("id").to_string(),
                value["reason"].as_str().expect("reason").to_string(),
            )
        })
        .collect()
}

/// G23 ⑤ / G19：可升级段只下发 `brief`（一句用途），不含 schema 与手册正文。
#[tokio::test]
async fn the_escalatable_section_carries_the_brief_only() {
    let registry = readonly_registry();
    let port: Arc<dyn ToolEscalationPort> = Arc::new(ScriptedEscalationPort::new(
        vec![candidate("super_read", "Read by line index.")],
        EscalationOutcome::default(),
    ));
    let context = context_with_port(vec!["read", "request_tool"], registry, port);

    let result = request(json!({}), &context).await;
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(escalatable(&result), vec!["super_read"]);

    let text = assistant_text(&result);
    assert!(
        text.contains("- super_read: Read by line index."),
        "the candidate line must be the brief: {text}"
    );
    assert!(
        text.contains("detail = \"enable\""),
        "the section must name the only way to enable: {text}"
    );
    assert!(
        !text.contains("input_schema") && !text.contains("\"properties\""),
        "the escalatable section must not carry a schema: {text}"
    );
    assert!(
        !text.contains("## super_read"),
        "the manual belongs to detail = \"manual\", not to the escalation section: {text}"
    );
}

/// G23 ④：已在本轮的伴随工具既不进 `listed`，也不进可升级段。
#[tokio::test]
async fn an_available_companion_is_not_escalatable() {
    let registry = readonly_registry();
    // 端口按真实判据返回空段：`super_read` 已在可用名里。
    let port: Arc<dyn ToolEscalationPort> = Arc::new(ScriptedEscalationPort::new(
        Vec::new(),
        EscalationOutcome::default(),
    ));
    let context = context_with_port(vec!["read", "super_read", "request_tool"], registry, port);

    let result = request(json!({}), &context).await;
    assert!(result.ok, "{:?}", result.error);
    assert!(escalatable(&result).is_empty());
    assert!(!assistant_text(&result).contains("detail = \"enable\""));
}

/// §3.8 ⑥：没有升级端口时不猜、不用目录代算——可升级段整段不出现。
#[tokio::test]
async fn without_a_port_there_is_no_escalatable_section() {
    let registry = readonly_registry();
    let context = context(Some(vec!["read", "request_tool"]), Some(registry));

    let result = request(json!({}), &context).await;
    assert!(result.ok, "{:?}", result.error);
    assert!(
        result
            .data
            .as_ref()
            .expect("data")
            .get("escalatable")
            .is_none(),
        "a missing port must not produce a second source of truth"
    );
    assert!(!assistant_text(&result).contains("detail = \"enable\""));
}

/// F13：`detail = "enable"` 缺 `tools` → 业务错误，但仍带可升级段（否则模型无从纠正）。
#[tokio::test]
async fn enable_without_tools_is_a_business_error_that_keeps_the_candidates() {
    let registry = readonly_registry();
    let port: Arc<dyn ToolEscalationPort> = Arc::new(ScriptedEscalationPort::new(
        vec![candidate("super_read", "Read by line index.")],
        EscalationOutcome::default(),
    ));
    let context = context_with_port(vec!["read", "request_tool"], registry, port);

    let result = request(json!({ "detail": "enable" }), &context).await;
    assert!(!result.ok);
    assert_eq!(
        result.error_kind,
        Some(assistant_tools::ToolErrorKind::Business)
    );
    assert!(enabled(&result).is_empty());
    assert!(rejected(&result).is_empty());
    assert_eq!(
        escalatable(&result),
        vec!["super_read"],
        "the business error must still tell the model what it can ask for"
    );
    assert!(result
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("non-empty 'tools'"));
}

/// §3.8 ⑥：端口缺失时 `enable` 是业务错误，不是「静默成功但没生效」。
#[tokio::test]
async fn enable_without_a_port_is_a_business_error() {
    let registry = readonly_registry();
    let context = context(Some(vec!["read", "request_tool"]), Some(registry));

    let result = request(
        json!({ "detail": "enable", "tools": ["super_read"] }),
        &context,
    )
    .await;
    assert!(!result.ok);
    assert_eq!(
        result.error_kind,
        Some(assistant_tools::ToolErrorKind::Business)
    );
    assert!(enabled(&result).is_empty());
    assert!(result
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("escalation is unavailable"));
}

/// F14：全部被拒 → `ok = false`，逐条原因进 `data.rejected`。
#[tokio::test]
async fn enable_all_rejected_reports_every_reason() {
    let registry = readonly_registry();
    let port: Arc<dyn ToolEscalationPort> = Arc::new(ScriptedEscalationPort::new(
        Vec::new(),
        EscalationOutcome {
            enabled: Vec::new(),
            rejected: vec![EscalationRejection {
                id: "super_read".to_string(),
                reason: EscalationRejectReason::BaseNotAvailable,
            }],
        },
    ));
    let context = context_with_port(vec!["edit", "request_tool"], registry, port);

    let result = request(
        json!({ "detail": "enable", "tools": ["super_read"] }),
        &context,
    )
    .await;
    assert!(!result.ok);
    assert_eq!(
        result.error_kind,
        Some(assistant_tools::ToolErrorKind::Business)
    );
    assert!(enabled(&result).is_empty());
    assert_eq!(
        rejected(&result),
        vec![("super_read".to_string(), "base_not_available".to_string())]
    );
    let text = assistant_text(&result);
    assert!(
        text.contains("super_read (base_not_available): its base tool is not available"),
        "the reason code and its message must both be shown: {text}"
    );
}

/// §3.8 ⑤：部分通过 → `ok = true`，`enabled` 与 `rejected` 同时非空。
#[tokio::test]
async fn enable_partial_pass_is_ok_and_reports_both_lists() {
    let registry = readonly_registry();
    let scripted = Arc::new(ScriptedEscalationPort::new(
        Vec::new(),
        EscalationOutcome {
            enabled: vec!["super_read".to_string()],
            rejected: vec![EscalationRejection {
                id: "read".to_string(),
                reason: EscalationRejectReason::NotCompanion,
            }],
        },
    ));
    let port: Arc<dyn ToolEscalationPort> = scripted.clone();
    let context = context_with_port(vec!["read", "request_tool"], registry, port);

    let result = request(
        json!({ "detail": "enable", "tools": ["super_read", "read"] }),
        &context,
    )
    .await;
    assert!(result.ok, "a registered item makes the call a success");
    assert_eq!(enabled(&result), vec!["super_read"]);
    assert_eq!(
        rejected(&result),
        vec![("read".to_string(), "not_companion".to_string())]
    );

    let text = assistant_text(&result);
    assert!(
        text.contains("effective from the next tool call")
            && text.contains("the current iteration's tool set is unchanged"),
        "the reply must state when the change takes effect: {text}"
    );
    assert!(text.contains("Not enabled:"), "{text}");

    let port = scripted
        .requested()
        .into_iter()
        .next()
        .expect("the port must receive the request");
    assert_eq!(port, vec!["super_read".to_string(), "read".to_string()]);
}

/// G4 / G21：`enable` 不改本轮工具集——生效只发生在批次结束后的尾部追加。
#[tokio::test]
async fn enable_does_not_touch_the_turn_tool_set() {
    let registry = readonly_registry();
    let before = registry.entries().await;
    let port: Arc<dyn ToolEscalationPort> = Arc::new(ScriptedEscalationPort::new(
        Vec::new(),
        EscalationOutcome {
            enabled: vec!["super_read".to_string()],
            rejected: Vec::new(),
        },
    ));
    let context = context_with_port(vec!["read", "request_tool"], registry.clone(), port);

    let result = request(
        json!({ "detail": "enable", "tools": ["super_read"] }),
        &context,
    )
    .await;
    assert!(result.ok, "{:?}", result.error);

    assert_eq!(
        registry.entries().await,
        before,
        "the registry is never mutated by an escalation"
    );
    assert_eq!(
        context.available_tool_names.as_deref(),
        Some(&["read".to_string(), "request_tool".to_string()][..]),
        "the turn tool set changes only when the loop appends after the batch"
    );
}

/// G23 ④：已生效的项从可升级段里移除（同一份回复里不能自相矛盾）。
#[tokio::test]
async fn enabled_items_leave_the_escalatable_section() {
    let registry = readonly_registry();
    let port: Arc<dyn ToolEscalationPort> = Arc::new(ScriptedEscalationPort::new(
        vec![
            candidate("super_edit", "Replace by line range."),
            candidate("super_read", "Read by line index."),
        ],
        EscalationOutcome {
            enabled: vec!["super_read".to_string()],
            rejected: Vec::new(),
        },
    ));
    let context = context_with_port(vec!["read", "edit", "request_tool"], registry, port);

    let result = request(
        json!({ "detail": "enable", "tools": ["super_read"] }),
        &context,
    )
    .await;
    assert!(result.ok, "{:?}", result.error);
    assert_eq!(
        escalatable(&result),
        vec!["super_edit"],
        "a just-enabled tool must not stay in the escalatable list"
    );
}
