use std::path::Path;

/// 非空目录拒绝：只读第一项，不整目录列举。`remedy` 由调用方给出各自的替代做法。
pub(crate) fn ensure_empty_directory(path: &Path, remedy: &str) -> Result<(), String> {
    let mut entries = std::fs::read_dir(path)
        .map_err(|error| format!("cannot read '{}': {}", path.display(), error))?;
    if entries.next().is_some() {
        return Err(format!(
            "'{}' is a non-empty directory; {}",
            path.display(),
            remedy
        ));
    }
    Ok(())
}
