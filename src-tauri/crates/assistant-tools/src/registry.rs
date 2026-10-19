use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;

use crate::r#trait::Tool;

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

    pub fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn tool_definitions_for_tier(&self, tier: crate::r#trait::ToolTier) -> Vec<Value> {
        self.tools
            .iter()
            .filter(|entry| entry.value().tier() == tier)
            .map(|entry| {
                serde_json::json!({
                    "name": entry.key(),
                    "description": "",
                    "inputSchema": entry.value().input_schema(),
                })
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
