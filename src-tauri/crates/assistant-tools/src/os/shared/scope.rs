use std::path::PathBuf;

use crate::context::ToolUseContext;
use crate::os::path;

/// 工具作用域的基准目录：显式 `path` 优先，否则回落到上下文的 workspace_root / resolve_base。
pub(crate) fn base_directory(
    path_input: Option<&str>,
    context: &ToolUseContext,
) -> Result<PathBuf, String> {
    match path_input {
        Some(input) => path::resolve(input, context),
        None => {
            let root = path::resolution_base(context).ok_or_else(|| {
                "'path' is required when the context has no workspace_root or resolve_base; pass \
                 an explicit directory"
                    .to_string()
            })?;
            path::resolve(root, context)
        }
    }
}
