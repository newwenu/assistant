//! 升级端口准入（I58 §10 G21 / G17）与 `ToolSpec` 解析（§3.8 ③）。
//!
//! 只驱动 `RegistryEscalationPort`：注册表 + 本轮层级 + 本轮收集器，不经过循环。

use std::sync::{Arc, Mutex};

use assistant_agent::agent::round_executor::EscalationSink;
use assistant_app::adapters::{resolve_available_tools, RegistryEscalationPort};
use assistant_providers::types::ToolSpec;
use assistant_tools::storage::InMemoryReadReceiptStore;
use assistant_tools::{
    os, EscalationRejectReason, ReadReceiptStore, ToolEscalationPort, ToolRegistry, ToolTier,
};

fn receipts() -> Arc<dyn ReadReceiptStore> {
    Arc::new(InMemoryReadReceiptStore::new())
}

/// 全量注册：只读组 + 修改组，让 `read` / `edit` 与它们的伴随工具都在。
fn full_registry() -> Arc<ToolRegistry> {
    let registry = Arc::new(ToolRegistry::new());
    os::register_os_tools(&registry);
    os::register_readonly_tools(&registry, receipts());
    os::register_write_tools(&registry, receipts());
    os::register_dangerous_tools(&registry, receipts());
    registry
}

/// 本轮收集器替身：记录端口登记的成品声明，`drain` 后清空。
#[derive(Default)]
struct TestSink {
    pending: Mutex<Vec<ToolSpec>>,
}

impl TestSink {
    fn pushed(&self) -> Vec<ToolSpec> {
        match self.pending.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl EscalationSink for TestSink {
    fn push(&self, tool: ToolSpec) {
        match self.pending.lock() {
            Ok(mut guard) => guard.push(tool),
            Err(poisoned) => poisoned.into_inner().push(tool),
        }
    }

    fn drain(&self) -> Vec<ToolSpec> {
        match self.pending.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
        }
    }
}

fn port_with(max_tier: ToolTier, sink: Arc<TestSink>) -> RegistryEscalationPort {
    RegistryEscalationPort::new(full_registry(), max_tier, sink)
}

fn names(specs: &[ToolSpec]) -> Vec<String> {
    specs.iter().map(|spec| spec.name.clone()).collect()
}

/// G21 / G23 ②：可升级段的判据是「有伴随基工具 ∧ 基工具在本轮 ∧ 自身不在本轮」。
#[tokio::test]
async fn candidates_require_the_base_tool_in_this_turn() {
    let sink = Arc::new(TestSink::default());
    let port = port_with(ToolTier::Os, sink);

    // `edit` 不在本轮：`super_edit` 不构成候选；`super_read` 的基工具 `read` 在。
    let candidates = port.candidates(&["read".to_string()]).await;
    assert_eq!(
        candidates
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>(),
        vec!["super_read"],
        "only companions whose base tool is available may be candidates"
    );
    let candidate = &candidates[0];
    assert_eq!(candidate.id, "super_read");
    assert!(
        !candidate.brief.is_empty(),
        "a candidate must carry its brief (one-line purpose)"
    );
    assert!(
        !candidate.brief.contains("input_schema"),
        "brief must not leak the schema"
    );

    // 两个基工具都在：两个伴随都成为候选，按 id 字典序。
    let candidates = port
        .candidates(&["read".to_string(), "edit".to_string()])
        .await;
    assert_eq!(
        candidates.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
        vec!["super_edit", "super_read"],
        "candidates are sorted by id"
    );
}

/// G23 ④：已在轮内的工具不再出现在可升级段（`listed ∩ escalatable = ∅`）。
#[tokio::test]
async fn candidates_exclude_tools_that_are_already_available() {
    let sink = Arc::new(TestSink::default());
    let port = port_with(ToolTier::Os, sink);
    let available = vec!["read".to_string(), "super_read".to_string()];
    assert!(
        port.candidates(&available).await.is_empty(),
        "an available tool cannot be escalated again"
    );
}

/// G21：基工具不在本轮 → F14 `base_not_available`，收集器保持不变。
#[tokio::test]
async fn escalating_without_the_base_tool_is_rejected() {
    let sink = Arc::new(TestSink::default());
    let port = port_with(ToolTier::Os, sink.clone());

    let outcome = port
        .request(&["super_read".to_string()], &["edit".to_string()])
        .await;
    assert!(outcome.enabled.is_empty(), "nothing may be enabled");
    assert_eq!(outcome.rejected.len(), 1);
    assert_eq!(outcome.rejected[0].id, "super_read");
    assert_eq!(
        outcome.rejected[0].reason,
        EscalationRejectReason::BaseNotAvailable
    );
    assert!(
        sink.pushed().is_empty(),
        "a rejected request must not reach the sink"
    );
}

/// G21：申请非伴随工具 → `not_companion`。
#[tokio::test]
async fn escalating_a_non_companion_tool_is_rejected() {
    let sink = Arc::new(TestSink::default());
    let port = port_with(ToolTier::Os, sink);

    let outcome = port
        .request(&["read".to_string()], &["read".to_string()])
        .await;
    assert_eq!(outcome.rejected.len(), 1);
    assert_eq!(
        outcome.rejected[0].reason,
        EscalationRejectReason::NotCompanion
    );
}

/// G21 / F13：名字在注册表里查不到 → `not_in_catalog`。
#[tokio::test]
async fn escalating_an_unknown_name_is_rejected() {
    let sink = Arc::new(TestSink::default());
    let port = port_with(ToolTier::Os, sink);

    let outcome = port
        .request(&["no_such_tool".to_string()], &["read".to_string()])
        .await;
    assert_eq!(outcome.rejected.len(), 1);
    assert_eq!(
        outcome.rejected[0].reason,
        EscalationRejectReason::NotInCatalog
    );
}

/// G21：层级上限优先于伴随关系——`Sandbox` 上限下 `super_read`（`Os` 层）被拒。
#[tokio::test]
async fn escalating_above_the_turn_tier_limit_is_rejected() {
    let sink = Arc::new(TestSink::default());
    let port = port_with(ToolTier::Sandbox, sink);

    let outcome = port
        .request(&["super_read".to_string()], &["read".to_string()])
        .await;
    assert_eq!(outcome.rejected.len(), 1);
    assert_eq!(
        outcome.rejected[0].reason,
        EscalationRejectReason::TierExceeded
    );
    assert!(
        port.candidates(&["read".to_string()]).await.is_empty(),
        "a tool above the tier limit is not even a candidate"
    );
}

/// §3.8 ③：通过项在端口侧解析成完整声明（`name` + `description()` + `input_schema()`），
/// 且是 `full` 档而不是 `brief`。
#[tokio::test]
async fn a_valid_request_registers_a_full_tool_spec() {
    let sink = Arc::new(TestSink::default());
    let port = port_with(ToolTier::Os, sink.clone());

    let outcome = port
        .request(&["super_read".to_string()], &["read".to_string()])
        .await;
    assert_eq!(outcome.enabled, vec!["super_read".to_string()]);
    assert!(outcome.rejected.is_empty());

    let pushed = sink.pushed();
    assert_eq!(names(&pushed), vec!["super_read"]);
    let spec = &pushed[0];
    assert!(
        !spec.description.is_empty() && spec.description.len() > 200,
        "the registered spec carries the full description, not the brief"
    );
    assert_eq!(
        spec.input_schema["type"], "object",
        "the registered spec carries the callable schema"
    );
}

/// §3.8 ⑤：部分通过 → 合法项照常登记，未通过项逐条给出原因。
#[tokio::test]
async fn a_partially_valid_request_keeps_the_valid_items() {
    let sink = Arc::new(TestSink::default());
    let port = port_with(ToolTier::Os, sink.clone());

    let outcome = port
        .request(
            &["super_read".to_string(), "read".to_string()],
            &["read".to_string()],
        )
        .await;
    assert_eq!(outcome.enabled, vec!["super_read".to_string()]);
    assert_eq!(outcome.rejected.len(), 1);
    assert_eq!(outcome.rejected[0].id, "read");
    assert_eq!(
        outcome.rejected[0].reason,
        EscalationRejectReason::NotCompanion
    );
    assert_eq!(names(&sink.pushed()), vec!["super_read"]);
}

/// G17 / G21：`disabledTools` 里的伴随 id **不构成阻断**——用户唯一控制点是基工具。
#[tokio::test]
async fn a_disabled_companion_id_does_not_block_escalation() {
    let registry = full_registry();
    let available = resolve_available_tools(
        &registry,
        &["read".to_string()],
        &["super_read".to_string()],
        ToolTier::Os,
    )
    .await;
    assert_eq!(
        names(&available),
        vec!["read"],
        "a companion id in the blacklist is ignored, not honoured"
    );

    let sink = Arc::new(TestSink::default());
    let port = RegistryEscalationPort::new(registry, ToolTier::Os, sink.clone());
    let available_names: Vec<String> = names(&available);
    let outcome = port
        .request(&["super_read".to_string()], &available_names)
        .await;
    assert_eq!(
        outcome.enabled,
        vec!["super_read".to_string()],
        "the blacklist is not an admission criterion"
    );
    assert_eq!(names(&sink.pushed()), vec!["super_read"]);
}

/// §3.8 ② / G22：收集器按轮次归属——新建端口配新收集器，上一轮登记不带过来。
#[tokio::test]
async fn a_fresh_sink_does_not_carry_previous_escalations() {
    let first_sink = Arc::new(TestSink::default());
    let first = port_with(ToolTier::Os, first_sink.clone());
    first
        .request(&["super_read".to_string()], &["read".to_string()])
        .await;
    assert_eq!(names(&first_sink.pushed()), vec!["super_read"]);

    let second_sink = Arc::new(TestSink::default());
    let second = port_with(ToolTier::Os, second_sink.clone());
    let outcome = second
        .request(&["super_edit".to_string()], &["read".to_string()])
        .await;
    assert_eq!(
        outcome.rejected[0].reason,
        EscalationRejectReason::BaseNotAvailable,
        "`edit` is not available in the new turn"
    );
    assert!(
        second_sink.pushed().is_empty(),
        "escalations do not survive across turns"
    );
}

// ---------------------------------------------------------------------------
// G23：三档描述不变式（none / brief / full）
// ---------------------------------------------------------------------------

/// 注册表里所有 agent 可见的伴随工具及其基工具 id。
fn companion_tools(registry: &ToolRegistry) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for id in registry.tool_ids() {
        let Some(registered) = registry.find(&id) else {
            continue;
        };
        if !registered.tool.audience().agent {
            continue;
        }
        if let Some(base) = registered.tool.companion_of() {
            pairs.push((registered.id, base.to_string()));
        }
    }
    pairs.sort();
    pairs
}

/// 本轮工具集矩阵：从空集到「全部只读 + 修改组 + 元工具 + 显式残留的伴随 id」。
const TURN_SETS: &[&[&str]] = &[
    &[],
    &["read"],
    &["edit"],
    &["read", "edit"],
    &["read", "request_tool"],
    &["read", "edit", "request_tool"],
    &["read", "request_tool", "super_read"],
    &["ls", "glob", "grep", "read", "edit", "request_tool"],
    &[
        "ls",
        "glob",
        "grep",
        "read",
        "edit",
        "request_tool",
        "super_read",
        "super_edit",
    ],
];

/// G23：`full ⇔ callable`、`brief ⇒ eligible`、`none ⇔ ¬eligible`、
/// `listed ∩ escalatable = ∅`——四条不变式在整个矩阵上逐条断言。
#[tokio::test]
async fn g23_the_three_description_tiers_hold_across_the_turn_matrix() {
    let registry = full_registry();
    let companions = companion_tools(&registry);
    assert_eq!(
        companions,
        vec![
            ("super_edit".to_string(), "edit".to_string()),
            ("super_read".to_string(), "read".to_string()),
        ],
        "the registry must declare exactly these two companions"
    );

    for turn in TURN_SETS {
        let enabled: Vec<String> = turn.iter().map(|id| id.to_string()).collect();
        for max_tier in [ToolTier::Sandbox, ToolTier::Os] {
            let callable = resolve_available_tools(&registry, &enabled, &[], max_tier).await;
            let callable_names: Vec<String> = callable.iter().map(|s| s.name.clone()).collect();
            let sink = Arc::new(TestSink::default());
            let port = RegistryEscalationPort::new(registry.clone(), max_tier, sink.clone());
            let candidates = port.candidates(&callable_names).await;
            let context = format!("turn = {turn:?}, tier = {max_tier:?}");

            // full ⇔ callable（callable ⇒ full）：可调用项必带完整描述与可解析的 schema。
            for spec in &callable {
                assert!(
                    !spec.description.is_empty(),
                    "{}: callable '{}' lacks a full description",
                    context,
                    spec.name
                );
                assert_eq!(
                    spec.input_schema["type"], "object",
                    "{}: callable '{}' lacks a callable schema",
                    context, spec.name
                );
            }

            // full ⇔ callable（full ⇒ callable）：伴随工具不得经配置路径成为 full。
            for (id, _base) in &companions {
                assert!(
                    !callable_names.contains(id),
                    "{}: companion '{}' must not be callable without an escalation",
                    context,
                    id
                );
            }

            // listed ∩ escalatable = ∅。
            for candidate in &candidates {
                assert!(
                    !callable_names.contains(&candidate.name)
                        && !callable_names.contains(&candidate.id),
                    "{}: '{}' is both listed and escalatable",
                    context,
                    candidate.id
                );
                // brief ⇒ eligible：可升级项必有基工具在本轮，且摘要非空、无 schema。
                let base = companions
                    .iter()
                    .find(|(id, _)| id == &candidate.id)
                    .map(|(_, base)| base.clone())
                    .expect("a candidate is always a companion");
                assert!(
                    callable_names.contains(&base),
                    "{}: '{}' is brief without its base '{}' in the turn",
                    context,
                    candidate.id,
                    base
                );
                assert!(
                    !candidate.brief.trim().is_empty(),
                    "{}: '{}' has an empty brief",
                    context,
                    candidate.id
                );
                assert!(
                    !candidate.brief.contains("\"properties\"")
                        && !candidate.brief.contains("input_schema"),
                    "{}: the brief of '{}' leaks the schema",
                    context,
                    candidate.id
                );
            }

            // none ⇔ ¬eligible：基工具不在本轮时，该伴随既不在清单也不在可升级段。
            for (id, base) in &companions {
                let eligible = callable_names.contains(base) && !callable_names.contains(id);
                let in_candidates = candidates.iter().any(|c| &c.id == id);
                assert_eq!(
                    in_candidates, eligible,
                    "{}: '{}' must be brief exactly when its base is available",
                    context, id
                );
                assert!(
                    !callable_names.contains(id),
                    "{}: '{}' must never be full without an escalation",
                    context,
                    id
                );
            }
        }
    }
}
