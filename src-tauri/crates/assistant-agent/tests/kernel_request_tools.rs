#![cfg(feature = "test-doubles")]

use assistant_agent::kernel::ports::test_doubles::FixtureSource;
use assistant_agent::kernel::ports::{TurnInput, TurnInputSource};
use assistant_agent::kernel::request;
use assistant_providers::types::ToolSpec;
use assistant_storage::agent_modes::AgentModeRow;

fn spec(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: String::new(),
        input_schema: serde_json::json!({ "type": "object", "properties": {} }),
    }
}

fn mode(kind: &str) -> AgentModeRow {
    AgentModeRow {
        id: format!("mode-{kind}"),
        name: kind.to_string(),
        description: String::new(),
        kind: kind.to_string(),
        prompt: String::new(),
        model: None,
        temperature: None,
        max_tokens: None,
        reasoning_enabled: None,
        reasoning_effort: None,
        presentation_rank: 0,
        is_builtin: false,
        created_at: 0,
        updated_at: 0,
        tool_config_json: None,
    }
}

async fn input_with_mode(kind: &str) -> TurnInput {
    let source = FixtureSource::simple("conv-tools");
    let mut input = source
        .conversation_detail("conv-tools")
        .await
        .expect("fixture detail");
    input.mode = Some(mode(kind));
    input
}

#[tokio::test]
async fn agent_mode_injects_resolved_tools() {
    let input = input_with_mode("agent").await;
    let tools = vec![spec("read_file"), spec("write_file")];
    let req = request::build(&input, 1, Some(&tools))
        .await
        .expect("build");

    let injected = req.options.tools.expect("tools injected");
    assert_eq!(injected.len(), 2);
    assert_eq!(injected[0].name, "read_file");
    assert_eq!(injected[1].name, "write_file");
}

#[tokio::test]
async fn non_agent_modes_do_not_inject_tools() {
    for kind in ["chat", "default", "quick", "deep", "custom"] {
        let input = input_with_mode(kind).await;
        let tools = vec![spec("read_file")];
        let req = request::build(&input, 1, Some(&tools))
            .await
            .expect("build");
        assert!(req.options.tools.is_none(), "{kind} must not inject tools");
    }
}

#[tokio::test]
async fn empty_tool_list_degrades_to_no_tools() {
    let input = input_with_mode("agent").await;
    let tools: Vec<ToolSpec> = Vec::new();
    let req = request::build(&input, 1, Some(&tools))
        .await
        .expect("build");
    assert!(req.options.tools.is_none());
}

#[tokio::test]
async fn missing_tool_list_degrades_to_no_tools() {
    let input = input_with_mode("agent").await;
    let req = request::build(&input, 1, None).await.expect("build");
    assert!(req.options.tools.is_none());
}
