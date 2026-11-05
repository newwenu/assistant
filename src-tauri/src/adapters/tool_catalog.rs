use std::sync::Arc;

use assistant_agent::kernel::ToolCatalog;
use assistant_providers::types::ToolSpec;
use assistant_tools::{ToolDefinition, ToolRegistry, ToolTier};
use async_trait::async_trait;

/// 注入给模型的工具上限档位。`External` 不在 P4 范围内，恒不注入。
const MAX_TIER: ToolTier = ToolTier::Os;

pub struct RegistryToolCatalog {
    registry: Arc<ToolRegistry>,
}

impl RegistryToolCatalog {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ToolCatalog for RegistryToolCatalog {
    async fn agent_tools(&self) -> Vec<ToolSpec> {
        self.registry
            .tool_definitions_for_tier(MAX_TIER)
            .await
            .into_iter()
            .map(into_tool_spec)
            .collect()
    }
}

fn into_tool_spec(definition: ToolDefinition) -> ToolSpec {
    ToolSpec {
        name: definition.name,
        description: definition.description,
        input_schema: definition.input_schema,
    }
}
