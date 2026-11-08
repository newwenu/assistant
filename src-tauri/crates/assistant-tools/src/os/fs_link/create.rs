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

fn create_symlink(source: &Path, target: &Path) -> Result<(), String> {
    let is_dir = target.is_dir();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, source)
            .map_err(|e| format!("symlink create failed: {}", e))
    }
    #[cfg(windows)]
    {
        if is_dir {
            std::os::windows::fs::symlink_dir(target, source)
                .map_err(|e| format!("symlink_dir create failed: {} (may require Developer Mode or SeCreateSymbolicLinkPrivilege)", e))
        } else {
            std::os::windows::fs::symlink_file(target, source)
                .map_err(|e| format!("symlink_file create failed: {} (may require Developer Mode or SeCreateSymbolicLinkPrivilege)", e))
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (source, target, is_dir);
        Err("symlink not supported on this platform".to_string())
    }
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
        let stderr = super::super::process_output::decode_process_output(&output.stderr);
        let stdout = super::super::process_output::decode_process_output(&output.stdout);
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
