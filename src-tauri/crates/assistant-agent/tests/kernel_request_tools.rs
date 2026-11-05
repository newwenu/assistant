#![cfg(feature = "test-doubles")]

use std::sync::Arc;

use assistant_agent::kernel::ports::test_doubles::FixtureSource;
use assistant_agent::kernel::ports::{ToolCatalog, TurnInput, TurnInputSource};
use assistant_agent::kernel::request;
use assistant_providers::types::ToolSpec;
use assistant_storage::agent_modes::AgentModeRow;
use async_trait::async_trait;

struct StaticCatalog(Vec<ToolSpec>);

#[async_trait]
impl ToolCatalog for StaticCatalog {
    async fn agent_tools(&self) -> Vec<ToolSpec> {
        self.0.clone()
    }
}

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
        tools_json: "[]".to_string(),
        presentation_rank: 0,
        is_builtin: false,
        created_at: 0,
        updated_at: 0,
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

fn catalog(tools: Vec<ToolSpec>) -> Arc<dyn ToolCatalog> {
    Arc::new(StaticCatalog(tools))
}

#[tokio::test]
async fn custom_mode_injects_catalog_tools() {
    let input = input_with_mode("custom").await;
    let catalog = catalog(vec![spec("read_file"), spec("write_file")]);
    let req = request::build(&input, 1, Some(&catalog))
        .await
        .expect("build");

    let tools = req.options.tools.expect("tools injected");
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "read_file");
    assert_eq!(tools[1].name, "write_file");
}

#[tokio::test]
async fn non_custom_modes_do_not_inject_tools() {
    for kind in ["default", "quick", "deep"] {
        let input = input_with_mode(kind).await;
        let catalog = catalog(vec![spec("read_file")]);
        let req = request::build(&input, 1, Some(&catalog))
            .await
            .expect("build");
        assert!(req.options.tools.is_none(), "{kind} must not inject tools");
    }
}

#[tokio::test]
async fn empty_catalog_degrades_to_no_tools() {
    let input = input_with_mode("custom").await;
    let catalog = catalog(Vec::new());
    let req = request::build(&input, 1, Some(&catalog))
        .await
        .expect("build");
    assert!(req.options.tools.is_none());
}

#[tokio::test]
async fn missing_catalog_degrades_to_no_tools() {
    let input = input_with_mode("custom").await;
    let req = request::build(&input, 1, None).await.expect("build");
    assert!(req.options.tools.is_none());
}
