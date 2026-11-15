use async_trait::async_trait;
use serde_json::Value;

use crate::context::ToolUseContext;
use crate::os::path;
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
        #[cfg(windows)]
        let kind_enum = vec![
            "symlink".to_string(),
            "hardlink".to_string(),
            "junction".to_string(),
        ];
        #[cfg(not(windows))]
        let kind_enum = vec!["symlink".to_string(), "hardlink".to_string()];

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
        context: &ToolUseContext,
    ) -> Result<Vec<PermissionIntent>, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;
        match object.get("action").and_then(Value::as_str) {
            Some("create") => {}
            // `check` 只读：声明 `read` 而不是空 intent，避免与「未声明意图」混为一谈。
            // 解析失败返回 `Err`，由网关记 `intentError`。
            Some("check") => {
                let source = validate::parse_check_input(input)?;
                let source = validate::resolve_path(&source, context)?;
                return Ok(vec![PermissionIntent::new(
                    "read",
                    source.display().to_string(),
                )]);
            }
            Some(other) => return Err(format!("unknown action '{}'", other)),
            None => return Err("missing 'action' field".to_string()),
        }

        // 意图必须用解析后的路径：策略规则按绝对路径匹配，原始字符串（含相对路径、
        // `..` 段）会让同一目标在不同规则下命中不一致；缺失字段也不能兜底成占位符。
        // `kind` 不参与意图：审批问的是「这两个路径要被改动」，链接类型不影响授权范围。
        let source = path::parse_path_arg(object, "source")?;
        let target = path::parse_path_arg(object, "target")?;
        let source = validate::resolve_path(&source, context)?;
        let target = validate::resolve_path(&target, context)?;

        Ok(vec![
            PermissionIntent::new("edit", source.display().to_string()),
            PermissionIntent::new("edit", target.display().to_string()),
        ])
    }

    async fn execute(&self, input: &Value, context: &ToolUseContext) -> Result<ToolResult, String> {
        let action = match input.get("action").and_then(Value::as_str) {
            Some(action) => action,
            None => return Ok(ToolResult::err("missing 'action' field".to_string())),
        };

        match action {
            "create" => self.exec_create(input, context).await,
            "check" => self.exec_check(input, context).await,
            _ => Ok(ToolResult::err(format!("unknown action '{}'", action))),
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
        let source = validate::resolve_path(&parsed.source, context)?;
        let target = validate::resolve_path(&parsed.target, context)?;

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
        let source = validate::resolve_path(&source_str, context)?;

        let info = check::check_link(&source)?;

        Ok(ToolResult::ok(serde_json::json!({
            "source": source_str,
            "isLink": info.is_link,
            "kind": info.kind,
            "target": info.target,
        })))
    }
}
