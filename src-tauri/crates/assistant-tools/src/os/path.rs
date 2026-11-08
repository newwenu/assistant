use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::context::ToolUseContext;

/// 把工具输入解析为可安全使用的绝对路径。
///
/// 输入：原始路径字符串（绝对路径，或配合 `workspace_root` 的相对路径）+ 上下文。
/// 返回：经 `canonicalize_lenient` 归一、并通过受保护目录与 workspace 边界检查的绝对路径。
/// 失败：盘符相对路径（`C:foo`）、含 `..`、相对路径缺少 `workspace_root`、
/// `workspace_root` 非绝对路径、路径不可归一、落在受保护目录、越出 workspace 根。
///
/// 调用方必须使用返回值做 IO，不得再用原始字符串，避免「校验 A、使用 B」。
pub fn resolve(input: &str, context: &ToolUseContext) -> Result<PathBuf, String> {
    let joined = join_input(input, context)?;
    let canonical = canonicalize_lenient(&joined)?;
    reject_system_dirs(&canonical)?;
    if let Some(root) = context.workspace_root.as_deref() {
        ensure_within_root(&canonical, Path::new(root))?;
    }
    Ok(canonical)
}

/// 与 `resolve` 相同的边界检查，但**保留叶子组件**，只规范化父目录。
///
/// 输入：原始路径字符串 + 上下文。
/// 返回：父目录经 `canonicalize_lenient` 归一后拼接原样叶子的绝对路径。
/// 失败：与 `resolve` 相同的形态错误，外加路径没有叶子组件（如根目录本身）。
///
/// 用于「操作链接本身」的工具：`canonicalize` 会穿过符号链接解析到真实目标，
/// 对 `check`（判断叶子是不是链接）与 `create`（保留链接语义）都是错的。
pub fn resolve_leaf_preserving(input: &str, context: &ToolUseContext) -> Result<PathBuf, String> {
    let joined = join_input(input, context)?;
    let normalized = normalize_components(&joined, input)?;
    let leaf = normalized
        .file_name()
        .ok_or_else(|| format!("path '{}' has no file name component to preserve", input))?;
    let parent = normalized
        .parent()
        .ok_or_else(|| format!("path '{}' has no parent directory", input))?;

    let parent_canonical = canonicalize_lenient(parent)?;
    let resolved = parent_canonical.join(leaf);

    reject_system_dirs(&resolved)?;
    if let Some(root) = context.workspace_root.as_deref() {
        ensure_within_root(&resolved, Path::new(root))?;
    }
    Ok(resolved)
}

/// 宽松规范化：路径不存在时也给出确定结果。
///
/// 输入：待规范化的路径。
/// 返回：路径存在时为 `canonicalize` 结果；不存在时为「第一个存在的祖先的规范化结果 + 原样尾部」。
/// 失败：含 `..`、盘符相对路径、`canonicalize` 失败、找不到任何存在的祖先（如盘符不存在）。
///
/// 绝不在规范化失败时回退为原路径：那会让校验对象与实际使用对象分叉。
pub fn canonicalize_lenient(path: &Path) -> Result<PathBuf, String> {
    let original = path.display().to_string();
    reject_drive_relative(path, &original)?;
    let normalized = normalize_components(path, &original)?;

    if normalized.exists() {
        return canonicalize_existing(&normalized);
    }

    let mut ancestor = normalized;
    let mut tail: Vec<OsString> = Vec::new();
    loop {
        if ancestor.exists() {
            break;
        }
        let name = ancestor
            .file_name()
            .ok_or_else(|| format!("path '{}' has no existing ancestor", original))?;
        tail.push(name.to_os_string());
        let parent = ancestor
            .parent()
            .ok_or_else(|| format!("path '{}' has no existing ancestor", original))?;
        ancestor = parent.to_path_buf();
    }

    let mut canonical = canonicalize_existing(&ancestor)?;
    for name in tail.iter().rev() {
        canonical.push(name);
    }
    Ok(canonical)
}

/// 拒绝落在受保护系统目录或凭据目录内的路径。
///
/// 输入：已归一的路径。
/// 返回：`Ok(())` 表示可以继续。
/// 失败：路径以某个受保护目录为组件级前缀（`C:\Windows.old` 不属于 `C:\Windows`），
/// 或任一组件命中凭据目录名单。
///
/// Windows 比较大小写不敏感，受保护目录取自 `SystemRoot` / `ProgramFiles` /
/// `ProgramFiles(x86)` / `ProgramData`，不写死盘符；Unix 使用固定列表。
/// 凭据目录（`.ssh` / `.gnupg` / `.aws` / `.azure` / `.kube` / `keychains` 等）
/// 无论位于何处一律拒绝，判定先于 `workspace_root` 授权。
pub fn reject_system_dirs(path: &Path) -> Result<(), String> {
    for protected in protected_dirs() {
        if component_prefix_match(path, &protected) {
            return Err(format!(
                "path '{}' is inside a protected system directory '{}'",
                path.display(),
                protected.display()
            ));
        }
    }
    reject_credential_dirs(path)
}

/// 判断 `path` 是否以 `base` 为组件级前缀（`path == base` 允许）。
///
/// 比较按路径组件进行：`/a/bc` 不属于 `/a/b`；Windows 大小写不敏感。
pub fn is_component_prefix(path: &Path, base: &Path) -> bool {
    component_prefix_match(path, base)
}

/// 判断两个路径是否指向同一位置（互为组件级前缀）。
///
/// 输入：两个已由 `resolve` / `resolve_leaf_preserving` 归一过的路径。
/// 用途：破坏性操作的目标判等。禁止用原始字符串比较——`a.txt` 与 `./a.txt` 解析后相同，
/// 字符串比较会把「同一目标」误判成两个，`move` 因此先删目标再移动而丢文件。
pub fn same_path(left: &Path, right: &Path) -> bool {
    component_prefix_match(left, right) && component_prefix_match(right, left)
}

/// 取必填路径参数：缺失、非字符串、空串、首尾空白都拒绝。
///
/// 不做 `trim()` 静默修正：`" a.txt"` 与 `"a.txt"` 是两个不同路径，静默改写会让
/// 删除 / 覆盖落到调用方没有指定的目标上。
pub fn parse_path_arg(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, String> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or non-string '{}' field", key))?;
    validate_path_text(value, key)?;
    Ok(value.to_string())
}

/// 取可选路径参数：缺失或 `null` 返回 `None`，其余按 [`parse_path_arg`] 校验。
pub fn parse_optional_path_arg(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => parse_path_arg(object, key).map(Some),
    }
}

fn validate_path_text(value: &str, key: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("missing or empty '{}' field", key));
    }
    if value != value.trim() {
        return Err(format!(
            "'{}' must not have leading or trailing whitespace",
            key
        ));
    }
    Ok(())
}

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

fn join_input(input: &str, context: &ToolUseContext) -> Result<PathBuf, String> {
    let raw = Path::new(input);
    reject_drive_relative(raw, input)?;

    if raw.is_absolute() {
        return Ok(raw.to_path_buf());
    }

    let Some(root) = context.workspace_root.as_deref() else {
        return Err(format!(
            "relative path '{}' requires workspace_root in context",
            input
        ));
    };

    let root_path = Path::new(root);
    if !root_path.is_absolute() {
        return Err(format!(
            "workspace_root '{}' must be an absolute path",
            root
        ));
    }
    if !root_path.exists() {
        return Err(format!("workspace_root '{}' does not exist", root));
    }

    Ok(root_path.join(raw))
}

fn reject_drive_relative(path: &Path, original: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut has_prefix = false;
        let mut has_root = false;
        for component in path.components() {
            match component {
                Component::Prefix(_) => has_prefix = true,
                Component::RootDir => has_root = true,
                _ => {}
            }
        }
        if has_prefix && !has_root {
            return Err(format!(
                "relative path with drive prefix is not supported: '{}'",
                original
            ));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (path, original);
    }
    Ok(())
}

fn normalize_components(path: &Path, original: &str) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    let mut has_normal = false;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(std::path::MAIN_SEPARATOR_STR),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!(
                    "path '{}' contains '..' traversal, use a canonical path instead",
                    original
                ))
            }
            Component::Normal(name) => {
                normalized.push(name);
                has_normal = true;
            }
        }
    }

    if !has_normal && normalized.as_os_str().is_empty() {
        normalized.push(".");
    }
    Ok(normalized)
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize '{}': {}", path.display(), error))?;
    Ok(simplify_verbatim(&canonical))
}

fn simplify_verbatim(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let simplified = dunce::simplified(path);
        let text = simplified.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{}", rest));
        }
        simplified.to_path_buf()
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

fn component_prefix_match(path: &Path, base: &Path) -> bool {
    let path_components: Vec<Component> = path.components().collect();
    let base_components: Vec<Component> = base.components().collect();

    if base_components.len() > path_components.len() {
        return false;
    }

    base_components
        .iter()
        .zip(path_components.iter())
        .all(|(base_component, path_component)| component_eq(base_component, path_component))
}

fn component_eq(left: &Component, right: &Component) -> bool {
    #[cfg(windows)]
    {
        left.as_os_str().to_string_lossy().to_lowercase()
            == right.as_os_str().to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        left.as_os_str() == right.as_os_str()
    }
}

fn protected_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let keys = [
            "SystemRoot",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramData",
        ];
        let mut dirs = Vec::new();
        for key in keys {
            let Some(value) = std::env::var_os(key) else {
                continue;
            };
            match canonicalize_lenient(Path::new(&value)) {
                Ok(canonical) => dirs.push(canonical),
                Err(error) => log::warn!("skip protected dir from '{}': {}", key, error),
            }
        }
        dirs
    }
    #[cfg(unix)]
    {
        ["/etc", "/usr", "/bin", "/sbin", "/System", "/Library"]
            .iter()
            .copied()
            .map(PathBuf::from)
            .collect()
    }
    #[cfg(not(any(unix, windows)))]
    {
        Vec::new()
    }
}

const CREDENTIAL_DIR_COMPONENTS: &[&str] =
    &[".ssh", ".gnupg", ".aws", ".azure", ".kube", "keychains"];

#[cfg(windows)]
const MICROSOFT_CREDENTIAL_DIR_COMPONENTS: &[&str] = &["Protect", "Credentials", "Crypto", "Vault"];

fn reject_credential_dirs(path: &Path) -> Result<(), String> {
    let components: Vec<&std::ffi::OsStr> = path
        .components()
        .map(|component| component.as_os_str())
        .collect();

    for (index, component) in components.iter().enumerate() {
        let name = component.to_string_lossy();
        if CREDENTIAL_DIR_COMPONENTS
            .iter()
            .any(|dir| name.eq_ignore_ascii_case(dir))
        {
            return Err(format!(
                "path '{}' is inside a protected credential directory '{}'",
                path.display(),
                name
            ));
        }

        #[cfg(windows)]
        if name.eq_ignore_ascii_case("Microsoft") {
            if let Some(next) = components.get(index + 1) {
                let next_name = next.to_string_lossy();
                if MICROSOFT_CREDENTIAL_DIR_COMPONENTS
                    .iter()
                    .any(|dir| next_name.eq_ignore_ascii_case(dir))
                {
                    return Err(format!(
                        "path '{}' is inside a protected credential directory 'Microsoft\\{}'",
                        path.display(),
                        next_name
                    ));
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context_with_root(root: &Path) -> ToolUseContext {
        ToolUseContext::for_toolbox(Some(root.display().to_string()))
    }

    fn create_dir_link(link: &Path, target: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            std::process::Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (link, target);
            false
        }
    }

    fn protected_root_for_test() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            std::env::var_os("SystemRoot").map(PathBuf::from)
        }
        #[cfg(unix)]
        {
            Some(PathBuf::from("/etc"))
        }
        #[cfg(not(any(unix, windows)))]
        {
            None
        }
    }

    #[test]
    fn parent_dir_traversal_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("child").join("..").join("file.txt");
        let error = canonicalize_lenient(&target).expect_err("must reject '..'");
        assert!(error.contains(".."), "unexpected error: {error}");
        assert!(
            error.contains(&target.display().to_string()),
            "error must quote the original path: {error}"
        );
    }

    #[test]
    fn nonexistent_path_keeps_existing_ancestor() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = canonicalize_lenient(dir.path()).expect("canonical root");
        let target = dir.path().join("missing").join("deeper").join("file.txt");
        let resolved = canonicalize_lenient(&target).expect("lenient canonicalize");
        assert_eq!(
            resolved,
            base.join("missing").join("deeper").join("file.txt")
        );
        assert!(!resolved.exists());
    }

    #[test]
    fn trailing_separator_and_curdir_are_normalized() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = canonicalize_lenient(dir.path()).expect("canonical root");

        let with_separator = format!("{}{}", dir.path().display(), std::path::MAIN_SEPARATOR);
        assert_eq!(
            canonicalize_lenient(Path::new(&with_separator)).expect("canonicalize"),
            base
        );

        let dotted = dir.path().join(".").join("missing");
        assert_eq!(
            canonicalize_lenient(&dotted).expect("canonicalize"),
            base.join("missing")
        );
    }

    #[test]
    fn mixed_curdir_and_parentdir_input_is_rejected() {
        let error = canonicalize_lenient(Path::new("a/./b/../c")).expect_err("must reject '..'");
        assert!(error.contains(".."), "unexpected error: {error}");
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_prefix_is_stripped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = canonicalize_lenient(dir.path()).expect("canonical root");
        let verbatim = PathBuf::from(format!(r"\\?\{}", dir.path().display()));
        let resolved = canonicalize_lenient(&verbatim).expect("canonicalize verbatim");
        assert_eq!(resolved, base);
        assert!(
            !resolved.to_string_lossy().starts_with(r"\\?\"),
            "verbatim prefix must be stripped: {}",
            resolved.display()
        );
    }

    #[cfg(windows)]
    #[test]
    fn unc_verbatim_prefix_is_simplified() {
        assert_eq!(
            simplify_verbatim(Path::new(r"\\?\UNC\server\share\dir\file.txt")),
            Path::new(r"\\server\share\dir\file.txt")
        );
        assert_eq!(
            simplify_verbatim(Path::new(r"\\?\C:\Windows")),
            Path::new(r"C:\Windows")
        );
    }

    #[cfg(windows)]
    #[test]
    fn drive_relative_path_is_rejected() {
        let error =
            canonicalize_lenient(Path::new("C:foo")).expect_err("must reject drive-relative");
        assert!(
            error.contains("relative path with drive prefix is not supported"),
            "unexpected error: {error}"
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let error = resolve("C:foo", &context).expect_err("must reject drive-relative");
        assert!(
            error.contains("relative path with drive prefix is not supported"),
            "unexpected error: {error}"
        );
    }

    /// UNC 路径按绝对路径处理，不要求 `workspace_root`。
    ///
    /// 只断言「绝对性判定」本身，不真的去访问共享：`resolve` 会触发 SMB 探测，
    /// 在共享不可达时 Windows 网络栈要等十几秒才超时（本机实测约 16s），
    /// 让每次全量测试都白等。判定逻辑是纯字符串/组件运算，无需触网即可覆盖。
    #[cfg(windows)]
    #[test]
    fn unc_path_is_treated_as_absolute() {
        let context = ToolUseContext::default();
        let joined = join_input(r"\\server\share\file.txt", &context)
            .expect("UNC path is absolute, must not require workspace_root");
        assert_eq!(joined, Path::new(r"\\server\share\file.txt"));
    }

    #[cfg(windows)]
    #[test]
    fn unc_path_can_be_resolved() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = canonicalize_lenient(dir.path()).expect("canonical root");
        let text = base.to_string_lossy().to_string();
        let Some(rest) = text.strip_prefix(r"C:\") else {
            return;
        };
        let unc = PathBuf::from(format!(r"\\localhost\C$\{}", rest));
        if !unc.exists() {
            return;
        }
        let resolved = canonicalize_lenient(&unc).expect("canonicalize unc");
        assert!(
            !resolved.to_string_lossy().starts_with(r"\\?\"),
            "verbatim prefix must be stripped: {}",
            resolved.display()
        );
        assert!(
            resolved.exists(),
            "resolved path must exist: {}",
            resolved.display()
        );
    }

    #[cfg(windows)]
    #[test]
    fn nonexistent_ancestor_is_an_error() {
        let drive = ('D'..='Z')
            .map(|letter| format!("{}:\\", letter))
            .find(|candidate| !Path::new(candidate).exists());
        let Some(drive) = drive else {
            return;
        };
        let target = Path::new(&drive).join("missing").join("file.txt");
        assert!(canonicalize_lenient(&target).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_old_is_not_rejected() {
        assert!(reject_system_dirs(Path::new(r"C:\Windows.old\data")).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn system32_is_rejected_case_insensitively() {
        let system_root = std::env::var("SystemRoot").expect("SystemRoot");
        let path = format!("{}\\System32\\cmd.exe", system_root.to_lowercase());
        assert!(reject_system_dirs(Path::new(&path)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn unix_protected_dirs_use_component_boundaries() {
        assert!(reject_system_dirs(Path::new("/etc/passwd")).is_err());
        assert!(reject_system_dirs(Path::new("/etcfoo/bar")).is_ok());
    }

    #[test]
    fn credential_dirs_are_rejected_by_component() {
        assert!(reject_system_dirs(Path::new("/home/me/.ssh/id_rsa")).is_err());
        assert!(reject_system_dirs(Path::new("/home/me/keychains/login.keychain")).is_err());
        assert!(reject_system_dirs(Path::new("/home/me/.aws/credentials")).is_err());
        assert!(reject_system_dirs(Path::new("/home/me/mykeychains/file.txt")).is_ok());
        assert!(reject_system_dirs(Path::new("/home/me/.ssh-backup/file.txt")).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn microsoft_credential_dirs_are_rejected() {
        assert!(reject_system_dirs(Path::new(
            r"C:\Users\me\AppData\Local\Microsoft\Credentials\abc"
        ))
        .is_err());
        assert!(reject_system_dirs(Path::new(
            r"C:\Users\me\AppData\Local\Microsoft\Crypto\keys"
        ))
        .is_err());
        assert!(reject_system_dirs(Path::new(
            r"C:\Users\me\AppData\Local\Microsoft\Windows\file.txt"
        ))
        .is_ok());
    }

    #[test]
    fn component_prefix_respects_component_boundaries() {
        assert!(is_component_prefix(Path::new("/a/b/c"), Path::new("/a/b")));
        assert!(is_component_prefix(Path::new("/a/b"), Path::new("/a/b")));
        assert!(!is_component_prefix(Path::new("/a/bc"), Path::new("/a/b")));
        assert!(!is_component_prefix(Path::new("/a"), Path::new("/a/b")));
    }

    #[test]
    fn path_outside_workspace_root_is_rejected() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let other_dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(root_dir.path());
        let input = other_dir.path().join("file.txt").display().to_string();
        let error = resolve(&input, &context).expect_err("must reject outside path");
        assert!(
            error.contains("outside workspace root"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn workspace_root_itself_is_allowed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let resolved = resolve(".", &context).expect("root itself must be allowed");
        assert_eq!(
            resolved,
            canonicalize_lenient(dir.path()).expect("canonical root")
        );
    }

    #[test]
    fn workspace_membership_matches_the_boundary_check() {
        let dir = tempfile::tempdir().expect("tempdir");
        let inside = dir.path().join("a.txt");
        std::fs::write(&inside, b"x").expect("write file");
        let other = tempfile::tempdir().expect("tempdir");
        let outside = other.path().join("b.txt");

        let context = context_with_root(dir.path());
        assert_eq!(
            workspace_membership(&inside, &context),
            WorkspaceMembership::Inside
        );
        assert_eq!(
            workspace_membership(&outside, &context),
            WorkspaceMembership::Outside
        );
        assert_eq!(
            workspace_membership(&inside, &ToolUseContext::default()),
            WorkspaceMembership::Unconfigured,
            "未配置工作区不得给出「在区内」的假结论"
        );
    }

    #[test]
    fn workspace_root_symlink_is_compared_after_canonicalization() {
        let dir = tempfile::tempdir().expect("tempdir");
        let real = dir.path().join("real");
        std::fs::create_dir(&real).expect("create real dir");
        let link = dir.path().join("link");
        if !create_dir_link(&link, &real) {
            return;
        }
        let context = context_with_root(&link);
        let resolved = resolve("inside.txt", &context).expect("resolve through symlinked root");
        assert_eq!(
            resolved,
            canonicalize_lenient(&real)
                .expect("canonical real")
                .join("inside.txt")
        );
    }

    #[test]
    fn relative_path_without_workspace_root_reports_error() {
        let error = resolve("relative.txt", &ToolUseContext::default())
            .expect_err("relative path must fail without root");
        assert!(
            error.contains("requires workspace_root"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn workspace_root_inside_protected_dir_is_rejected() {
        let Some(protected) = protected_root_for_test() else {
            return;
        };
        let context = context_with_root(&protected);
        let error = resolve("child.txt", &context).expect_err("protected root must be rejected");
        assert!(
            error.contains("protected system directory"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn workspace_root_must_exist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing_root = dir.path().join("missing_root");
        let context = context_with_root(&missing_root);
        let error = resolve("file.txt", &context).expect_err("missing root must be rejected");
        assert!(
            error.contains("workspace_root") && error.contains("does not exist"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn workspace_root_must_be_absolute() {
        let context = context_with_root(Path::new("relative_root"));
        let error = resolve("file.txt", &context).expect_err("relative root must be rejected");
        assert!(
            error.contains("must be an absolute path"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn leaf_preserving_keeps_leaf_and_canonicalizes_parent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = canonicalize_lenient(dir.path()).expect("canonical root");
        let context = context_with_root(dir.path());
        let resolved = resolve_leaf_preserving("missing/link.txt", &context).expect("resolve leaf");
        assert_eq!(resolved, base.join("missing").join("link.txt"));
    }

    #[test]
    fn leaf_preserving_rejects_paths_outside_root() {
        let root_dir = tempfile::tempdir().expect("tempdir");
        let other_dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(root_dir.path());
        let input = other_dir.path().join("link.txt").display().to_string();
        let error =
            resolve_leaf_preserving(&input, &context).expect_err("must reject outside path");
        assert!(
            error.contains("outside workspace root"),
            "unexpected error: {error}"
        );
    }
}
