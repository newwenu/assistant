use dashmap::DashMap;
use std::sync::Arc;

use crate::definition::{ToolDefinition, ToolDescriptor};
use crate::r#trait::{Tool, ToolTier};

/// 一次注册的完整身份。`id` 是内部唯一标识（注册表键、白名单匹配、审批、审计、
/// 报错前缀），`name` 是发给模型的调用名（`ToolSpec.name`）。
///
/// 两者都由工具实现自身声明（`Tool::id` / `Tool::name`），注册处只登记不覆盖。
#[derive(Clone)]
pub struct RegisteredTool {
    pub id: String,
    pub name: String,
    pub tool: Arc<dyn Tool>,
}

pub struct ToolRegistry {
    tools: DashMap<String, RegisteredTool>,
    /// 模型可见名 → 内部 id。模型回传的调用名走这张表解析回工具。
    names: DashMap<String, String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: DashMap::new(),
            names: DashMap::new(),
        }
    }

    /// 以 `tool.id()` 为键注册，同时登记 `tool.name()` → id 映射。
    ///
    /// 同一 id 重复注册会覆盖前一个实现；旧 name 映射一并清除，避免留下悬空别名。
    /// 同一 name 被不同 id 占用时以最后一次注册为准，并记录告警。
    pub fn register(&self, tool: Arc<dyn Tool>) {
        let id = tool.id().to_string();
        let name = tool.name().to_string();
        self.names.retain(|_, mapped| mapped.as_str() != id);
        if let Some(existing) = self.names.get(&name) {
            if existing.value() != &id {
                log::warn!(
                    "tool name '{}' already maps to id '{}'; re-pointing to '{}'",
                    name,
                    existing.value(),
                    id
                );
            }
        }
        self.names.insert(name.clone(), id.clone());
        self.tools
            .insert(id.clone(), RegisteredTool { id, name, tool });
    }

    /// 按内部 id 查找。管理、诊断与 ToolBox 入口走这条路径。
    pub fn find(&self, id: &str) -> Option<RegisteredTool> {
        self.tools.get(id).map(|entry| entry.value().clone())
    }

    /// 按模型可见名查找。模型回传的调用名走这条路径。
    pub fn find_by_name(&self, name: &str) -> Option<RegisteredTool> {
        let id = self.names.get(name).map(|entry| entry.value().clone())?;
        self.find(&id)
    }

    /// ToolBox 入口按内部 id 查找（`os_tool_exec` 的 `tool` 参数即 id）。
    pub fn find_for_toolbox(&self, id: &str) -> Option<RegisteredTool> {
        self.find(id)
            .filter(|registered| registered.tool.audience().toolbox)
    }

    pub fn tool_ids(&self) -> Vec<String> {
        self.tools.iter().map(|entry| entry.key().clone()).collect()
    }

    pub async fn tool_definitions_for_tier(&self, max_tier: ToolTier) -> Vec<ToolDefinition> {
        let selected = {
            let mut collected: Vec<RegisteredTool> = self
                .tools
                .iter()
                .filter(|entry| {
                    let registered = entry.value();
                    registered.tool.tier() <= max_tier && registered.tool.audience().agent
                })
                .map(|entry| entry.value().clone())
                .collect();
            collected.sort_by(|left, right| left.id.cmp(&right.id));
            collected
        };

        let mut definitions = Vec::with_capacity(selected.len());
        for registered in selected {
            let description = match registered.tool.description().await {
                Ok(description) => description,
                Err(error) => {
                    log::warn!(
                        "tool '{}' description unavailable, falling back to empty: {}",
                        registered.id,
                        error
                    );
                    String::new()
                }
            };
            definitions.push(ToolDefinition::new(
                registered.id,
                registered.name,
                description,
                registered.tool.input_schema(),
            ));
        }
        definitions
    }

    /// 装配面板清单：全部 `agent` 受众工具（**不按层级过滤**），带 `tier` 供前端分组。
    ///
    /// 不过滤层级是有意的：面板要展示「超出层级上限、将被忽略」的已选工具，
    /// 由前端按 `max_tool_tier` 置灰，后端只提供事实。
    pub async fn agent_tool_descriptors(&self) -> Vec<ToolDescriptor> {
        let selected = {
            let mut collected: Vec<RegisteredTool> = self
                .tools
                .iter()
                .filter(|entry| entry.value().tool.audience().agent)
                .map(|entry| entry.value().clone())
                .collect();
            collected.sort_by(|left, right| left.id.cmp(&right.id));
            collected
        };

        let mut descriptors = Vec::with_capacity(selected.len());
        for registered in selected {
            let description = match registered.tool.description().await {
                Ok(description) => description,
                Err(error) => {
                    log::warn!(
                        "tool '{}' description unavailable, falling back to empty: {}",
                        registered.id,
                        error
                    );
                    String::new()
                }
            };
            descriptors.push(ToolDescriptor {
                id: registered.id.clone(),
                name: registered.name.clone(),
                description,
                tier: registered.tool.tier(),
            });
        }
        descriptors
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
    use crate::r#trait::{BehaviorHints, ToolAudience};
    use async_trait::async_trait;
    use serde_json::Value;

    struct StubTool {
        id: &'static str,
        name: &'static str,
        tier: ToolTier,
        description: Result<&'static str, &'static str>,
        audience: ToolAudience,
    }

    impl StubTool {
        fn new(tier: ToolTier, description: Result<&'static str, &'static str>) -> Self {
            Self {
                id: "stub",
                name: "stub",
                tier,
                description,
                audience: ToolAudience::BOTH,
            }
        }

        fn with_identity(mut self, id: &'static str, name: &'static str) -> Self {
            self.id = id;
            self.name = name;
            self
        }

        fn with_audience(mut self, audience: ToolAudience) -> Self {
            self.audience = audience;
            self
        }
    }

    #[async_trait]
    impl Tool for StubTool {
        fn id(&self) -> &str {
            self.id
        }

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

        fn audience(&self) -> ToolAudience {
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

    fn stub_named(id: &'static str, name: &'static str, tier: ToolTier) -> Arc<dyn Tool> {
        Arc::new(StubTool::new(tier, Ok("stub")).with_identity(id, name)) as Arc<dyn Tool>
    }

    fn registry_with(names: &[(&'static str, ToolTier)]) -> ToolRegistry {
        let registry = ToolRegistry::new();
        for (name, tier) in names {
            registry.register(stub_named(name, name, *tier));
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
        let ids: Vec<&str> = sandbox_defs.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["sandbox_tool"]);

        let os_defs = registry.tool_definitions_for_tier(ToolTier::Os).await;
        let ids: Vec<&str> = os_defs.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["os_tool", "sandbox_tool"]);

        let external_defs = registry.tool_definitions_for_tier(ToolTier::External).await;
        assert_eq!(external_defs.len(), 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn definitions_are_sorted_by_id() {
        let registry = registry_with(&[
            ("zeta", ToolTier::Os),
            ("alpha", ToolTier::Os),
            ("mu", ToolTier::Os),
        ]);

        let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
        let ids: Vec<&str> = definitions.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "mu", "zeta"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn description_failure_degrades_to_empty_string() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(
            StubTool::new(ToolTier::Os, Err("boom")).with_identity("broken", "broken"),
        ) as Arc<dyn Tool>);
        registry.register(Arc::new(
            StubTool::new(ToolTier::Os, Ok("ok")).with_identity("healthy", "healthy"),
        ) as Arc<dyn Tool>);

        let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
        assert_eq!(definitions.len(), 2);

        let broken = definitions.iter().find(|d| d.id == "broken").unwrap();
        assert!(broken.description.is_empty());

        let healthy = definitions.iter().find(|d| d.id == "healthy").unwrap();
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
        let registry = ToolRegistry::new();
        registry.register(stub_named("agent_visible", "agent_visible", ToolTier::Os));
        registry.register(Arc::new(
            StubTool::new(ToolTier::Os, Ok("ok"))
                .with_identity("toolbox_only", "toolbox_only")
                .with_audience(ToolAudience::TOOLBOX_ONLY),
        ) as Arc<dyn Tool>);

        let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
        let ids: Vec<&str> = definitions.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["agent_visible"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn model_name_is_separate_from_id() {
        let registry = ToolRegistry::new();
        registry.register(stub_named("internal_read", "read", ToolTier::Os));

        assert!(registry.find("internal_read").is_some());
        assert!(registry.find("read").is_none());

        assert!(registry.find_by_name("read").is_some());
        assert!(registry.find_by_name("internal_read").is_none());

        assert!(registry.find_for_toolbox("internal_read").is_some());
        assert!(registry.find_for_toolbox("read").is_none());

        let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].id, "internal_read");
        assert_eq!(definitions[0].name, "read");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn default_model_name_equals_id() {
        let registry = registry_with(&[("read", ToolTier::Os)]);
        let definitions = registry.tool_definitions_for_tier(ToolTier::Os).await;
        assert_eq!(definitions[0].id, "read");
        assert_eq!(definitions[0].name, "read");
    }

    #[test]
    fn re_registering_same_id_drops_stale_name_alias() {
        let registry = ToolRegistry::new();
        registry.register(stub_named("t", "old", ToolTier::Os));
        registry.register(stub_named("t", "new", ToolTier::Os));

        assert!(registry.find_by_name("old").is_none());
        assert!(registry.find_by_name("new").is_some());
        assert!(registry.find("t").is_some());
    }

    #[test]
    fn find_for_toolbox_rejects_toolbox_disabled_tools() {
        let registry = ToolRegistry::new();
        registry.register(stub_named("both", "both", ToolTier::Os));
        registry.register(Arc::new(
            StubTool::new(ToolTier::Os, Ok("ok"))
                .with_identity("agent_only", "agent_only")
                .with_audience(ToolAudience::AGENT_ONLY),
        ) as Arc<dyn Tool>);

        assert!(registry.find_for_toolbox("both").is_some());
        assert!(registry.find_for_toolbox("agent_only").is_none());
        assert!(registry.find_for_toolbox("missing").is_none());

        assert!(registry.find("agent_only").is_some());
    }

    #[test]
    fn default_audience_is_fail_closed() {
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
