use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::context::ToolUseContext;
use crate::r#trait::{BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier};
use crate::result::ToolResult;

use super::call::{guarded_call, map_outcome, McpCaller};
use super::naming::{build_tool_names, McpToolName};

/// MCP `tools/list` 的 `annotations`。
///
/// 三个字段全部是**自报**，只能升级严格程度（规格 §3.3）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct McpAnnotations {
    pub read_only_hint: Option<bool>,
    pub destructive_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
}

impl McpAnnotations {
    /// 保守推断：无声明即视为破坏性 + 开放世界；`readOnlyHint` 可采纳但不自动放行。
    pub fn behavior_hints(&self) -> BehaviorHints {
        let read_only = self.read_only_hint.unwrap_or(false);
        let destructive = self.destructive_hint.unwrap_or(false) || !read_only;
        let open_world = self.open_world_hint.unwrap_or(false) || !read_only;
        BehaviorHints {
            read_only,
            destructive,
            open_world,
        }
    }
}

/// `tools/list` 返回的单个工具定义（快照，重连时整体替换）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDef {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Option<Value>,
    #[serde(default)]
    pub annotations: McpAnnotations,
}

/// 一个 MCP 工具在 Registry 中的形态（规格 §3.2）。
pub struct McpTool {
    name: McpToolName,
    pub(super) description: String,
    input_schema: Value,
    behavior: BehaviorHints,
    config_hash: String,
    caller: Arc<dyn McpCaller>,
}

impl McpTool {
    pub fn new(
        name: McpToolName,
        def: &McpToolDef,
        config_hash: impl Into<String>,
        caller: Arc<dyn McpCaller>,
    ) -> Self {
        Self {
            name,
            description: def.description.clone().unwrap_or_default(),
            input_schema: def
                .input_schema
                .clone()
                .unwrap_or_else(|| json!({ "type": "object" })),
            behavior: def.annotations.behavior_hints(),
            config_hash: config_hash.into(),
            caller,
        }
    }

    pub fn tool_name(&self) -> &McpToolName {
        &self.name
    }

    pub fn config_hash(&self) -> &str {
        &self.config_hash
    }
}

/// 按 `tools/list` 快照批量构造工具，供 C3 在 `Ready` 后一次性注册。
pub fn build_tools(
    server_id: &str,
    server_display_name: &str,
    config_hash: &str,
    defs: &[McpToolDef],
    caller: Arc<dyn McpCaller>,
) -> Vec<Arc<McpTool>> {
    let names = build_tool_names(server_id, server_display_name, defs);
    defs.iter()
        .zip(names)
        .map(|(def, name)| Arc::new(McpTool::new(name, def, config_hash, caller.clone())))
        .collect()
}

#[async_trait]
impl Tool for McpTool {
    fn id(&self) -> &str {
        &self.name.full
    }

    async fn description(&self) -> Result<String, String> {
        Ok(self.description.clone())
    }

    fn input_schema(&self) -> Value {
        self.input_schema.clone()
    }

    fn tier(&self) -> ToolTier {
        ToolTier::External
    }

    fn audience(&self) -> ToolAudience {
        ToolAudience::AGENT_ONLY
    }

    fn behavior_hints(&self) -> BehaviorHints {
        self.behavior
    }

    fn permission_intents(
        &self,
        _input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        Ok(vec![PermissionIntent::new(
            "mcp",
            format!("mcp:{}:{}", self.name.server_id, self.config_hash),
        )])
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        if !input.is_object() {
            return Ok(ToolResult::err("input must be a JSON object"));
        }
        Ok(map_outcome(
            guarded_call(
                self.caller.as_ref(),
                &self.name.original,
                input,
                context.cancel.as_ref(),
            )
            .await,
        ))
    }
}
