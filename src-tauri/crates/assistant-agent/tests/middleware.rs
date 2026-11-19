mod common;

use assistant_agent::agent::ladder::CLEARED_PLACEHOLDER_PREFIX;
use assistant_agent::agent::middleware::{
    classify_failure, FailureClass, MiddlewareChain, MiddlewareOutcome, ModelRequestView,
    ToolResultView, TurnCtx, EXEMPT_TOOLS, P4_MIDDLEWARE_ORDER, REPEATED_FAILURE_HALT_AT,
    REPEATED_FAILURE_NUDGE_AT, TOOL_OUTPUT_CAP_CHARS,
};
use assistant_agent::agent::round_executor::{ToolErrorClass, ToolExecResult, ToolStage};
use assistant_providers::types::{ChatMessage, MessageRole};
use common::chat::{text_msg, tool_msg};

fn ctx() -> TurnCtx<'static> {
    TurnCtx {
        turn_id: "turn-1",
        conversation_id: "conv-1",
        iteration: 1,
        max_iterations: 12,
        model: "custom",
    }
}

fn result(
    tool: &str,
    is_error: bool,
    stage: ToolStage,
    kind: Option<ToolErrorClass>,
) -> ToolExecResult {
    ToolExecResult {
        call_id: format!("call-{tool}"),
        tool_name: tool.to_string(),
        content: if is_error {
            "boom".to_string()
        } else {
            "ok".to_string()
        },
        is_error,
        stage,
        error_kind: kind,
        confirmation_wait_ms: None,
    }
}

async fn run_after_tool(
    chain: &MiddlewareChain,
    res: &mut ToolExecResult,
    fingerprint: &str,
    scope: &str,
) -> MiddlewareOutcome {
    let ctx = ctx();
    let mut view = ToolResultView {
        result: res,
        arguments: "{}",
        argument_fingerprint: fingerprint,
        scope,
    };
    chain.after_tool(&mut view, &ctx).await.0
}

#[test]
fn p4_chain_matches_declared_order() {
    let chain = MiddlewareChain::p4(6);
    assert_eq!(chain.names(), P4_MIDDLEWARE_ORDER.to_vec());
    assert!(!chain.names().contains(&"summarize"));
    assert_eq!(chain.names().len(), 7);
}

#[test]
fn failure_classification_matches_d25_table() {
    assert_eq!(
        classify_failure(ToolStage::Find, Some(ToolErrorClass::Internal)),
        Some(FailureClass::Validation)
    );
    assert_eq!(
        classify_failure(ToolStage::Validate, Some(ToolErrorClass::Business)),
        Some(FailureClass::Validation)
    );
    assert_eq!(
        classify_failure(ToolStage::Gateway, Some(ToolErrorClass::Denied)),
        Some(FailureClass::Permission)
    );
    assert_eq!(
        classify_failure(ToolStage::Gateway, Some(ToolErrorClass::Internal)),
        Some(FailureClass::Transient)
    );
    assert_eq!(
        classify_failure(ToolStage::Execute, Some(ToolErrorClass::Timeout)),
        Some(FailureClass::UncertainSideEffect)
    );
    assert_eq!(
        classify_failure(ToolStage::Execute, Some(ToolErrorClass::Business)),
        Some(FailureClass::Validation)
    );
    assert_eq!(
        classify_failure(ToolStage::Execute, Some(ToolErrorClass::Internal)),
        Some(FailureClass::Transient)
    );
    assert_eq!(
        classify_failure(ToolStage::Persist, Some(ToolErrorClass::Internal)),
        None
    );
    assert_eq!(
        classify_failure(ToolStage::Execute, Some(ToolErrorClass::Cancelled)),
        None
    );
    assert_eq!(classify_failure(ToolStage::Execute, None), None);
}

#[test]
fn retry_budgets_match_d25_table() {
    assert_eq!(FailureClass::Validation.retry_budget(), 1);
    assert_eq!(FailureClass::Permission.retry_budget(), 0);
    assert_eq!(FailureClass::Transient.retry_budget(), 2);
    assert_eq!(FailureClass::UncertainSideEffect.retry_budget(), 1);
}

#[test]
fn exempt_tools_table_is_empty_in_p4() {
    assert!(EXEMPT_TOOLS.is_empty());
}

#[tokio::test]
async fn tool_output_cap_truncates_oversized_result() {
    let mut body = String::from("head");
    body.push_str(&"m".repeat(TOOL_OUTPUT_CAP_CHARS + 100));
    body.push_str("tail");
    let original_len = body.chars().count();
    let mut res = result("read", false, ToolStage::Execute, None);
    res.content = body;
    let chain = MiddlewareChain::p4(6);
    let out = run_after_tool(&chain, &mut res, "fp", "read").await;
    assert_eq!(out, MiddlewareOutcome::Continue);
    assert!(res.content.chars().count() < original_len);
    assert!(res.content.starts_with("head"));
    assert!(res.content.ends_with("tail"));
}

#[tokio::test]
async fn tool_output_cap_is_noop_below_limit() {
    let mut res = result("read", false, ToolStage::Execute, None);
    res.content = "short".to_string();
    let chain = MiddlewareChain::p4(6);
    let out = run_after_tool(&chain, &mut res, "fp", "read").await;
    assert_eq!(out, MiddlewareOutcome::Continue);
    assert_eq!(res.content, "short");
}

#[tokio::test]
async fn generic_failure_nudges_at_two_and_halts_at_three() {
    assert_eq!(REPEATED_FAILURE_NUDGE_AT, 2);
    assert_eq!(REPEATED_FAILURE_HALT_AT, 3);
    let chain = MiddlewareChain::p4(6);
    let ctx = ctx();
    let mut res = result(
        "read",
        true,
        ToolStage::Persist,
        Some(ToolErrorClass::Internal),
    );

    let out = run_after_tool(&chain, &mut res, "same-args", "read").await;
    assert_eq!(out, MiddlewareOutcome::Continue);
    assert!(chain.take_pending_nudges().is_empty());
    assert_eq!(
        chain.after_iteration(&ctx).await,
        MiddlewareOutcome::Continue
    );

    let _ = run_after_tool(&chain, &mut res, "same-args", "read").await;
    assert_eq!(chain.take_pending_nudges().len(), 1);
    assert_eq!(
        chain.after_iteration(&ctx).await,
        MiddlewareOutcome::Continue
    );

    let _ = run_after_tool(&chain, &mut res, "same-args", "read").await;
    assert!(chain.take_pending_nudges().is_empty());
    match chain.after_iteration(&ctx).await {
        MiddlewareOutcome::StopTurn { middleware, reason } => {
            assert_eq!(middleware, "repeated-failure");
            assert!(reason.contains('3'));
            assert!(
                !reason.contains("repeated-failure"),
                "用户可见文本不得混入中间件内部标识: {reason}"
            );
        }
        other => panic!("期望 StopTurn，实际 {other:?}"),
    }
}

#[tokio::test]
async fn success_resets_generic_counter() {
    let chain = MiddlewareChain::p4(6);
    let ctx = ctx();
    let mut failed = result(
        "read",
        true,
        ToolStage::Persist,
        Some(ToolErrorClass::Internal),
    );
    let mut ok = result("read", false, ToolStage::Execute, None);

    let _ = run_after_tool(&chain, &mut failed, "same-args", "read").await;
    let _ = run_after_tool(&chain, &mut ok, "same-args", "read").await;
    let _ = run_after_tool(&chain, &mut failed, "same-args", "read").await;
    assert!(chain.take_pending_nudges().is_empty());
    assert_eq!(
        chain.after_iteration(&ctx).await,
        MiddlewareOutcome::Continue
    );
}

#[tokio::test]
async fn generic_counter_is_scoped_by_tool_name() {
    let chain = MiddlewareChain::p4(6);
    let mut read = result(
        "read",
        true,
        ToolStage::Persist,
        Some(ToolErrorClass::Internal),
    );
    let mut grep = result(
        "grep",
        true,
        ToolStage::Persist,
        Some(ToolErrorClass::Internal),
    );

    // 参数文本跨工具不唯一：两个工具用同一段参数各失败一次，不应合并成「重复调用」。
    let _ = run_after_tool(&chain, &mut read, "same-args", "read").await;
    let _ = run_after_tool(&chain, &mut grep, "same-args", "grep").await;
    assert!(chain.take_pending_nudges().is_empty());
    assert_eq!(
        chain.after_iteration(&ctx()).await,
        MiddlewareOutcome::Continue
    );
}

#[tokio::test]
async fn permission_denial_halts_on_first_failure() {
    let chain = MiddlewareChain::p4(6);
    let ctx = ctx();
    let mut res = result(
        "write",
        true,
        ToolStage::Gateway,
        Some(ToolErrorClass::Denied),
    );
    let _ = run_after_tool(&chain, &mut res, "fp", "write").await;
    match chain.after_iteration(&ctx).await {
        MiddlewareOutcome::StopTurn { middleware, reason } => {
            assert_eq!(middleware, "repeated-failure");
            assert!(reason.contains("write"));
            assert!(!reason.contains("repeated-failure"));
        }
        other => panic!("期望 StopTurn，实际 {other:?}"),
    }
}

#[tokio::test]
async fn cancelled_result_does_not_count_as_failure() {
    let chain = MiddlewareChain::p4(6);
    let ctx = ctx();
    let mut res = result(
        "read",
        true,
        ToolStage::Execute,
        Some(ToolErrorClass::Cancelled),
    );
    let _ = run_after_tool(&chain, &mut res, "fp", "read").await;
    assert!(chain.take_pending_nudges().is_empty());
    assert_eq!(
        chain.after_iteration(&ctx).await,
        MiddlewareOutcome::Continue
    );
}

#[tokio::test]
async fn captured_outcomes_records_call_metadata() {
    let chain = MiddlewareChain::p4(6);
    let mut res = result("grep", false, ToolStage::Execute, None);
    let _ = run_after_tool(&chain, &mut res, "fp", "grep").await;
    let captured = chain.captured_outcomes();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].call_id, "call-grep");
    assert_eq!(captured[0].tool_name, "grep");
    assert!(captured[0].success);
    assert_eq!(captured[0].arguments, "{}");
}

fn ladder_messages() -> Vec<ChatMessage> {
    let body = "z".repeat(4_000);
    vec![
        text_msg(MessageRole::System, "sys"),
        tool_msg("read", "c1", &body),
        tool_msg("read", "c2", &body),
        tool_msg("read", "c3", &body),
        text_msg(MessageRole::User, "current"),
    ]
}

#[tokio::test]
async fn before_model_microcompacts_only_older_tool_bodies() {
    let mut messages = ladder_messages();
    let chain = MiddlewareChain::p4(1);
    let ctx = ctx();
    let mut req = ModelRequestView {
        messages: &mut messages,
        context_window_tokens: Some(5_000),
        max_tokens: 1_000,
        is_wrap_up_call: false,
    };
    let out = chain.before_model(&mut req, &ctx).await.0;
    assert_eq!(out, MiddlewareOutcome::Continue);
    assert!(req.messages[1]
        .content
        .starts_with(CLEARED_PLACEHOLDER_PREFIX));
    assert!(req.messages[2]
        .content
        .starts_with(CLEARED_PLACEHOLDER_PREFIX));
    assert_eq!(req.messages[3].content, "z".repeat(4_000));
}

#[tokio::test]
async fn wrap_up_restore_only_runs_for_capped_call() {
    let mut messages = ladder_messages();
    let chain = MiddlewareChain::p4(1);
    let ctx = ctx();

    let mut req = ModelRequestView {
        messages: &mut messages,
        context_window_tokens: Some(5_000),
        max_tokens: 1_000,
        is_wrap_up_call: false,
    };
    let _ = chain.before_model(&mut req, &ctx).await;
    assert!(req.messages[1]
        .content
        .starts_with(CLEARED_PLACEHOLDER_PREFIX));
    assert!(req.messages[2]
        .content
        .starts_with(CLEARED_PLACEHOLDER_PREFIX));

    let mut wrap_req = ModelRequestView {
        messages: &mut messages,
        context_window_tokens: Some(20_000),
        max_tokens: 1_000,
        is_wrap_up_call: true,
    };
    let _ = chain.before_model(&mut wrap_req, &ctx).await;
    assert_eq!(wrap_req.messages[1].content, "z".repeat(4_000));
    assert_eq!(wrap_req.messages[2].content, "z".repeat(4_000));
}

#[tokio::test]
async fn hard_trim_evicts_oldest_when_still_over_budget() {
    let mut messages = ladder_messages();
    let chain = MiddlewareChain::p4(0);
    let ctx = ctx();
    let mut req = ModelRequestView {
        messages: &mut messages,
        context_window_tokens: Some(2_000),
        max_tokens: 1_000,
        is_wrap_up_call: false,
    };
    let _ = chain.before_model(&mut req, &ctx).await;
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].role, MessageRole::System);
    assert_eq!(req.messages[1].content, "current");
}
