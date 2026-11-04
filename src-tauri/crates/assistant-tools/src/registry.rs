use dashmap::DashMap;
use std::sync::Arc;

use crate::definition::ToolDefinition;
use crate::r#trait::{Tool, ToolTier};

pub struct ToolRegistry {
    tools: DashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: DashMap::new(),
        }
    }

    pub fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn find(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).map(|entry| Arc::clone(entry.value()))
    }

    pub fn find_for_toolbox(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.find(name).filter(|tool| tool.audience().toolbox)
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|entry| entry.key().clone()).collect()
    }

    pub async fn tool_definitions_for_tier(&self, max_tier: ToolTier) -> Vec<ToolDefinition> {
        let selected = {
            let mut collected: Vec<(String, Arc<dyn Tool>)> = self
                .tools
                .iter()
                .filter(|entry| {
                    let tool = entry.value();
                    tool.tier() <= max_tier && tool.audience().agent
                })
                .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
                .collect();
            collected.sort_by(|left, right| left.0.cmp(&right.0));
            collected
        };

        let mut definitions = Vec::with_capacity(selected.len());
        for (name, tool) in selected {
            let description = match tool.description().await {
                Ok(description) => description,
                Err(error) => {
                    log::warn!(
                        "tool '{}' description unavailable, falling back to empty: {}",
                        name,
                        error
                    );
                    String::new()
                }
            };
            definitions.push(ToolDefinition::new(name, description, tool.input_schema()));
        }
        definitions
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#trait::BehaviorHints;
    use async_trait::async_trait;
    use serde_json::Value;

    struct StubTool {
        name: &'static str,
        tier: ToolTier,
        description: Result<&'static str, &'static str>,
        audience: crate::r#trait::ToolAudience,
    }

    impl StubTool {
        fn new(
            name: &'static str,
            tier: ToolTier,
            description: Result<&'static str, &'static str>,
        ) -> Self {
            Self {
                name,
                tier,
                description,
                audience: crate::r#trait::ToolAudience::BOTH,
            }
        }

        fn with_audience(mut self, audience: crate::r#trait::ToolAudience) -> Self {
            self.audience = audience;
            self
        }
    }

    #[async_trait]
    impl Tool for StubTool {
        fn name(&self) -> &str {
            self.name
        }

        async fn description(&self) -> Result<String, String> {
            self.description.map(str::to_string).map_err(str::to_string)
        }

        fn input_schema(&self) -> Value {
            serde_json::json!({ "type": "object" })
        }

        fn tier(&self) -> ToolTier {
            self.tier
        }

        fn audience(&self) -> crate::r#trait::ToolAudience {
            self.audience
        }

        fn behavior_hints(&self) -> BehaviorHints {
            BehaviorHints::default()
        }

        async fn execute(
            &self,
            _input: &Value,
            _context: &crate::context::ToolUseContext,
        ) -> Result<crate::result::ToolResult, String> {
            Ok(crate::result::ToolResult::ok(serde_json::json!({})))
        }
    }

    fn registry_with(names: &[(&'static str, ToolTier)]) -> ToolRegistry {
        let registry = ToolRegistry::new();
        for (name, tier) in names {
            registry.register(Arc::new(StubTool::new(name, *tier, Ok("stub"))) as Arc<dyn Tool>);
        }
        registry
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn definitions_include_lower_tiers() {
        let registry = registry_with(&[
            ("sandbox_tool", ToolTier::Sandbox),
            ("os_tool", ToolTier::Os),
            ("external_tool", ToolTier::External),
        ]);

        let sandbox_defs = registry.tool_definitions_for_tier(ToolTier::Sandbox).await;
        let names: Vec<&str> = sandbox_defs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["sandbox_tool"]);

        let os_defs = registry.tool_definitions_for_tier(ToolTier::Os).await;
        let names: Vec<&str> = os_defs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["os_tool", "sandbox_tool"]);

        let external_defs = registry.tool_definitions_for_tier(ToolTier::External).await;
        assert_eq!(external_defs.len(), 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn definitions_are_sorted_by_name() {
        let registry = registry_with(&[
            ("zeta", ToolTier::Os),
            ("alpha", ToolTier::Os),
            ("mu", ToolTier::Os),
        ]);

        let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
        let names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mu", "zeta"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn description_failure_degrades_to_empty_string() {
        let registry = ToolRegistry::new();
        registry.register(
            Arc::new(StubTool::new("broken", ToolTier::Os, Err("boom"))) as Arc<dyn Tool>
        );
        registry
            .register(Arc::new(StubTool::new("healthy", ToolTier::Os, Ok("ok"))) as Arc<dyn Tool>);

        let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
        assert_eq!(definitions.len(), 2);

        let broken = definitions.iter().find(|d| d.name == "broken").unwrap();
        assert!(broken.description.is_empty());

        let healthy = definitions.iter().find(|d| d.name == "healthy").unwrap();
        assert_eq!(healthy.description, "ok");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn definition_serializes_with_camel_case() {
        let registry = registry_with(&[("only", ToolTier::Os)]);
        let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
        let value = serde_json::to_value(&definitions[0]).unwrap();
        assert!(value.get("inputSchema").is_some());
        assert!(value.get("input_schema").is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toolbox_only_tool_is_excluded_from_agent_definitions() {
        use crate::r#trait::ToolAudience;

        let registry = ToolRegistry::new();
        registry.register(
            Arc::new(StubTool::new("agent_visible", ToolTier::Os, Ok("ok"))) as Arc<dyn Tool>,
        );
        registry.register(Arc::new(
            StubTool::new("toolbox_only", ToolTier::Os, Ok("ok"))
                .with_audience(ToolAudience::TOOLBOX_ONLY),
        ) as Arc<dyn Tool>);

        let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
        let names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["agent_visible"]);
    }

    #[test]
    fn find_for_toolbox_rejects_toolbox_disabled_tools() {
        use crate::r#trait::ToolAudience;

        let registry = ToolRegistry::new();
        registry.register(Arc::new(StubTool::new("both", ToolTier::Os, Ok("ok"))) as Arc<dyn Tool>);
        registry.register(Arc::new(
            StubTool::new("agent_only", ToolTier::Os, Ok("ok"))
                .with_audience(ToolAudience::AGENT_ONLY),
        ) as Arc<dyn Tool>);

        assert!(registry.find_for_toolbox("both").is_some());
        assert!(registry.find_for_toolbox("agent_only").is_none());
        assert!(registry.find_for_toolbox("missing").is_none());

        assert!(registry.find("agent_only").is_some());
    }

    #[test]
    fn default_audience_is_fail_closed() {
        use crate::r#trait::ToolAudience;

        assert_eq!(ToolAudience::default(), ToolAudience::NONE);
        assert_eq!(
            (ToolAudience::NONE.agent, ToolAudience::NONE.toolbox),
            (false, false)
        );
        assert_eq!(
            (ToolAudience::BOTH.agent, ToolAudience::BOTH.toolbox),
            (true, true)
        );
        assert_eq!(
            (
                ToolAudience::TOOLBOX_ONLY.agent,
                ToolAudience::TOOLBOX_ONLY.toolbox
            ),
            (false, true)
        );
        assert_eq!(
            (
                ToolAudience::AGENT_ONLY.agent,
                ToolAudience::AGENT_ONLY.toolbox
            ),
            (true, false)
        );
    }
}
