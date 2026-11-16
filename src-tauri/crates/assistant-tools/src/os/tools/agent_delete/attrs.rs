use std::path::Path;

/// 目标文件是否带只读属性。
///
/// Windows 上 `std::fs::remove_file` 在 `DeleteFileW` 失败后会**自动清掉**只读属性再重试，
/// 那等于「删除顺手改了属性」——正是 delete §3.4 明确禁止的额外破坏性写。为了保持
/// 「删只读文件 → `Business` + 提示先移除属性」的确定性，这里改成前置拒绝：
/// 不在删除路径里改任何属性，只把选择权交回用户。
pub(crate) fn is_readonly(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
        metadata.file_attributes() & FILE_ATTRIBUTE_READONLY != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

pub(crate) fn readonly_message(target: &Path) -> String {
    format!(
        "'{}' is read-only; delete never changes file attributes, so this target cannot be removed",
        target.display()
    )
}

/// 是否「链接类」条目（Unix symlink、Windows symlink 与 junction）。
///
/// Windows 的 junction 在 `symlink_metadata` 里表现为**目录 + 重解析点**，`is_symlink()`
/// 为 false；只靠 `is_dir()` 会把 junction 当成真实目录去递归，等于删掉它的目标内容
/// （delete §3.2 明令禁止）。因此这里额外看 `FILE_ATTRIBUTE_REPARSE_POINT`。
pub(crate) fn is_link_like(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
