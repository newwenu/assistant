use std::io::Write;
use std::path::{Path, PathBuf};

use crate::types::VaultError;

/// 明文导出必须携带的确认令牌。缺失或不等即拒绝导出。
pub const EXPORT_CONFIRM_TOKEN: &str = "EXPORT-PLAINTEXT";

/// 校验导出确认令牌。
///
/// 输入：前端传入的令牌字符串。
/// 返回：令牌正确时 `Ok(())`，否则 `Validation`。
/// 用途：明文出口必须由用户显式请求，任何调用方都不能绕过。
pub fn verify_export_token(token: &str) -> Result<(), VaultError> {
    if token == EXPORT_CONFIRM_TOKEN {
        Ok(())
    } else {
        Err(VaultError::Validation(
            "Plaintext export requires explicit confirmation".to_string(),
        ))
    }
}

/// 校验导出目标路径策略。
///
/// 输入：已由调用方归一过的绝对路径 + 禁止写入的目录列表（`security/`、仓库目录等）。
/// 返回：路径合法时返回原路径。
/// 失败：路径不是绝对路径、没有父目录、父目录不存在、落在禁止目录、
/// 目标已存在（禁止静默覆盖）。
///
/// 与系统级路径校验（受保护系统目录、盘符相对路径）叠加使用：
/// 调用方先做外部输入归一，再用本函数叠加仓库域约束。
pub fn validate_export_target(
    target: &Path,
    forbidden_dirs: &[&Path],
) -> Result<PathBuf, VaultError> {
    if !target.is_absolute() {
        return Err(VaultError::Validation(
            "Export path must be absolute".to_string(),
        ));
    }

    let parent = target
        .parent()
        .ok_or_else(|| VaultError::Validation("Export path has no parent directory".to_string()))?;
    if !parent.is_dir() {
        return Err(VaultError::Validation(
            "Export path parent directory does not exist".to_string(),
        ));
    }

    for dir in forbidden_dirs {
        if target.starts_with(dir) || parent.starts_with(dir) {
            return Err(VaultError::Validation(format!(
                "Export path must not be inside '{}'",
                dir.display()
            )));
        }
    }

    if target.exists() {
        return Err(VaultError::Validation(
            "Export path already exists; choose a new file".to_string(),
        ));
    }

    Ok(target.to_path_buf())
}

/// 以私有权限写入文件：同目录 `.tmp` + `rename`，不落在系统临时目录。
///
/// 输入：目标路径 + 完整内容。
/// 返回：`Ok(())` 表示目标文件已就位。
/// 失败：父目录缺失、写入失败、rename 失败；失败路径都会清理 `.tmp`，
/// 不留半截目标文件。
///
/// 权限：Unix 创建时 `0600`；Windows 依赖当前用户 ACL（不额外放宽）。
pub fn write_private_file(target: &Path, content: &[u8]) -> Result<(), VaultError> {
    let parent = target
        .parent()
        .ok_or_else(|| VaultError::Validation("Export path has no parent directory".to_string()))?;
    let file_name = target
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "vault-export".to_string());

    let tmp = parent.join(format!(".{}.{}.tmp", file_name, uuid::Uuid::new_v4()));

    let write_result = (|| -> std::io::Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0_600);
        }
        let mut file = options.open(&tmp)?;
        file.write_all(content)?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(VaultError::Io(err));
    }

    if let Err(err) = std::fs::rename(&tmp, target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(VaultError::Io(err));
    }

    Ok(())
}
