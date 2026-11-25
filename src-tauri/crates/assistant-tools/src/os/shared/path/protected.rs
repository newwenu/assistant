use std::path::{Path, PathBuf};

use super::component_prefix_match;

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
    reject_system_dirs_as(path, &path.display().to_string())
}

/// 与 [`reject_system_dirs`] 相同，但错误文本用调用方给的 `label`。
///
/// 引用（`tool-output://` / `scratch://`）命中受保护目录时必须回显原串，不得把域根
/// 物理路径写进模型可见文本（I73 §3.10 D1 / D2）；其余调用点用 1 参版本，行为不变。
pub fn reject_system_dirs_as(path: &Path, label: &str) -> Result<(), String> {
    // 符号链接（macOS `/etc` `/tmp` `/var` → `/private`，Debian `/bin` `/sbin` → `/usr`）
    // 只按字面比较会漏判，故两侧都取归一结果；归一失败保留原形态，方向只会更严。
    let canonical =
        super::resolve::canonicalize_lenient(path).unwrap_or_else(|_| path.to_path_buf());
    for protected in protected_dirs() {
        if protected_prefix_hit(path, &canonical, &protected) {
            return Err(format!(
                "path '{}' is inside a protected system directory '{}'",
                label,
                protected.display()
            ));
        }
    }
    reject_credential_dirs(path, label)?;
    if canonical.as_path() != path {
        reject_credential_dirs(&canonical, label)?;
    }
    Ok(())
}

/// 受保护目录的组件级前缀判定。
///
/// macOS 上不能直接比真实路径：`/Users`、`/home`、`/Library` 的真实路径挂在
/// 数据卷 `/System/Volumes/Data` 下，按真实路径比会让 `/System` 命中每个家目录。
/// 故 macOS 先把两侧还原成逻辑形态再比，其余平台直接比较归一结果。
fn protected_prefix_hit(path: &Path, canonical: &Path, protected: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        let data_volume_root = "/System/Volumes/Data";
        let logical = |value: &Path| match value.strip_prefix(data_volume_root) {
            Ok(rest) => Path::new("/").join(rest),
            Err(_) => value.to_path_buf(),
        };
        return component_prefix_match(&logical(path), &logical(protected))
            || component_prefix_match(&logical(canonical), &logical(protected));
    }
    #[cfg(not(target_os = "macos"))]
    {
        component_prefix_match(path, protected) || component_prefix_match(canonical, protected)
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
            match super::resolve::canonicalize_lenient(Path::new(&value)) {
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
            .map(|entry| {
                let path = PathBuf::from(entry);
                super::resolve::canonicalize_lenient(&path).unwrap_or(path)
            })
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

fn reject_credential_dirs(path: &Path, label: &str) -> Result<(), String> {
    let components: Vec<&std::ffi::OsStr> = path
        .components()
        .map(|component| component.as_os_str())
        .collect();

    for component in &components {
        let name = component.to_string_lossy();
        if CREDENTIAL_DIR_COMPONENTS
            .iter()
            .any(|dir| name.eq_ignore_ascii_case(dir))
        {
            return Err(format!(
                "path '{}' is inside a protected credential directory '{}'",
                label, name
            ));
        }
    }

    // `Microsoft\Protect\...` 形态只在 Windows 上成立，且要读相邻组件，故单独遍历。
    #[cfg(windows)]
    for (index, component) in components.iter().enumerate() {
        let name = component.to_string_lossy();
        if name.eq_ignore_ascii_case("Microsoft") {
            if let Some(next) = components.get(index + 1) {
                let next_name = next.to_string_lossy();
                if MICROSOFT_CREDENTIAL_DIR_COMPONENTS
                    .iter()
                    .any(|dir| next_name.eq_ignore_ascii_case(dir))
                {
                    return Err(format!(
                        "path '{}' is inside a protected credential directory 'Microsoft\\{}'",
                        label, next_name
                    ));
                }
            }
        }
    }

    Ok(())
}
