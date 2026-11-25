mod args;
mod membership;
mod protected;
mod r#ref;
mod resolve;

use std::path::{Component, Path};

pub use args::{parse_optional_path_arg, parse_path_arg};
pub use membership::{ensure_within_root, workspace_membership, WorkspaceMembership};
pub use protected::reject_system_dirs;
pub use r#ref::{
    parse_ref, tool_output_ref, write_rejected, PathRef, Scheme, SchemeNature, WriteAlternate,
    BROWSE_REJECTED, CWD_REJECTED, SCRATCH_SCHEME, TOOL_OUTPUT_SCHEME,
};
pub(crate) use resolve::resolution_base;
pub use resolve::{
    canonicalize_lenient, resolve_external_input, ExternalInputMode, ReadablePath, ReadableScope,
};

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

/// 组件相等判定：Windows 上大小写不敏感。
///
/// 不用 `to_string_lossy` 做归一：两条不同的非 UTF-8 组件会被塌缩成同一串，
/// 使 `component_prefix_match` 把「不在工作区内」判成「在工作区内」（I44 §10.3
/// 禁止 lossy 写入比较键）。非 UTF-8 组件退化为按字节精确比较，方向只会更严。
fn component_eq(left: &Component, right: &Component) -> bool {
    #[cfg(windows)]
    {
        match (left.as_os_str().to_str(), right.as_os_str().to_str()) {
            (Some(a), Some(b)) => a.to_lowercase() == b.to_lowercase(),
            _ => left.as_os_str() == right.as_os_str(),
        }
    }
    #[cfg(not(windows))]
    {
        left.as_os_str() == right.as_os_str()
    }
}

#[cfg(test)]
mod tests {
    use super::resolve::{resolve, resolve_leaf_preserving, resolve_readable};
    use super::*;
    use crate::context::ToolUseContext;
    use std::path::PathBuf;

    /// Agent 链路形态：只填 `workspace_root`，相对路径基准由 `resolution_base` 回退提供。
    fn context_with_root(root: &Path) -> ToolUseContext {
        ToolUseContext {
            workspace_root: Some(root.display().to_string()),
            ..Default::default()
        }
    }

    /// ToolBox 形态：只给解析基准，没有权限边界（I18b §2.2）。
    fn toolbox_context(base: &Path) -> ToolUseContext {
        ToolUseContext::for_toolbox(Some(base.display().to_string()))
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
            resolve::simplify_verbatim(Path::new(r"\\?\UNC\server\share\dir\file.txt")),
            Path::new(r"\\server\share\dir\file.txt")
        );
        assert_eq!(
            resolve::simplify_verbatim(Path::new(r"\\?\C:\Windows")),
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

    /// `\?\C:\...` 少一个反斜杠：组件层面只是「根相对路径」，不是合法 verbatim 前缀。
    ///
    /// 若放行，`join_input` 会与工作区根拼接，`normalize_components` 再把其中的 `C:`
    /// 当盘符前缀重新压入缓冲区（`PathBuf::push` 清空已累积内容），产物退化为盘符
    /// 相对路径，随后 `exists()` / `canonicalize()` 按进程 cwd 补全 —— 开发时拼出
    /// `src-tauri\...`，打包后拼出安装目录或用户目录。修复点在归一化出口复查形态。
    #[cfg(windows)]
    #[test]
    fn malformed_verbatim_prefix_is_not_resolved_against_cwd() {
        let dir = tempfile::tempdir().expect("tempdir");
        let context = context_with_root(dir.path());
        let root = canonicalize_lenient(dir.path()).expect("canonical root");
        let root_text = root.to_string_lossy().to_string();
        let Some((drive, rest)) = root_text.split_once(":\\") else {
            return;
        };

        let malformed = format!("\\?\\{drive}:\\{rest}");
        let error = resolve(&malformed, &context).expect_err("must reject malformed verbatim");
        assert!(
            error.contains("relative path with drive prefix is not supported"),
            "unexpected error: {error}"
        );
        assert!(
            !error.contains("src-tauri"),
            "error must not leak a cwd-derived path: {error}"
        );

        let verbatim = format!("\\\\?\\{root_text}");
        let resolved = resolve(&verbatim, &context).expect("correct verbatim form still resolves");
        assert_eq!(
            resolved, root,
            "verbatim form must resolve to the workspace root"
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
        let joined = resolve::join_input(r"\\server\share\file.txt", &context)
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
    fn toolbox_base_resolves_relative_paths_without_creating_a_boundary() {
        let base_dir = tempfile::tempdir().expect("tempdir");
        let other_dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(base_dir.path().join("inside.txt"), b"x").expect("write file");
        std::fs::write(other_dir.path().join("outside.txt"), b"x").expect("write file");

        let context = toolbox_context(base_dir.path());
        let relative = resolve("inside.txt", &context).expect("relative path resolves via base");
        assert_eq!(
            relative,
            canonicalize_lenient(base_dir.path())
                .expect("canonical base")
                .join("inside.txt")
        );

        let outside = other_dir.path().join("outside.txt").display().to_string();
        let resolved = resolve(&outside, &context).expect("ToolBox has no boundary");
        assert_eq!(
            resolved,
            canonicalize_lenient(other_dir.path())
                .expect("canonical other")
                .join("outside.txt")
        );
        assert_eq!(
            workspace_membership(&resolved, &context),
            WorkspaceMembership::Unconfigured,
            "ToolBox 只给解析基准，不产生「在区内/越界」结论"
        );
    }

    #[test]
    fn differing_base_and_boundary_fails_closed() {
        let base_dir = tempfile::tempdir().expect("tempdir");
        let boundary_dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(base_dir.path().join("a.txt"), b"x").expect("write file");

        let context = ToolUseContext {
            workspace_root: Some(boundary_dir.path().display().to_string()),
            resolve_base: Some(base_dir.path().display().to_string()),
            ..Default::default()
        };

        let error =
            resolve("a.txt", &context).expect_err("基准在边界外时必须拒绝，不能只按基准放行");
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
    fn resolve_readable_marks_workspace_and_output_scopes() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let output = tempfile::tempdir().expect("tempdir");
        std::fs::write(workspace.path().join("a.txt"), b"x").expect("write workspace file");
        std::fs::write(output.path().join("out.txt"), b"y").expect("write output file");

        let context = ToolUseContext {
            workspace_root: Some(workspace.path().display().to_string()),
            tool_output_root: Some(output.path().to_path_buf()),
            ..Default::default()
        };

        let inside = resolve_readable("a.txt", &context).expect("workspace file is readable");
        assert_eq!(inside.scope, ReadableScope::Workspace);
        assert_eq!(
            inside.path,
            canonicalize_lenient(&workspace.path().join("a.txt")).expect("canonical")
        );

        let spilled = resolve_readable(
            &output.path().join("out.txt").display().to_string(),
            &context,
        )
        .expect("output file is readable outside the workspace");
        assert_eq!(spilled.scope, ReadableScope::ToolOutput);
    }

    #[test]
    fn resolve_readable_rejects_foreign_output_namespace() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let session = tempfile::tempdir().expect("tempdir");
        let other_session = tempfile::tempdir().expect("tempdir");
        std::fs::write(other_session.path().join("secret.txt"), b"z").expect("write file");

        let context = ToolUseContext {
            workspace_root: Some(workspace.path().display().to_string()),
            tool_output_root: Some(session.path().to_path_buf()),
            ..Default::default()
        };

        // 越域必须在路径判定阶段被拒，不能落到「弹个框让用户批准」。
        let error = resolve_readable(
            &other_session
                .path()
                .join("secret.txt")
                .display()
                .to_string(),
            &context,
        )
        .expect_err("cross-session output must be rejected");
        assert!(
            error.contains("outside workspace root"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn resolve_readable_prefers_output_scope_over_workspace() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nested_output = dir.path().join("outputs");
        std::fs::create_dir(&nested_output).expect("create nested output");
        std::fs::write(nested_output.join("run.log"), b"log").expect("write file");

        // 输出域与工作区重叠（配置失误）：必须走更严的 `ToolOutput` 分支，
        // 不得因为「也在工作区内」而落回 `read` 的默认 `Allow`。
        let context = ToolUseContext {
            workspace_root: Some(dir.path().display().to_string()),
            tool_output_root: Some(nested_output.clone()),
            ..Default::default()
        };
        let resolved = resolve_readable(
            &nested_output.join("run.log").display().to_string(),
            &context,
        )
        .expect("nested output resolves");
        assert_eq!(resolved.scope, ReadableScope::ToolOutput);
    }

    #[test]
    fn resolve_readable_without_output_root_keeps_workspace_boundary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let other = tempfile::tempdir().expect("tempdir");
        std::fs::write(other.path().join("b.txt"), b"x").expect("write file");
        let context = context_with_root(dir.path());
        let error = resolve_readable(&other.path().join("b.txt").display().to_string(), &context)
            .expect_err("no output root means no extra allowance");
        assert!(
            error.contains("outside workspace root"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn workspace_membership_matches_the_boundary_check() {
        let dir = tempfile::tempdir().expect("tempdir");
        // `workspace_membership` 的契约是「输入已归一」（macOS 上 `/var` → `/private/var`）。
        let inside = canonicalize_lenient(&dir.path().join("a.txt")).expect("canonical inside");
        std::fs::write(&inside, b"x").expect("write file");
        let other = tempfile::tempdir().expect("tempdir");
        let outside = canonicalize_lenient(&other.path().join("b.txt")).expect("canonical outside");

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
        if !crate::os::test_support::link::dir_link(&real, &link) {
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
            error.contains("cannot be resolved"),
            "unexpected error: {error}"
        );
        // 错误文本会进入模型上下文，不得携带「改用绝对路径」这类调用策略。
        assert!(
            !error.contains("instead") && !error.contains("absolute path"),
            "error must not suggest a workaround: {error}"
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
            error.contains("workspace root") && error.contains("does not exist"),
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

    #[test]
    fn external_input_rejects_relative_and_traversal() {
        let error = resolve_external_input("relative.txt", ExternalInputMode::Read)
            .expect_err("相对路径必须拒绝");
        assert!(
            error.contains("must be absolute"),
            "unexpected error: {error}"
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let traversal = dir
            .path()
            .join("child")
            .join("..")
            .join("file.txt")
            .display()
            .to_string();
        let error =
            resolve_external_input(&traversal, ExternalInputMode::Read).expect_err("'..' 必须拒绝");
        assert!(error.contains(".."), "unexpected error: {error}");
    }

    #[test]
    fn external_input_read_requires_a_regular_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.txt").display().to_string();
        assert!(resolve_external_input(&missing, ExternalInputMode::Read).is_err());

        let as_dir = dir.path().display().to_string();
        assert!(resolve_external_input(&as_dir, ExternalInputMode::Read).is_err());

        let file = dir.path().join("ok.txt");
        std::fs::write(&file, b"x").expect("write file");
        let resolved = resolve_external_input(&file.display().to_string(), ExternalInputMode::Read)
            .expect("普通文件应通过");
        assert_eq!(resolved, canonicalize_lenient(&file).expect("canonical"));
    }

    #[test]
    fn external_input_write_requires_existing_parent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing_parent = dir
            .path()
            .join("missing")
            .join("out.txt")
            .display()
            .to_string();
        let error = resolve_external_input(&missing_parent, ExternalInputMode::Write)
            .expect_err("父目录不存在必须拒绝");
        assert!(
            error.contains("does not exist"),
            "unexpected error: {error}"
        );

        let ok = dir.path().join("out.txt").display().to_string();
        assert!(resolve_external_input(&ok, ExternalInputMode::Write).is_ok());
    }

    #[test]
    fn external_input_rejects_protected_and_credential_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let creds = dir.path().join(".ssh");
        std::fs::create_dir(&creds).expect("create .ssh");
        let target = creds.join("id_rsa").display().to_string();
        let error = resolve_external_input(&target, ExternalInputMode::Write)
            .expect_err("凭据目录必须拒绝");
        assert!(error.contains("credential"), "unexpected error: {error}");

        if let Some(protected) = protected_root_for_test() {
            let target = protected.join("child.txt");
            assert!(
                resolve_external_input(&target.display().to_string(), ExternalInputMode::Write)
                    .is_err(),
                "受保护系统目录必须拒绝"
            );
        }
    }

    /// A4：输出域引用只接受单个文件名，其余形态在格式层拒绝（I73 §7）。
    #[test]
    fn a_tool_output_reference_rejects_anything_but_a_file_name() {
        for raw in [
            "tool-output://../x",
            "tool-output://a/b",
            "tool-output://C:\\x",
            "tool-output://",
            "tool-output:// x",
        ] {
            let error = parse_ref(raw).expect_err(raw);
            assert!(
                error.contains("invalid path reference"),
                "{} -> {}",
                raw,
                error
            );
        }
    }

    /// A6：绕过 `PathRef` 把引用直接喂给解析层时，最低层拒绝（I73 §3.3 E1）。
    #[test]
    fn a_reference_that_bypasses_path_ref_is_rejected_at_the_lowest_layer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ctx = context_with_root(dir.path());

        for raw in ["tool-output://x", "scratch://logs/a.log"] {
            let error = resolve(raw, &ctx).expect_err(raw);
            assert!(
                error.contains("must be resolved through PathRef"),
                "{} -> {}",
                raw,
                error
            );
        }
    }

    /// A21：引用 body 不得是 verbatim 形态——拒绝，而不是剥离后当成合法路径（I73 §3.9 / N17）。
    #[test]
    fn reference_bodies_reject_verbatim_prefixes() {
        for raw in [
            "tool-output://\\\\?\\C:\\x",
            "tool-output:////?/C:/x",
            "scratch://\\\\?\\C:\\x",
            "scratch:////?/C:/x",
        ] {
            let error = parse_ref(raw).expect_err(raw);
            assert!(
                error.contains("invalid path reference"),
                "{} -> {}",
                raw,
                error
            );
        }

        // 无 scheme 的 verbatim 串不是引用，仍按原规则解析成功。
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("ok.txt");
        std::fs::write(&file, b"x").expect("write file");
        let ctx = context_with_root(dir.path());
        let verbatim = format!("\\\\?\\{}", file.display());
        assert!(parse_ref(&verbatim).is_ok(), "非引用串必须原样通过");
        assert_eq!(
            resolve(&verbatim, &ctx).expect("verbatim 串必须可解析"),
            canonicalize_lenient(&file).expect("canonical")
        );
    }
}
