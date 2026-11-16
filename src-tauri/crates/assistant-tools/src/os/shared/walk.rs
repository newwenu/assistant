use std::path::Path;

/// 遍历配置：跳过隐藏项，遵守 `.gitignore` / 全局忽略 / `.git/info/exclude`，不跟随链接。
pub(crate) fn builder(base: &Path) -> ignore::WalkBuilder {
    let mut builder = ignore::WalkBuilder::new(base);
    builder
        .hidden(true)
        .require_git(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .follow_links(false);
    builder
}
