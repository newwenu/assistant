use async_trait::async_trait;
use serde_json::Value;

use crate::context::ToolUseContext;
use crate::r#trait::{BehaviorHints, PermissionIntent, Tool, ToolAudience, ToolTier};
use crate::result::ToolResult;

mod check;
mod create;
mod validate;

/// 内部唯一标识（注册表键、白名单匹配、审计）。模型可见名默认与它同名。
const TOOL_ID: &str = "fs_link";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LinkKind {
    Symlink,
    Hardlink,
    #[cfg(windows)]
    Junction,
}

impl LinkKind {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "symlink" => Ok(Self::Symlink),
            "hardlink" => Ok(Self::Hardlink),
            #[cfg(windows)]
            "junction" => Ok(Self::Junction),
            _ => Err(format!(
                "unsupported link kind '{}'; expected 'symlink' or 'hardlink'{}",
                s,
                if cfg!(windows) { " or 'junction'" } else { "" }
            )),
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Symlink => "symlink",
            Self::Hardlink => "hardlink",
            #[cfg(windows)]
            Self::Junction => "junction",
        }
    }
}

pub struct FsLinkTool;

impl FsLinkTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsLinkTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FsLinkTool {
    fn id(&self) -> &str {
        TOOL_ID
    }

    async fn description(&self) -> Result<String, String> {
        Ok("Create or check filesystem links (symlink, hardlink, junction)".to_string())
    }

    fn input_schema(&self) -> Value {
        let mut kind_enum = vec!["symlink".to_string(), "hardlink".to_string()];
        #[cfg(windows)]
        kind_enum.push("junction".to_string());

        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "check"],
                    "description": "Action to perform"
                },
                "source": {
                    "type": "string",
                    "description": "Source path (existing file/dir for create; link path for check)"
                },
                "target": {
                    "type": "string",
                    "description": "Target path (where the link points to)"
                },
                "kind": {
                    "type": "string",
                    "enum": kind_enum,
                    "description": "Link type"
                }
            },
            "required": ["action", "source"],
            "additionalProperties": false
        })
    }

    fn tier(&self) -> ToolTier {
        ToolTier::Os
    }

    fn audience(&self) -> ToolAudience {
        ToolAudience::TOOLBOX_ONLY
    }

    fn behavior_hints(&self) -> BehaviorHints {
        BehaviorHints {
            read_only: false,
            destructive: false,
            open_world: true,
        }
    }

    fn permission_intents(
        &self,
        input: &Value,
        _context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let action = input.get("action").and_then(Value::as_str).unwrap_or("");

        if action == "check" {
            return Ok(vec![]);
        }

        let source = input
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let target = input
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");

        Ok(vec![
            PermissionIntent::new("edit", source),
            PermissionIntent::new("edit", target),
        ])
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let action = input
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing 'action' field".to_string())?;

        match action {
            "create" => self.exec_create(input, context).await,
            "check" => self.exec_check(input, context).await,
            _ => Err(format!("unknown action '{}'", action)),
        }
    }
}

impl FsLinkTool {
    async fn exec_create(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<ToolResult, String> {
        let parsed = validate::parse_create_input(input)?;
        let source = validate::resolve_path(parsed.source, context)?;
        let target = validate::resolve_path(parsed.target, context)?;

        if let Err(message) = validate::ensure_target_exists(&target) {
            return Ok(ToolResult::err(message));
        }
        if let Err(message) = validate::ensure_source_parent_exists(&source) {
            return Ok(ToolResult::err(message));
        }

        create::create_link(&source, &target, parsed.kind)?;

        Ok(ToolResult::ok(serde_json::json!({
            "source": parsed.source,
            "target": parsed.target,
            "kind": parsed.kind.as_str(),
            "created": true
        })))
    }

    async fn exec_check(
        &self,
        input: &Value,
        context: &ToolUseContext,
    ) -> Result<ToolResult, String> {
        let source_str = validate::parse_check_input(input)?;
        let source = validate::resolve_path(source_str, context)?;

        let info = check::check_link(&source)?;

        Ok(ToolResult::ok(serde_json::json!({
            "source": source_str,
            "isLink": info.is_link,
            "kind": info.kind,
            "target": info.target,
        })))
    }
}
