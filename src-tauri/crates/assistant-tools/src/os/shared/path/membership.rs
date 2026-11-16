use std::path::Path;

use crate::context::ToolUseContext;

use super::component_prefix_match;
use super::resolve::canonicalize_lenient;

/// 确认路径位于 workspace 根之内。
///
/// 输入：`path` 必须已由 `canonicalize_lenient` 或 `resolve_leaf_preserving` 归一；
/// `root` 在此处做宽松规范化（workspace 根本身可以是符号链接）。
/// 返回：`Ok(())` 表示 `path` 以 `root` 为组件级前缀（`path == root` 允许）。
/// 失败：`root` 不可规范化，或 `path` 越出根。
pub fn ensure_within_root(path: &Path, root: &Path) -> Result<(), String> {
    let root_canonical = canonicalize_lenient(root)?;
    if !component_prefix_match(path, &root_canonical) {
        return Err(format!(
            "path '{}' is outside workspace root '{}'",
            path.display(),
            root_canonical.display()
        ));
    }
    Ok(())
}

/// 路径相对工作区根的位置，供审批卡片回答「工作区内/外」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceMembership {
    /// 已配置工作区，且路径位于其内。
    Inside,
    /// 已配置工作区，但路径越界。
    Outside,
    /// 未配置工作区：没有边界，就没有结论。
    Unconfigured,
}

/// 判定路径是否落在工作区内。
///
/// 直接复用 [`ensure_within_root`]：审批卡片的结论与执行期的边界校验同源，
/// 不会出现「审批说在区内、执行说越界」两套口径。
pub fn workspace_membership(path: &Path, context: &ToolUseContext) -> WorkspaceMembership {
    let Some(root) = context.workspace_root.as_deref() else {
        return WorkspaceMembership::Unconfigured;
    };
    match ensure_within_root(path, Path::new(root)) {
        Ok(()) => WorkspaceMembership::Inside,
        Err(_) => WorkspaceMembership::Outside,
    }
}
