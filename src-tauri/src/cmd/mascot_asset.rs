use std::fs;
use std::path::{Path, PathBuf};
use tauri::command;

use crate::app_dirs::AppDirs;
use crate::protocol::{InstallMascotAssetResultDto, RiveManifestDto};
use assistant_config::atomic_write::atomic_write;
use assistant_tools::os::path::{resolve_external_input, ExternalInputMode};

const MAX_ASSET_BYTES: u64 = 2 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MANIFEST_FILENAME: &str = "custom-manifest.json";

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

/// 伴生声明的路径：`<mascot-assets>/<dir>/custom-manifest.json`。
/// 与素材同目录，卸载素材时一并删除。
fn manifest_path(asset_type: &str) -> Result<PathBuf, String> {
    let spec = get_asset_spec(asset_type)?;
    Ok(mascot_assets_dir()?
        .join(spec.dir_name)
        .join(MANIFEST_FILENAME))
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

/// 读取素材源文件同目录的 `custom-manifest.json`。
/// 文件不存在或为空视为“无声明”，返回 `None`；存在但非法则报错（安装整体失败，
/// 不让新素材带着旧声明落地）。
fn read_sibling_manifest(src_path: &Path) -> Result<Option<RiveManifestDto>, String> {
    let Some(path) = src_path.parent().map(|dir| dir.join(MANIFEST_FILENAME)) else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let path_str = path
        .to_str()
        .ok_or_else(|| format!("manifest path not valid UTF-8: {}", path.display()))?;
    let resolved = resolve_external_input(path_str, ExternalInputMode::Read)
        .map_err(|e| format!("manifest path rejected: {}", e))?;
    let bytes = fs::metadata(&resolved)
        .map_err(|e| format!("manifest metadata failed: {}", e))?
        .len();
    if bytes > MAX_MANIFEST_BYTES {
        return Err(format!(
            "manifest too large: {} bytes (max {} bytes)",
            bytes, MAX_MANIFEST_BYTES
        ));
    }
    let text = fs::read_to_string(&resolved).map_err(|e| format!("manifest read failed: {}", e))?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<RiveManifestDto>(&text)
        .map(Some)
        .map_err(|e| format!("manifest invalid: {}", e))
}

/// 把声明落到素材目录：有声明则写入，无声明则清除旧文件——
/// 旧素材的意图不能漂移到新素材上。
fn apply_manifest(target: &Path, manifest: Option<&RiveManifestDto>) -> Result<(), String> {
    match manifest {
        Some(manifest) => {
            let text = serde_json::to_string_pretty(manifest)
                .map_err(|e| format!("manifest encode failed: {}", e))?;
            atomic_write(target, text.as_bytes())
                .map_err(|e| format!("manifest write failed: {}", e))
        }
        None => {
            if target.exists() {
                let _ = fs::remove_file(target);
            }
            Ok(())
        }
    }
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

    let sibling_manifest = if asset_type == "rive" {
        read_sibling_manifest(&src_path)?
    } else {
        None
    };

    let assets_dir = mascot_assets_dir()?;
    let target_dir = assets_dir.join(spec.dir_name);
    let target_name = format!("{}.{}", spec.target_filename, src_ext);
    let target_path = target_dir.join(&target_name);

    atomic_write(&target_path, &data).map_err(|e| format!("write failed: {}", e))?;

    if asset_type == "rive" {
        apply_manifest(&manifest_path(&asset_type)?, sibling_manifest.as_ref())?;
    }

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
    let manifest = manifest_path(&asset_type)?;
    if manifest.exists() {
        let _ = fs::remove_file(&manifest);
    }
    Ok(())
}

/// 读取自定义素材的伴生声明。文件不存在或为空视为“无声明”，由前端回退到运行时自省。
#[command]
pub async fn get_mascot_manifest(asset_type: String) -> Result<Option<RiveManifestDto>, String> {
    let path = manifest_path(&asset_type)?;
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("manifest read failed: {}", e))?;
    if text.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<RiveManifestDto>(&text)
        .map(Some)
        .map_err(|e| format!("manifest invalid: {}", e))
}

/// 写入自定义素材的伴生声明。整体替换，不做字段级合并。
/// 空声明等价于“清除”：删掉文件，不留空壳，读取侧据此回退到运行时自省。
#[command]
pub async fn set_mascot_manifest(
    asset_type: String,
    manifest: RiveManifestDto,
) -> Result<(), String> {
    let path = manifest_path(&asset_type)?;
    if manifest_is_empty(&manifest) {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("manifest remove failed: {}", e))?;
        }
        return Ok(());
    }
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("manifest encode failed: {}", e))?;
    atomic_write(&path, text.as_bytes()).map_err(|e| format!("manifest write failed: {}", e))
}

fn manifest_is_empty(manifest: &RiveManifestDto) -> bool {
    manifest == &RiveManifestDto::default()
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

    #[test]
    fn parses_manifest_with_camel_case_keys() {
        let manifest: RiveManifestDto = serde_json::from_str(
            r#"{
                "stateMachine": "MascotSM",
                "properties": { "pose": "mood", "viseme": "lipSync" },
                "poses": ["Idle", "Happy"],
                "faceToPose": { "happy": "Happy" },
                "idlePoseCycle": ["Happy"]
            }"#,
        )
        .expect("合法声明必须能解析");
        assert_eq!(manifest.state_machine.as_deref(), Some("MascotSM"));
        let face_to_pose = manifest.face_to_pose.expect("faceToPose 已声明");
        assert_eq!(face_to_pose.get("happy").map(String::as_str), Some("Happy"));
    }

    #[test]
    fn rejects_manifest_with_unknown_fields() {
        let error = serde_json::from_str::<RiveManifestDto>(r#"{"poses":[],"typo":1}"#)
            .expect_err("未知字段必须报错，避免拼写错误被静默忽略");
        assert!(
            error.to_string().contains("unknown field"),
            "unexpected: {error}"
        );
    }

    #[test]
    fn empty_object_manifest_is_cleared() {
        let manifest: RiveManifestDto = serde_json::from_str("{}").expect("空对象必须能解析");
        assert!(manifest_is_empty(&manifest), "空对象应视为清除声明");
        let manifest: RiveManifestDto =
            serde_json::from_str(r#"{"poses":["Idle"]}"#).expect("合法声明必须能解析");
        assert!(!manifest_is_empty(&manifest), "带字段的声明不得被当作清除");
    }
}
