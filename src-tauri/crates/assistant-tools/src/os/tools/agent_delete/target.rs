use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::context::ToolUseContext;
use crate::os::path;

pub(crate) struct DeleteRequest {
    pub(crate) path: path::PathRef,
    pub(crate) recursive: bool,
    pub(crate) permanent: bool,
}

impl DeleteRequest {
    pub(crate) fn parse(input: &Value) -> Result<Self, String> {
        let object = input
            .as_object()
            .ok_or_else(|| "input must be a JSON object".to_string())?;

        let path = path::parse_path_arg(object, "path")?;

        let recursive = match object.get("recursive") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("'recursive' must be a boolean".to_string()),
        };

        let permanent = match object.get("permanent") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err("'permanent' must be a boolean".to_string()),
        };

        Ok(Self {
            path,
            recursive,
            permanent,
        })
    }
}

/// 解析删除目标。
///
/// 用 `resolve_leaf_preserving` 而不是 `resolve`：`resolve` 会把叶子上的符号链接换成真实
/// 目标，删除就变成「删目标」而不是「删链接」（delete §3.2 明令禁止）。
/// 父目录仍按 `canonicalize_lenient` 归一，`..` 穿越、受保护目录与 workspace 边界照常生效。
pub(crate) fn resolve_target(
    request: &DeleteRequest,
    context: &ToolUseContext,
) -> Result<PathBuf, String> {
    let raw = Path::new(request.path.raw());
    if raw.parent().is_none() {
        return Err(format!(
            "refusing to delete the filesystem root '{}'",
            request.path.raw()
        ));
    }
    let target = request.path.resolve_leaf_preserving(context)?;
    reject_undeletable(&target, context)?;
    Ok(target)
}

/// 不可删目标：文件系统根、`workspace_root` 本身、家目录本身。
///
/// 只拒家目录本身，不拒其子项（工作区通常就在家目录下）。凭据目录与系统目录由
/// `os::path` 的 `reject_system_dirs` 在解析阶段拒绝，这里不重复判定。
///
/// 每个候选都同时检查「叶子保留路径」与「跟随链接后的真实路径」：指向工作区根 / 家目录
/// 的链接同样拒绝（delete §3.1 的「链接形式」），删除链接虽然只删链接，但没有理由允许
/// 模型对这类目标下手。
fn reject_undeletable(target: &Path, context: &ToolUseContext) -> Result<(), String> {
    let followed = path::canonicalize_lenient(target).ok();
    let candidates = std::iter::once(target.to_path_buf()).chain(followed);

    for candidate in candidates {
        if candidate.parent().is_none() {
            return Err(format!(
                "refusing to delete the filesystem root '{}'",
                candidate.display()
            ));
        }
        if let Some(root) = context.workspace_root.as_deref() {
            if let Ok(root) = path::canonicalize_lenient(Path::new(root)) {
                if path::same_path(&candidate, &root) {
                    return Err(format!(
                        "refusing to delete the workspace root itself ('{}')",
                        root.display()
                    ));
                }
            }
        }
        if let Some(home) = home_dir() {
            if let Ok(home) = path::canonicalize_lenient(&home) {
                if path::same_path(&candidate, &home) {
                    return Err(format!(
                        "refusing to delete the home directory itself ('{}')",
                        home.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
