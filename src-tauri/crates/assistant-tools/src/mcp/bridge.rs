//! MCP 外部工具桥接层（P7-C1）。
//!
//! 本模块只回答「MCP 工具如何变成一个 `Tool`」：命名、安全标记推断、结果映射、
//! 精确审批键所需的身份信息。不做 IO、不持有子进程、不碰 `ToolRegistry`。
//!
//! 规格：[`docs/store/I38e-mcp-tools-spec.md`](../../../../../docs/store/I38e-mcp-tools-spec.md) §3。

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use async_trait::async_trait;
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::context::ToolUseContext;
use crate::r#trait::{BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier};
use crate::result::{ToolErrorKind, ToolResult};

/// 模型可见名的固定前缀（规格 §3.1）。
pub const TOOL_PREFIX: &str = "mcp";
const NAME_SEPARATOR: &str = "__";
const SHORT_HASH_LEN: usize = 8;

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

/// 一个 MCP 工具的三层名字。
///
/// `full` 是模型可见名（稳定标识，展示名不参与）；`original` 是发给 server 的原始名；
/// `display` 只给 UI 用，可随 server 展示名变化。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolName {
    pub full: String,
    pub server_id: String,
    pub tool: String,
    pub original: String,
    pub display: String,
}

/// 段规范化：非 `[a-zA-Z0-9_-]` 替换为 `_`；空段回退 `_`（规格 §3.1）。
pub fn normalize_segment(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if replaced.is_empty() {
        "_".to_string()
    } else {
        replaced
    }
}

/// 为一次 `tools/list` 快照生成工具名。
///
/// 顺序与 `defs` 一致；同一 server 内规范化后重名的，追加原始名的短哈希
/// （8 位 hex，`_` 分隔），避免两个工具共用同一个模型可见名。
pub fn build_tool_names(
    server_id: &str,
    server_display_name: &str,
    defs: &[McpToolDef],
) -> Vec<McpToolName> {
    let normalized_server = normalize_segment(server_id);
    let display_server = if server_display_name.trim().is_empty() {
        server_id
    } else {
        server_display_name.trim()
    };

    let normalized: Vec<String> = defs
        .iter()
        .map(|def| normalize_segment(&def.name))
        .collect();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for name in &normalized {
        *counts.entry(name.clone()).or_default() += 1;
    }

    defs.iter()
        .zip(&normalized)
        .map(|(def, base)| {
            let tool = if counts.get(base).copied().unwrap_or(0) > 1 {
                format!("{}_{}", base, short_hash(&def.name))
            } else {
                base.clone()
            };
            McpToolName {
                full: format!(
                    "{TOOL_PREFIX}{NAME_SEPARATOR}{normalized_server}{NAME_SEPARATOR}{tool}"
                ),
                server_id: normalized_server.clone(),
                tool,
                original: def.name.clone(),
                display: format!("{display_server} / {}", def.name),
            }
        })
        .collect()
}

fn short_hash(raw: &str) -> String {
    let digest = hex::encode(Sha256::digest(raw.as_bytes()));
    digest[..SHORT_HASH_LEN].to_string()
}

/// 一次 `tools/call` 的归一化结局。
#[derive(Debug, Clone, PartialEq)]
pub enum McpCallOutcome {
    /// server 正常应答（`isError` 由 MCP 侧声明）。
    Content { blocks: Vec<Value>, is_error: bool },
    /// server 不认识这个工具（`tools/list` 之后被下线）。
    ToolNotFound,
    /// 进程已退出 / 管道断开。
    ServerUnavailable,
    /// 客户端本地超时。`exec::invoke` 的统一 deadline 是另一层，两层都归 `Timeout`。
    Timeout,
    /// turn 取消（用户停止）：只中止本次请求，进程与工具保留（规格 §4.3）。
    Cancelled,
    /// 协议错误、非法帧等客户端内部失败。
    Internal { message: String },
}

/// `McpTool` 的传输抽象：C1 只定义契约，C2 用 stdio 实现。
#[async_trait]
pub trait McpCaller: Send + Sync {
    /// `tool` 是**原始** MCP 工具名（非规范化名）。
    ///
    /// `cancel` 来自 `ToolUseContext.cancel`；`None` 表示本次调用不响应取消。
    async fn call_tool(
        &self,
        tool: &str,
        arguments: &Value,
        cancel: Option<&CancellationToken>,
    ) -> McpCallOutcome;
}

/// 调用边界：把传输实现的 panic 收敛成一次失败调用（规格 §4.4 硬约束三）。
///
/// 只在 unwind 构建下有效；release 是 `panic = "abort"`，那层防线靠
/// 「客户端解析零 panic」与「读取任务自带收敛」两条硬约束。
pub async fn guarded_call(
    caller: &dyn McpCaller,
    tool: &str,
    arguments: &Value,
    cancel: Option<&CancellationToken>,
) -> McpCallOutcome {
    match AssertUnwindSafe(caller.call_tool(tool, arguments, cancel))
        .catch_unwind()
        .await
    {
        Ok(outcome) => outcome,
        Err(_) => McpCallOutcome::Internal {
            message: "MCP client panicked while handling the call".to_string(),
        },
    }
}

/// 一个 MCP 工具在 Registry 中的形态（规格 §3.2）。
pub struct McpTool {
    name: McpToolName,
    description: String,
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

fn map_outcome(outcome: McpCallOutcome) -> ToolResult {
    match outcome {
        McpCallOutcome::Content { blocks, is_error } => map_content(blocks, is_error),
        McpCallOutcome::ToolNotFound => ToolResult::err("MCP tool is not registered on the server"),
        McpCallOutcome::ServerUnavailable => {
            ToolResult::err_with_kind(ToolErrorKind::Internal, "MCP server is unavailable")
        }
        McpCallOutcome::Timeout => {
            ToolResult::err_with_kind(ToolErrorKind::Timeout, "MCP call timed out")
        }
        McpCallOutcome::Cancelled => {
            ToolResult::err_with_kind(ToolErrorKind::Cancelled, "MCP call was cancelled")
        }
        McpCallOutcome::Internal { message } => {
            ToolResult::err_with_kind(ToolErrorKind::Internal, message)
        }
    }
}

/// 内容块 → `ToolResult`。
///
/// 只有文本块进入 `result_for_assistant`；图片 / 资源块原样留在 `data`，
/// 避免 base64 负载被当成模型文本回填。无文本块时给计数摘要，
/// 禁止回退到 `data` 序列化（`assistant_text` 的兜底路径会把整个负载写进模型通道）。
fn map_content(blocks: Vec<Value>, is_error: bool) -> ToolResult {
    let text = blocks
        .iter()
        .filter_map(text_block)
        .collect::<Vec<_>>()
        .join("\n");
    let data = Value::Array(blocks.clone());

    if is_error {
        let message = if text.is_empty() {
            "MCP tool returned an error".to_string()
        } else {
            text
        };
        return ToolResult {
            ok: false,
            data: Some(data),
            result_for_assistant: None,
            error: Some(message),
            error_kind: Some(ToolErrorKind::Business),
        };
    }

    let assistant = if text.is_empty() {
        format!(
            "MCP tool returned {} content block(s) without text",
            blocks.len()
        )
    } else {
        text
    };
    ToolResult {
        ok: true,
        data: Some(data),
        result_for_assistant: Some(assistant),
        error: None,
        error_kind: None,
    }
}

fn text_block(block: &Value) -> Option<&str> {
    if block.get("type").and_then(Value::as_str) == Some("text") {
        block.get("text").and_then(Value::as_str)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct StubCaller {
        outcome: McpCallOutcome,
        seen: Mutex<Vec<(String, Value)>>,
    }

    impl StubCaller {
        fn new(outcome: McpCallOutcome) -> Arc<Self> {
            Arc::new(Self {
                outcome,
                seen: Mutex::new(Vec::new()),
            })
        }

        fn calls(&self) -> Vec<(String, Value)> {
            self.seen.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl McpCaller for StubCaller {
        async fn call_tool(
            &self,
            tool: &str,
            arguments: &Value,
            _cancel: Option<&CancellationToken>,
        ) -> McpCallOutcome {
            self.seen
                .lock()
                .unwrap()
                .push((tool.to_string(), arguments.clone()));
            self.outcome.clone()
        }
    }

    fn def(name: &str, annotations: McpAnnotations) -> McpToolDef {
        McpToolDef {
            name: name.to_string(),
            description: Some(format!("description for {name}")),
            input_schema: Some(json!({ "type": "object", "properties": {} })),
            annotations,
        }
    }

    fn tool(def: &McpToolDef, caller: Arc<StubCaller>) -> McpTool {
        let name = build_tool_names("srv-1", "Server One", std::slice::from_ref(def)).remove(0);
        McpTool::new(name, def, "abc123", caller)
    }

    #[test]
    fn name_is_three_segments() {
        let defs = [def("search", McpAnnotations::default())];
        let names = build_tool_names("srv-1", "Server One", &defs);
        assert_eq!(names[0].full, "mcp__srv-1__search");
        assert_eq!(names[0].tool, "search");
        assert_eq!(names[0].original, "search");
        assert_eq!(names[0].display, "Server One / search");
    }

    #[test]
    fn display_name_change_keeps_tool_name() {
        let defs = [def("search", McpAnnotations::default())];
        let before = build_tool_names("srv-1", "Old Name", &defs);
        let after = build_tool_names("srv-1", "New Name", &defs);
        assert_eq!(before[0].full, after[0].full);
        assert_ne!(before[0].display, after[0].display);
    }

    #[test]
    fn empty_display_name_falls_back_to_server_id() {
        let defs = [def("search", McpAnnotations::default())];
        let names = build_tool_names("srv-1", "   ", &defs);
        assert_eq!(names[0].display, "srv-1 / search");
    }

    #[test]
    fn illegal_chars_are_replaced() {
        assert_eq!(normalize_segment("a b/c"), "a_b_c");
        assert_eq!(normalize_segment("工具"), "__");
        assert_eq!(normalize_segment(""), "_");
        assert_eq!(normalize_segment("keep_-09"), "keep_-09");
    }

    #[test]
    fn duplicate_normalized_names_get_short_hash() {
        let defs = [
            def("a b", McpAnnotations::default()),
            def("a/b", McpAnnotations::default()),
            def("a-b", McpAnnotations::default()),
            def("plain", McpAnnotations::default()),
        ];
        let names = build_tool_names("srv-1", "Server One", &defs);
        assert_eq!(names[0].tool, format!("a_b_{}", short_hash("a b")));
        assert_eq!(names[1].tool, format!("a_b_{}", short_hash("a/b")));
        assert_ne!(names[0].tool, names[1].tool);
        assert_eq!(names[2].tool, "a-b");
        assert_eq!(names[3].tool, "plain");
        assert!(names[0].full.starts_with("mcp__srv-1__"));
        assert_eq!(names[0].original, "a b");
    }

    #[test]
    fn name_order_follows_input_order() {
        let defs = [
            def("z", McpAnnotations::default()),
            def("a", McpAnnotations::default()),
        ];
        let names = build_tool_names("srv-1", "Server One", &defs);
        assert_eq!(names[0].tool, "z");
        assert_eq!(names[1].tool, "a");
    }

    #[tokio::test]
    async fn tier_and_audience_are_external_agent_only() {
        let def = def("search", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::ToolNotFound);
        let tool = tool(&def, caller);
        assert_eq!(tool.tier(), ToolTier::External);
        assert_eq!(tool.audience(), ToolAudience::AGENT_ONLY);
        assert_eq!(tool.name(), "mcp__srv-1__search");
        assert_eq!(tool.id(), "mcp__srv-1__search");
    }

    #[tokio::test]
    async fn description_and_schema_are_passthrough() {
        let def = def("search", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::ToolNotFound);
        let tool = tool(&def, caller);
        assert_eq!(tool.description().await.unwrap(), "description for search");
        assert_eq!(
            tool.input_schema(),
            json!({ "type": "object", "properties": {} })
        );
    }

    #[test]
    fn missing_description_and_schema_have_defaults() {
        let def = McpToolDef {
            name: "bare".to_string(),
            description: None,
            input_schema: None,
            annotations: McpAnnotations::default(),
        };
        let caller = StubCaller::new(McpCallOutcome::ToolNotFound);
        let tool = tool(&def, caller);
        assert_eq!(tool.description.clone(), "");
        assert_eq!(tool.input_schema(), json!({ "type": "object" }));
    }

    #[test]
    fn permission_intent_resource_shape() {
        let def = def("search", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::ToolNotFound);
        let tool = tool(&def, caller);
        let intents = tool
            .permission_intents(&json!({}), &ToolUseContext::default())
            .unwrap();
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].action, "mcp");
        assert_eq!(intents[0].resources, vec!["mcp:srv-1:abc123".to_string()]);
    }

    #[test]
    fn no_annotations_is_destructive_and_open_world() {
        let hints = McpAnnotations::default().behavior_hints();
        assert!(!hints.read_only);
        assert!(hints.destructive);
        assert!(hints.open_world);
    }

    #[test]
    fn read_only_hint_is_not_destructive() {
        let hints = McpAnnotations {
            read_only_hint: Some(true),
            ..Default::default()
        }
        .behavior_hints();
        assert!(hints.read_only);
        assert!(!hints.destructive);
        assert!(!hints.open_world);
    }

    #[test]
    fn destructive_hint_upgrades_read_only() {
        let hints = McpAnnotations {
            read_only_hint: Some(true),
            destructive_hint: Some(true),
            ..Default::default()
        }
        .behavior_hints();
        assert!(hints.read_only);
        assert!(hints.destructive);
    }

    #[test]
    fn false_hints_cannot_downgrade() {
        let hints = McpAnnotations {
            read_only_hint: Some(false),
            destructive_hint: Some(false),
            open_world_hint: Some(false),
        }
        .behavior_hints();
        assert!(!hints.read_only);
        assert!(hints.destructive);
        assert!(hints.open_world);
    }

    #[test]
    fn open_world_hint_can_upgrade_read_only() {
        let hints = McpAnnotations {
            read_only_hint: Some(true),
            open_world_hint: Some(true),
            ..Default::default()
        }
        .behavior_hints();
        assert!(!hints.destructive);
        assert!(hints.open_world);
    }

    #[tokio::test]
    async fn execute_sends_original_name_and_maps_text() {
        let def = def("search", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::Content {
            blocks: vec![
                json!({ "type": "text", "text": "line one" }),
                json!({ "type": "text", "text": "line two" }),
            ],
            is_error: false,
        });
        let tool = tool(&def, caller.clone());
        let args = json!({ "query": "rust" });
        let result = tool
            .execute(&args, &ToolUseContext::default())
            .await
            .unwrap();

        assert!(result.ok);
        assert_eq!(
            result.result_for_assistant.as_deref(),
            Some("line one\nline two")
        );
        assert_eq!(result.data.as_ref().unwrap().as_array().unwrap().len(), 2);
        let calls = caller.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "search");
        assert_eq!(calls[0].1, args);
    }

    #[tokio::test]
    async fn is_error_maps_to_business_failure() {
        let def = def("search", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::Content {
            blocks: vec![json!({ "type": "text", "text": "boom" })],
            is_error: true,
        });
        let tool = tool(&def, caller);
        let result = tool
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .unwrap();
        assert!(!result.ok);
        assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
        assert_eq!(result.error.as_deref(), Some("boom"));
    }

    #[tokio::test]
    async fn tool_not_found_is_business_failure() {
        let def = def("search", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::ToolNotFound);
        let tool = tool(&def, caller);
        let result = tool
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .unwrap();
        assert!(!result.ok);
        assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
    }

    #[tokio::test]
    async fn server_unavailable_is_internal_failure() {
        let def = def("search", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::ServerUnavailable);
        let tool = tool(&def, caller);
        let result = tool
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .unwrap();
        assert!(!result.ok);
        assert_eq!(result.error_kind, Some(ToolErrorKind::Internal));
    }

    #[tokio::test]
    async fn timeout_and_cancelled_keep_their_error_kinds() {
        let def = def("search", McpAnnotations::default());

        let timeout = tool(&def, StubCaller::new(McpCallOutcome::Timeout))
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .unwrap();
        assert_eq!(timeout.error_kind, Some(ToolErrorKind::Timeout));

        let cancelled = tool(&def, StubCaller::new(McpCallOutcome::Cancelled))
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .unwrap();
        assert_eq!(cancelled.error_kind, Some(ToolErrorKind::Cancelled));
    }

    #[tokio::test]
    async fn image_blocks_do_not_enter_assistant_text() {
        let def = def("shot", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::Content {
            blocks: vec![
                json!({ "type": "text", "text": "captured" }),
                json!({ "type": "image", "data": "BASE64PAYLOAD", "mimeType": "image/png" }),
            ],
            is_error: false,
        });
        let tool = tool(&def, caller);
        let result = tool
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .unwrap();
        let assistant = result.assistant_text();
        assert_eq!(assistant, "captured");
        assert!(!assistant.contains("BASE64PAYLOAD"));
        assert_eq!(result.data.as_ref().unwrap().as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn non_text_only_result_does_not_serialize_payload() {
        let def = def("shot", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::Content {
            blocks: vec![json!({ "type": "image", "data": "BASE64PAYLOAD" })],
            is_error: false,
        });
        let tool = tool(&def, caller);
        let result = tool
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .unwrap();
        let assistant = result.assistant_text();
        assert!(!assistant.contains("BASE64PAYLOAD"));
        assert_eq!(
            assistant,
            "MCP tool returned 1 content block(s) without text"
        );
    }

    #[tokio::test]
    async fn empty_content_result_has_summary() {
        let def = def("noop", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::Content {
            blocks: Vec::new(),
            is_error: false,
        });
        let tool = tool(&def, caller);
        let result = tool
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .unwrap();
        assert_eq!(
            result.assistant_text(),
            "MCP tool returned 0 content block(s) without text"
        );
    }

    #[tokio::test]
    async fn non_object_input_is_rejected_without_calling_server() {
        let def = def("search", McpAnnotations::default());
        let caller = StubCaller::new(McpCallOutcome::ToolNotFound);
        let tool = tool(&def, caller.clone());
        let result = tool
            .execute(&json!([1, 2]), &ToolUseContext::default())
            .await
            .unwrap();
        assert!(!result.ok);
        assert_eq!(result.error_kind, Some(ToolErrorKind::Business));
        assert!(caller.calls().is_empty());
    }

    struct PanicCaller;

    #[async_trait]
    impl McpCaller for PanicCaller {
        async fn call_tool(
            &self,
            _tool: &str,
            _arguments: &Value,
            _cancel: Option<&CancellationToken>,
        ) -> McpCallOutcome {
            panic!("transport blew up");
        }
    }

    #[cfg(panic = "unwind")]
    #[tokio::test]
    async fn caller_panic_is_normalized_to_internal_failure() {
        let def = def("search", McpAnnotations::default());
        let name = build_tool_names("srv-1", "Server One", std::slice::from_ref(&def)).remove(0);
        let caller: Arc<dyn McpCaller> = Arc::new(PanicCaller);
        let tool = McpTool::new(name, &def, "abc123", caller);

        let result = tool
            .execute(&json!({}), &ToolUseContext::default())
            .await
            .unwrap();

        assert!(!result.ok);
        assert_eq!(result.error_kind, Some(ToolErrorKind::Internal));
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("panicked"),
            "{:?}",
            result.error
        );
    }
}
