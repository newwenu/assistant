use assistant_agent::kernel::limits::{
    derived_tool_call_limit, effective_tool_call_limit, DEFAULT_MAX_ITERATIONS,
    DEFAULT_MICROCOMPACT_KEEP_RECENT, DEFAULT_MODEL_CALL_TIMEOUT, DEFAULT_TURN_WALL_CLOCK,
};
use assistant_agent::kernel::{
    EventRoute, LadderPolicy, OnBusy, SessionState, TurnLimits, TurnPhase, TurnSpec, TurnStatus,
};
use std::path::Path;
use std::time::Duration;

#[test]
fn chat_spec_field_values() {
    let spec = TurnSpec::chat();
    assert_eq!(spec.on_busy, OnBusy::Replace);
    assert_eq!(spec.max_iterations, 1);
    assert!(spec.tools.is_none());
    assert_eq!(spec.ladder, LadderPolicy::ProjectionOnly);
    assert!(spec.stop_hooks.is_empty());
    assert!(!spec.wrap_up);
    assert_eq!(spec.route, EventRoute::Orb);
    assert_eq!(spec.kind, "chat");
    assert_eq!(spec.limits, TurnLimits::chat());
}

#[test]
fn chat_limits_keep_existing_chat_behaviour() {
    let limits = TurnLimits::chat();
    assert_eq!(limits.max_iterations, 1);
    assert_eq!(limits.turn_wall_clock, None);
    assert_eq!(limits.model_call, None);
    assert_eq!(limits.empty_response_retries, 0);
    assert_eq!(limits.microcompact_keep_recent, 0);
}

#[test]
fn agent_spec_field_values() {
    let spec = TurnSpec::agent();
    assert_eq!(spec.on_busy, OnBusy::Reject);
    assert_eq!(spec.max_iterations, 12);
    assert!(spec.tools.is_none());
    assert_eq!(spec.ladder, LadderPolicy::Full);
    let hook_names: Vec<&str> = spec.stop_hooks.iter().map(|h| h.name()).collect();
    assert_eq!(
        hook_names,
        vec!["iteration-limit", "tool-call-limit", "turn-wall-clock"]
    );
    assert!(spec.wrap_up);
    assert_eq!(spec.route, EventRoute::Broadcast);
    assert_eq!(spec.kind, "agent");
    assert_eq!(spec.limits, TurnLimits::default());
}

#[test]
fn default_limits_match_spec_table() {
    let limits = TurnLimits::default();
    assert_eq!(limits.max_iterations, DEFAULT_MAX_ITERATIONS);
    assert_eq!(limits.max_iterations, 12);
    assert_eq!(limits.max_tool_calls, derived_tool_call_limit(12));
    assert_eq!(limits.max_tool_calls, 96);
    assert_eq!(limits.turn_wall_clock, Some(DEFAULT_TURN_WALL_CLOCK));
    assert_eq!(limits.turn_wall_clock, Some(Duration::from_secs(30 * 60)));
    assert_eq!(limits.model_call, Some(DEFAULT_MODEL_CALL_TIMEOUT));
    assert_eq!(limits.model_call, Some(Duration::from_secs(10 * 60)));
    assert_eq!(limits.empty_response_retries, 1);
    assert_eq!(
        limits.microcompact_keep_recent,
        DEFAULT_MICROCOMPACT_KEEP_RECENT
    );
    assert_eq!(limits.microcompact_keep_recent, 6);
}

#[test]
fn derived_tool_call_limit_has_floor_of_eight() {
    assert_eq!(derived_tool_call_limit(0), 8);
    assert_eq!(derived_tool_call_limit(1), 8);
    assert_eq!(derived_tool_call_limit(2), 16);
    assert_eq!(derived_tool_call_limit(12), 96);
    assert_eq!(derived_tool_call_limit(u32::MAX), u32::MAX);
}

#[test]
fn effective_tool_call_limit_only_tightens() {
    assert_eq!(effective_tool_call_limit(96, None), 96);
    assert_eq!(effective_tool_call_limit(96, Some(96)), 96);
    assert_eq!(effective_tool_call_limit(96, Some(8)), 8);
    assert_eq!(effective_tool_call_limit(8, Some(96)), 8);
    assert_eq!(effective_tool_call_limit(8, Some(0)), 0);
}

#[test]
fn turn_phase_serializes_as_camel_case() {
    let cases = [
        (TurnPhase::Starting, "\"starting\""),
        (TurnPhase::Compacting, "\"compacting\""),
        (TurnPhase::Thinking, "\"thinking\""),
        (TurnPhase::Streaming, "\"streaming\""),
        (TurnPhase::ToolCalling, "\"toolCalling\""),
        (TurnPhase::AwaitingApproval, "\"awaitingApproval\""),
        (TurnPhase::Finalizing, "\"finalizing\""),
    ];
    for (phase, expected) in cases {
        let json = serde_json::to_string(&phase).expect("serialize phase");
        assert_eq!(json, expected);
    }
}

#[test]
fn session_state_serializes_as_camel_case() {
    let idle = serde_json::to_value(SessionState::Idle).expect("serialize idle");
    assert_eq!(idle, serde_json::json!("idle"));

    let processing = SessionState::Processing {
        turn_id: "conv:1".to_string(),
        phase: TurnPhase::ToolCalling,
    };
    let value = serde_json::to_value(&processing).expect("serialize processing");
    assert_eq!(
        value,
        serde_json::json!({
            "processing": { "turnId": "conv:1", "phase": "toolCalling" }
        })
    );

    let error = SessionState::Error {
        message: "boom".to_string(),
        recoverable: false,
    };
    let value = serde_json::to_value(&error).expect("serialize error");
    assert_eq!(
        value,
        serde_json::json!({
            "error": { "message": "boom", "recoverable": false }
        })
    );
}

#[test]
fn turn_status_serializes_as_camel_case() {
    let cases = [
        (TurnStatus::Completed, "\"completed\""),
        (TurnStatus::Cancelled, "\"cancelled\""),
        (TurnStatus::Failed, "\"failed\""),
        (TurnStatus::Interrupted, "\"interrupted\""),
    ];
    for (status, expected) in cases {
        let json = serde_json::to_string(&status).expect("serialize status");
        assert_eq!(json, expected);
    }
}

fn collect_kernel_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("read kernel dir");
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_kernel_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn kernel_sources_have_no_branch_on_agent_kind() {
    let kernel_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("kernel");
    let mut sources = Vec::new();
    collect_kernel_sources(&kernel_dir, &mut sources);
    assert!(!sources.is_empty(), "kernel sources not found");

    let forbidden = ["is_agent", "is_chat"];
    for path in sources {
        let text = std::fs::read_to_string(&path).expect("read kernel source");
        for needle in forbidden {
            assert!(
                !text.contains(needle),
                "{} contains forbidden token {}",
                path.display(),
                needle
            );
        }
    }
}

/// I36b H7：未接线字段显式登记，防止「字段存在即视为已实现」的误判。
///
/// `TurnSpec` 的 `ladder` / `wrap_up` / `stop_hooks` 目前只有定义与默认值，
/// 内核没有任何读取点（P4 才接线）。一旦有人接线，本用例会失败并要求同步更新清单。
#[test]
fn unwired_turn_spec_fields_have_no_read_points_yet() {
    let kernel_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("kernel");
    let mut sources = Vec::new();
    collect_kernel_sources(&kernel_dir, &mut sources);
    assert!(!sources.is_empty(), "kernel sources not found");

    let unwired = ["spec.ladder", "spec.wrap_up", "spec.stop_hooks"];
    for path in sources {
        let text = std::fs::read_to_string(&path).expect("read kernel source");
        for needle in unwired {
            assert!(
                !text.contains(needle),
                "{} 已读取 {}，请同步更新 I36b H7 未接线清单",
                path.display(),
                needle
            );
        }
    }
}
