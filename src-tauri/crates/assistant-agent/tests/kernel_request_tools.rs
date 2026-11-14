#![cfg(feature = "test-doubles")]

mod common;

use assistant_agent::kernel::ports::test_doubles::FixtureSource;
use assistant_agent::kernel::ports::{TurnInput, TurnInputSource};
use assistant_agent::kernel::request;
use assistant_agent::prompt_builder::PromptFacts;
use assistant_providers::base::MessageRole;
use assistant_providers::types::ToolSpec;
use common::chat::tool_spec;
use common::modes::mode_row;

fn spec(name: &str) -> ToolSpec {
    tool_spec(
        name,
        "",
        serde_json::json!({ "type": "object", "properties": {} }),
    )
}

async fn input_with_mode(kind: &str) -> TurnInput {
    let source = FixtureSource::simple("conv-tools");
    let mut input = source
        .conversation_detail("conv-tools")
        .await
        .expect("fixture detail");
    input.mode = Some(mode_row(kind));
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

fn facts(working_directory: Option<&str>) -> PromptFacts {
    PromptFacts {
        working_directory: working_directory.map(|p| p.to_string()),
    }
}

#[tokio::test]
async fn agent_mode_injects_workspace_fact_into_system_message() {
    let mut input = input_with_mode("agent").await;
    input.prompt_facts = facts(Some("G:/ws"));

    let req = request::build(&input, 1, None).await.expect("build");

    let system = req.chat_messages.first().expect("system message inserted");
    assert!(matches!(system.role, MessageRole::System));
    assert_eq!(system.content, "Working directory: G:/ws");
    assert_eq!(
        req.chat_messages
            .iter()
            .filter(|m| m.content.contains("Working directory:"))
            .count(),
        1,
        "fact line must appear exactly once"
    );
}

#[tokio::test]
async fn chat_mode_does_not_inject_workspace_fact() {
    for kind in ["chat", "default", "quick", "deep", "custom"] {
        let mut input = input_with_mode(kind).await;
        input.prompt_facts = facts(Some("G:/ws"));

        let req = request::build(&input, 1, None).await.expect("build");

        assert!(
            req.chat_messages
                .iter()
                .all(|m| !m.content.contains("Working directory:")),
            "{kind} must not see the workspace fact"
        );
        assert!(req
            .chat_messages
            .iter()
            .all(|m| !matches!(m.role, MessageRole::System)));
    }
}

#[tokio::test]
async fn unset_workspace_keeps_system_message_absent() {
    let mut input = input_with_mode("agent").await;
    input.prompt_facts = PromptFacts::default();

    let req = request::build(&input, 1, None).await.expect("build");

    assert!(req
        .chat_messages
        .iter()
        .all(|m| !matches!(m.role, MessageRole::System)));
}
