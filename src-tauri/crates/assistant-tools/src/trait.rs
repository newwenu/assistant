use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::context::ToolUseContext;
use crate::result::ToolResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolTier {
    Sandbox,
    Os,
    External,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BehaviorHints {
    pub read_only: bool,
    pub destructive: bool,
    pub open_world: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionIntent {
    pub action: String,
    pub resources: Vec<String>,
}

impl PermissionIntent {
    pub fn new(action: impl Into<String>, resource: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            resources: vec![resource.into()],
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;

    async fn description(&self) -> Result<String, String>;

    fn input_schema(&self) -> Value;

    fn tier(&self) -> ToolTier;

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints::default()
    }

    fn is_readonly(&self) -> bool {
        self.behavior_hints().read_only
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        Ok(vec![])
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String>;
}