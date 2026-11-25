use std::path::PathBuf;

use crate::context::ToolUseContext;
use crate::os::path;

/// 工具作用域的基准目录：显式 `path` 优先，否则回落到上下文的 workspace_root / resolve_base。
///
/// 仅供**浏览类**工具（`list` / `grep` / `glob`）使用，故走 `resolve_browse`：
/// 引用一律拒绝，且用浏览专用文案而非写方向文案（I73 §4 V5）。
pub(crate) fn base_directory(
    path_input: Option<&path::PathRef>,
    context: &ToolUseContext,
) -> Result<PathBuf, String> {
    match path_input {
        Some(input) => input.resolve_browse(context),
        None => {
            let root = path::resolution_base(context).ok_or_else(|| {
                "'path' is required when the context has no workspace_root or resolve_base; pass \
                 an explicit directory"
                    .to_string()
            })?;
            path::parse_ref(root)?.resolve_browse(context)
        }
    }
}
