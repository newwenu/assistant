use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::context::ToolUseContext;

use super::LinkKind;

pub(super) struct CreateInput<'a> {
    pub source: &'a str,
    pub target: &'a str,
    pub kind: LinkKind,
}

pub(super) fn parse_create_input(input: &Value) -> Result<CreateInput<'_>, String> {
    let source = input
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing 'source' field".to_string())?;
    let target = input
        .get("target")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing 'target' field".to_string())?;
    let kind_str = input
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing 'kind' field".to_string())?;

    Ok(CreateInput {
        source,
        target,
        kind: LinkKind::from_str(kind_str)?,
    })
}

pub(super) fn parse_check_input(input: &Value) -> Result<&str, String> {
    input
        .get("source")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing 'source' field".to_string())
}

pub(super) fn resolve_path(raw: &str, context: &ToolUseContext) -> Result<PathBuf, String> {
    super::super::path::resolve_leaf_preserving(raw, context)
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
