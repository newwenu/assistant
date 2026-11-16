use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::context::ToolUseContext;
use crate::os::path;

use super::LinkKind;

pub(super) struct CreateInput {
    pub source: String,
    pub target: String,
    pub kind: LinkKind,
}

pub(super) fn parse_create_input(input: &Value) -> Result<CreateInput, String> {
    let object = input
        .as_object()
        .ok_or_else(|| "input must be a JSON object".to_string())?;
    let source = path::parse_path_arg(object, "source")?;
    let target = path::parse_path_arg(object, "target")?;
    let kind_str = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing 'kind' field".to_string())?;

    Ok(CreateInput {
        source,
        target,
        kind: LinkKind::from_str(kind_str)?,
    })
}

pub(super) fn parse_check_input(input: &Value) -> Result<String, String> {
    let object = input
        .as_object()
        .ok_or_else(|| "input must be a JSON object".to_string())?;
    path::parse_path_arg(object, "source")
}

pub(super) fn resolve_path(raw: &str, context: &ToolUseContext) -> Result<PathBuf, String> {
    crate::os::path::resolve_leaf_preserving(raw, context)
}

pub(super) fn ensure_target_exists(target: &Path) -> Result<(), String> {
    if target.exists() {
        return Ok(());
    }
    Err(format!("target '{}' does not exist", target.display()))
}

pub(super) fn ensure_source_parent_exists(source: &Path) -> Result<(), String> {
    let Some(parent) = source.parent() else {
        return Err(format!(
            "source '{}' has no parent directory",
            source.display()
        ));
    };
    if parent.exists() {
        return Ok(());
    }
    Err(format!(
        "parent directory '{}' of source does not exist",
        parent.display()
    ))
}
