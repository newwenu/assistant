use std::path::{Path, PathBuf};

use super::component_prefix_match;
use super::resolve::canonicalize_lenient;

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

    for component in &components {
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
                        path.display(),
                        next_name
                    ));
                }
            }
        }
    }

    Ok(())
}
