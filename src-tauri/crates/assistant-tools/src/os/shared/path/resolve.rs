use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use crate::context::ToolUseContext;

use super::membership::ensure_within_root;
use super::protected::reject_system_dirs;

/// 把工具输入解析为可安全使用的绝对路径。
///
/// 输入：原始路径字符串（绝对路径，或配合 `resolve_base` 的相对路径，
/// `resolve_base` 缺省时回退 `workspace_root`）+ 上下文。
/// 返回：经 `canonicalize_lenient` 归一、并通过受保护目录与 workspace 边界检查的绝对路径。
/// 失败：盘符相对路径（`C:foo`）、含 `..`、相对路径缺少解析基准、
/// 解析基准非绝对路径、路径不可归一、落在受保护目录、越出 workspace 根。
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

/// 可读路径的归属域：决定 `read` 用哪个动作、走哪条审批规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadableScope {
    /// 工作区内：`read` 的默认 `Allow` 覆盖。
    Workspace,
    /// 本会话工具输出目录：跨出工作区边界，恒 `Ask`（`readToolOutput`）。
    ToolOutput,
}

/// 可读路径：规范化后的绝对路径 + 它落在哪个域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadablePath {
    pub path: PathBuf,
    pub scope: ReadableScope,
}

/// `read` 专用解析：在 workspace 之外额外放行**本会话工具输出目录**（I52 §5.11.4）。
///
/// 判定顺序固定为「输出域优先于工作区域」：若两者意外重叠（配置失误），
/// 路径必须走**更严**的 `ToolOutput` 分支，不得因为「也在工作区内」而落回 `read` 的 `Allow`。
/// `reject_system_dirs` 仍在一切边界判定之前——凭据目录在任何作用域内都不可读。
///
/// 只被 `read` 调用；`write` / `edit` / `delete` / `move` 仍用 [`resolve`]，
/// 因此输出域对它们天然只读。
pub fn resolve_readable(input: &str, context: &ToolUseContext) -> Result<ReadablePath, String> {
    let joined = join_input(input, context)?;
    let canonical = canonicalize_lenient(&joined)?;
    reject_system_dirs(&canonical)?;

    if let Some(output_root) = context.tool_output_root.as_deref() {
        if ensure_within_root(&canonical, output_root).is_ok() {
            return Ok(ReadablePath {
                path: canonical,
                scope: ReadableScope::ToolOutput,
            });
        }
    }

    if let Some(root) = context.workspace_root.as_deref() {
        ensure_within_root(&canonical, Path::new(root))?;
    }
    Ok(ReadablePath {
        path: canonical,
        scope: ReadableScope::Workspace,
    })
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

/// 外部路径的用途：读入受管目录，或从受管目录写出。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalInputMode {
    Read,
    Write,
}

/// 校验受管目录之外的用户输入路径（导入源 / 导出目标）。
///
/// 输入：原始路径字符串（必须是绝对路径）+ 用途。
/// 返回：经 `canonicalize_lenient` 归一、并通过受保护目录与凭据目录检查的绝对路径。
/// 失败：非绝对路径、盘符相对路径、含 `..`、不可归一、落在受保护目录或凭据目录；
/// `Read` 要求目标是文件，`Write` 要求父目录存在且本身不是受保护目录。
///
/// 与 [`resolve`] 的区别是**不套 workspace 边界**：这些路径本来就不在工作区内。
/// 调用方必须使用返回值做 IO，不得再用原始字符串。
pub fn resolve_external_input(input: &str, mode: ExternalInputMode) -> Result<PathBuf, String> {
    let raw = Path::new(input);
    reject_drive_relative(raw, input)?;
    if !raw.is_absolute() {
        return Err(format!("external path '{}' must be absolute", input));
    }

    let canonical = canonicalize_lenient(raw)?;
    reject_system_dirs(&canonical)?;

    match mode {
        ExternalInputMode::Read => {
            let meta = std::fs::metadata(&canonical)
                .map_err(|e| format!("cannot stat '{}': {}", canonical.display(), e))?;
            if !meta.is_file() {
                return Err(format!(
                    "external path '{}' is not a regular file",
                    canonical.display()
                ));
            }
        }
        ExternalInputMode::Write => {
            let parent = canonical
                .parent()
                .ok_or_else(|| format!("external path '{}' has no parent directory", input))?;
            if !parent.is_dir() {
                return Err(format!(
                    "parent directory of '{}' does not exist",
                    canonical.display()
                ));
            }
            reject_system_dirs(parent)?;
        }
    }

    Ok(canonical)
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

pub(crate) fn join_input(input: &str, context: &ToolUseContext) -> Result<PathBuf, String> {
    let raw = Path::new(input);
    reject_drive_relative(raw, input)?;

    if raw.is_absolute() {
        return Ok(raw.to_path_buf());
    }

    let Some(root) = resolution_base(context) else {
        // 只陈述失败事实，不给调用策略：错误文本会进入模型上下文，
        // 「改用绝对路径」之类的建议等于教模型去试探边界。
        return Err(format!(
            "relative path '{}' cannot be resolved: no workspace directory is configured",
            input
        ));
    };

    let root_path = Path::new(root);
    if !root_path.is_absolute() {
        return Err(format!(
            "configured workspace root '{}' must be an absolute path",
            root
        ));
    }
    if !root_path.exists() {
        return Err(format!(
            "configured workspace root '{}' does not exist",
            root
        ));
    }

    Ok(root_path.join(raw))
}

/// 相对路径的解析基准：优先 `resolve_base`（ToolBox 只填它），回退 `workspace_root`。
///
/// 两者语义不同：`resolve_base` 只提供「从哪算起」，`workspace_root` 额外构成权限边界。
/// 缺省 `path` 参数的工具（`glob` / `grep`）默认目录也用它，不算边界。
pub(crate) fn resolution_base(context: &ToolUseContext) -> Option<&str> {
    context
        .resolve_base
        .as_deref()
        .or(context.workspace_root.as_deref())
}

/// 盘符相对路径（`C:foo`）在 Windows 上会按进程 cwd 补全，非 Windows 无此形态。
///
/// 按平台定义两份而不是在一个函数体内套 `cfg` 块：后者必须用 `let _ = (path, original)`
/// 消掉本平台用不到参数的警告，等于在签名上隐瞒「此处不参与」。
#[cfg(windows)]
fn reject_drive_relative(path: &Path, original: &str) -> Result<(), String> {
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
    Ok(())
}

#[cfg(not(windows))]
fn reject_drive_relative(_path: &Path, _original: &str) -> Result<(), String> {
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
                    "path '{}' contains '..' traversal, which is not allowed",
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
    // 归一化自身可能产出盘符相对路径：`PathBuf::push` 遇到带 prefix 的片段会清空
    // 缓冲区（`\?\G:\...` 里的 `G:` 组件即如此），结果退化成「G 盘当前目录」。
    // 该形态一旦流出，`exists()` / `canonicalize()` 会按进程 cwd 补全，拼出与工作区
    // 无关的目录（开发时是 `src-tauri`，打包后是安装目录或用户目录）。
    // 原始输入已在 `join_input` 查过一次，这里必须复查产物：校验对象与使用对象是同一个。
    reject_drive_relative(&normalized, original)?;
    Ok(normalized)
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize '{}': {}", path.display(), error))?;
    Ok(simplify_verbatim(&canonical))
}

/// 去掉 verbatim 前缀；非 UTF-8 路径原样返回。
///
/// 这里只做前缀剥离，不重新构造路径：`to_string_lossy` 得到的文本若被用来
/// `PathBuf::from`，非 UTF-8 字节会被替换字符改写，得到另一个目录（I44 §10.3）。
pub(crate) fn simplify_verbatim(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let simplified = dunce::simplified(path);
        if let Some(text) = simplified.to_str() {
            if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
                return PathBuf::from(format!(r"\\{}", rest));
            }
        }
        simplified.to_path_buf()
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}
