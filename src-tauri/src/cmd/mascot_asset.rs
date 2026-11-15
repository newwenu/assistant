use std::fs;
use std::path::PathBuf;
use tauri::command;

use crate::app_dirs::AppDirs;
use crate::protocol::InstallMascotAssetResultDto;
use assistant_config::atomic_write::atomic_write;
use assistant_tools::os::path::{resolve_external_input, ExternalInputMode};

const MAX_ASSET_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug)]
struct AssetSpec {
    dir_name: &'static str,
    allowed_extensions: &'static [&'static str],
    target_filename: &'static str,
}

fn get_asset_spec(asset_type: &str) -> Result<AssetSpec, String> {
    match asset_type {
        "rive" => Ok(AssetSpec {
            dir_name: "rive",
            allowed_extensions: &[".riv"],
            target_filename: "custom",
        }),
        "gif" => Ok(AssetSpec {
            dir_name: "gif",
            allowed_extensions: &[".gif", ".png", ".webp"],
            target_filename: "custom",
        }),
        "svg" => Ok(AssetSpec {
            dir_name: "svg",
            allowed_extensions: &[".svg"],
            target_filename: "custom",
        }),
        _ => Err(format!("unknown asset_type: {}", asset_type)),
    }
}

pub(crate) fn mascot_assets_dir() -> Result<PathBuf, String> {
    AppDirs::resolve()
        .map(|dirs| dirs.mascot_assets())
        .map_err(|e| e.to_string())
}

fn validate_mime(data: &[u8], asset_type: &str) -> Result<(), String> {
    match asset_type {
        "gif" => {
            if data.len() < 8 {
                return Err("file too small to identify".to_string());
            }
            let is_gif = &data[0..3] == b"GIF";
            let is_png = &data[0..8] == b"\x89PNG\r\n\x1a\n";
            let is_webp = data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP";
            if !is_gif && !is_png && !is_webp {
                return Err("not a valid image file (expected GIF, PNG, or WebP)".to_string());
            }
        }
        "svg" => {
            let head_len = data.len().min(256);
            let head_str = String::from_utf8_lossy(&data[..head_len]).to_lowercase();
            if !head_str.contains("<svg") {
                return Err("not a valid SVG file".to_string());
            }
        }
        "rive" if data.len() < 4 || &data[0..4] != b"RIVE" => {
            return Err("not a valid Rive runtime file (missing RIVE header)".to_string());
        }
        _ => {}
    }
    Ok(())
}

fn resolve_source(asset_type: &str, file_path: &str) -> Result<(AssetSpec, PathBuf), String> {
    let spec = get_asset_spec(asset_type)?;

    let src_path = resolve_external_input(file_path, ExternalInputMode::Read)
        .map_err(|e| format!("source path rejected: {}", e))?;

    let src_ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let ext_with_dot = format!(".{}", src_ext);
    if !spec.allowed_extensions.contains(&ext_with_dot.as_str()) {
        return Err(format!(
            "extension .{} not allowed for asset_type \"{}\"; allowed: {:?}",
            src_ext, asset_type, spec.allowed_extensions
        ));
    }

    Ok((spec, src_path))
}

#[command]
pub async fn install_mascot_asset(
    asset_type: String,
    file_path: String,
) -> Result<InstallMascotAssetResultDto, String> {
    let (spec, src_path) = resolve_source(&asset_type, &file_path)?;

    let src_ext = src_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let src_bytes = fs::metadata(&src_path)
        .map_err(|e| format!("metadata failed: {}", e))?
        .len();
    if src_bytes > MAX_ASSET_BYTES {
        return Err(format!(
            "file too large: {} bytes (max {} bytes)",
            src_bytes, MAX_ASSET_BYTES
        ));
    }

    let data = fs::read(&src_path).map_err(|e| format!("read failed: {}", e))?;

    validate_mime(&data, &asset_type)?;

    let assets_dir = mascot_assets_dir()?;
    let target_dir = assets_dir.join(spec.dir_name);
    let target_name = format!("{}.{}", spec.target_filename, src_ext);
    let target_path = target_dir.join(&target_name);

    atomic_write(&target_path, &data).map_err(|e| format!("write failed: {}", e))?;

    let metadata = fs::metadata(&target_path).map_err(|e| format!("metadata failed: {}", e))?;
    let version = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok(InstallMascotAssetResultDto {
        rel_path: format!("{}/{}", spec.dir_name, target_name),
        bytes: data.len() as u64,
        version,
    })
}

#[command]
pub async fn uninstall_mascot_asset(asset_type: String) -> Result<(), String> {
    let spec = get_asset_spec(&asset_type)?;
    let target_dir = mascot_assets_dir()?.join(spec.dir_name);
    for ext in spec.allowed_extensions {
        let path = target_dir.join(format!("{}{}", spec.target_filename, ext));
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }
    if asset_type == "rive" {
        let manifest_path = mascot_assets_dir()?.join("rive/custom-manifest.json");
        if manifest_path.exists() {
            let _ = fs::remove_file(&manifest_path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_asset_type() {
        let error = get_asset_spec("video").expect_err("未知素材类型必须报错");
        assert!(error.contains("unknown asset_type"), "unexpected: {error}");
    }

    #[test]
    fn rejects_extension_outside_asset_type() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let error =
            resolve_source("rive", path.to_str().unwrap()).expect_err("扩展名不在白名单必须报错");
        assert!(error.contains("not allowed"), "unexpected: {error}");
    }

    #[test]
    fn rejects_system_directory_source() {
        let path = if cfg!(windows) {
            r"C:\Windows\System32\drivers\etc\hosts"
        } else {
            "/etc/hosts"
        };
        if !std::path::Path::new(path).exists() {
            return;
        }
        let error = resolve_source("gif", path).expect_err("系统目录源必须被拒绝");
        assert!(
            error.contains("source path rejected"),
            "unexpected: {error}"
        );
    }

    #[test]
    fn rejects_relative_source() {
        let error = resolve_source("gif", "assets/custom.gif").expect_err("相对路径必须被拒绝");
        assert!(
            error.contains("source path rejected"),
            "unexpected: {error}"
        );
    }
}
