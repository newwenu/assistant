//! MCP 外部工具桥接层（P7-C1）。
//!
//! 本模块只回答「MCP 工具如何变成一个 `Tool`」：命名、安全标记推断、结果映射、
//! 精确审批键所需的身份信息。不做 IO、不持有子进程、不碰 `ToolRegistry`。
//!
//! 规格：[`docs/archived/agent/I38e-mcp-tools-spec.md`](../../../../../docs/archived/agent/I38e-mcp-tools-spec.md) §3。

mod call;
mod defs;
mod naming;

pub use call::*;
pub use defs::*;
pub use naming::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ToolUseContext;
    use crate::r#trait::{Tool, ToolAudience, ToolTier};
    use crate::result::ToolErrorKind;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};
    use tokio_util::sync::CancellationToken;

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
        assert_eq!(tool.description, "");
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
