use std::path::Path;

use super::LinkKind;

pub(super) fn create_link(source: &Path, target: &Path, kind: LinkKind) -> Result<(), String> {
    match kind {
        LinkKind::Symlink => create_symlink(source, target),
        LinkKind::Hardlink => create_hardlink(source, target),
        #[cfg(windows)]
        LinkKind::Junction => create_junction(source, target),
    }
}

/// Unix 与 Windows 的目录 / 文件判定方式不同（Windows 需要先看目标是目录才能选
/// `symlink_dir`），因此按平台各定义一份，而不是在一个函数体内套 `cfg` 块——后者
/// 必须在「不支持」分支用 `let _ = (source, target, is_dir)` 消掉未使用参数的警告。
#[cfg(unix)]
fn create_symlink(source: &Path, target: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, source).map_err(|e| format!("symlink create failed: {}", e))
}

#[cfg(windows)]
fn create_symlink(source: &Path, target: &Path) -> Result<(), String> {
    if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, source)
            .map_err(|e| format!("symlink_dir create failed: {} (may require Developer Mode or SeCreateSymbolicLinkPrivilege)", e))
    } else {
        std::os::windows::fs::symlink_file(target, source)
            .map_err(|e| format!("symlink_file create failed: {} (may require Developer Mode or SeCreateSymbolicLinkPrivilege)", e))
    }
}

#[cfg(not(any(unix, windows)))]
fn create_symlink(_source: &Path, _target: &Path) -> Result<(), String> {
    Err("symlink not supported on this platform".to_string())
}

fn create_hardlink(source: &Path, target: &Path) -> Result<(), String> {
    if target.is_dir() {
        return Err("hardlink cannot target a directory".to_string());
    }
    std::fs::hard_link(target, source).map_err(|e| format!("hardlink create failed: {}", e))
}

#[cfg(windows)]
fn create_junction(source: &Path, target: &Path) -> Result<(), String> {
    if !target.is_dir() {
        return Err("junction can only target a directory".to_string());
    }
    if source.exists() {
        return Err(format!(
            "junction source '{}' already exists",
            source.display()
        ));
    }

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(source)
        .arg(target)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("failed to execute mklink: {}", e))?;

    if !output.status.success() {
        let stderr = crate::os::process_output::decode_process_output(&output.stderr);
        let stdout = crate::os::process_output::decode_process_output(&output.stdout);
        return Err(format!(
            "junction create failed for '{}' -> '{}' (exit code: {}, stderr: {}, stdout: {})",
            source.display(),
            target.display(),
            output.status.code().unwrap_or(-1),
            stderr,
            stdout
        ));
    }
    Ok(())
}
