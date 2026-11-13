use std::path::{Path, PathBuf};
use tauri::{command, AppHandle, Manager};

use crate::app_dirs::AppDirs;
use crate::protocol::{AssetDirsDto, AttachmentImportDto, AttachmentLimitsDto};
use assistant_agent::multimodal::ATTACHMENT_LIMITS;
use assistant_storage::attachment_store::AttachmentStore;
use assistant_tools::os::path::{resolve_external_input, ExternalInputMode};

/// 与 `src/chat/attachment/types.ts` 的白名单保持一致；后端判定为准。
fn media_type_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" | "jfif" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        "pdf" => Some("application/pdf"),
        "txt" | "log" => Some("text/plain"),
        "md" => Some("text/markdown"),
        "csv" => Some("text/csv"),
        "html" => Some("text/html"),
        "json" => Some("application/json"),
        "py" => Some("text/x-python"),
        "js" => Some("text/javascript"),
        "ts" | "tsx" => Some("text/typescript"),
        "css" => Some("text/css"),
        "yaml" | "yml" => Some("text/yaml"),
        "xml" => Some("text/xml"),
        "rs" => Some("text/x-rust"),
        "toml" => Some("text/x-toml"),
        "java" => Some("text/x-java"),
        "go" => Some("text/x-go"),
        "c" | "h" => Some("text/x-c"),
        "cpp" => Some("text/x-cpp"),
        "sh" => Some("text/x-shellscript"),
        "ps1" => Some("text/x-powershell"),
        "sql" => Some("application/sql"),
        _ => None,
    }
}

/// 错误串直接使用前端 `AdmitError` 键，前端据此提示，不解析后端文案。
const IMPORT_FAILED: &str = "import_failed";

fn utf8_path(path: &Path, field: &str) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("{} is not valid UTF-8", field))
}

fn asset_dirs_from(dirs: &AppDirs) -> Result<AssetDirsDto, String> {
    Ok(AssetDirsDto {
        attachments_dir: utf8_path(&dirs.attachments(), "attachmentsDir")?,
        mascot_assets_dir: utf8_path(&dirs.mascot_assets(), "mascotAssetsDir")?,
    })
}

#[command]
pub fn get_asset_dirs() -> Result<AssetDirsDto, String> {
    let dirs = AppDirs::resolve().map_err(|e| e.to_string())?;
    asset_dirs_from(&dirs)
}

#[command]
pub fn get_attachment_limits() -> AttachmentLimitsDto {
    AttachmentLimitsDto {
        max_images: ATTACHMENT_LIMITS.max_images as u32,
        max_files: ATTACHMENT_LIMITS.max_files as u32,
        max_image_bytes: ATTACHMENT_LIMITS.max_image_bytes,
        max_file_bytes: ATTACHMENT_LIMITS.max_file_bytes,
    }
}

#[derive(Debug)]
struct ImportPlan {
    path: PathBuf,
    file_name: String,
    media_type: &'static str,
}

/// 校验导入源并给出入库所需的全部信息；不做 IO 读取与写入。
fn plan_import(path: &str) -> Result<ImportPlan, String> {
    let resolved = resolve_external_input(path, ExternalInputMode::Read)
        .map_err(|_| IMPORT_FAILED.to_string())?;

    let file_name = resolved
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| IMPORT_FAILED.to_string())?;

    let ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let media_type =
        media_type_for_extension(&ext).ok_or_else(|| "invalid_file_type".to_string())?;
    let is_image = media_type.starts_with("image/");

    let bytes = std::fs::metadata(&resolved)
        .map_err(|_| IMPORT_FAILED.to_string())?
        .len();
    let limit = if is_image {
        ATTACHMENT_LIMITS.max_image_bytes
    } else {
        ATTACHMENT_LIMITS.max_file_bytes
    };
    if bytes > limit {
        return Err(if is_image {
            "too_large".to_string()
        } else {
            "file_too_large".to_string()
        });
    }

    Ok(ImportPlan {
        path: resolved,
        file_name,
        media_type,
    })
}

#[command]
pub async fn import_attachment(
    app: AppHandle,
    path: String,
) -> Result<AttachmentImportDto, String> {
    let plan = plan_import(&path)?;

    let data = std::fs::read(&plan.path).map_err(|_| IMPORT_FAILED.to_string())?;

    let saved = app
        .state::<AttachmentStore>()
        .save_attachment(&data, plan.media_type)
        .await
        .map_err(|_| IMPORT_FAILED.to_string())?;

    Ok(AttachmentImportDto {
        attachment_id: saved.id,
        media_type: saved.media_type,
        bytes: saved.bytes,
        file_name: plan.file_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn maps_all_frontend_extensions() {
        for ext in [
            "png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp", "tif", "tiff", "pdf", "txt", "md",
            "csv", "html", "json", "py", "js", "ts", "tsx", "css", "yaml", "yml", "xml",
        ] {
            assert!(
                media_type_for_extension(ext).is_some(),
                "缺少扩展名映射: {ext}"
            );
        }
        assert!(media_type_for_extension("exe").is_none());
        assert!(media_type_for_extension("").is_none());
    }

    #[test]
    fn asset_dirs_report_roots_without_lossy_fallback() {
        let data = PathBuf::from("/data/root");
        let local = PathBuf::from("/local/root");
        let dirs = AppDirs::with_bases(data.clone(), local);
        let dto = asset_dirs_from(&dirs).expect("utf-8 bases");
        assert_eq!(
            dto.attachments_dir,
            data.join("attachments").to_str().unwrap()
        );
        assert_eq!(
            dto.mascot_assets_dir,
            data.join("mascot-assets").to_str().unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn asset_dirs_reject_non_utf8_roots() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let data = PathBuf::from(OsStr::from_bytes(b"/data/\xff"));
        let local = PathBuf::from("/local/root");
        let dirs = AppDirs::with_bases(data, local);
        let error = asset_dirs_from(&dirs).expect_err("非 UTF-8 基根必须报错");
        assert!(error.contains("UTF-8"), "unexpected error: {error}");
        assert!(!error.contains('\u{fffd}'), "错误信息不得输出 lossy 路径");
    }

    fn scratch(name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("创建临时目录");
        let file = dir.path().join(name);
        (dir, file)
    }

    #[test]
    fn rejects_unknown_extension_at_import() {
        let (_dir, file) = scratch("payload.lock");
        std::fs::write(&file, b"x").expect("写入临时文件");
        let error = plan_import(file.to_str().unwrap()).expect_err("未知扩展名必须拒绝");
        assert_eq!(error, "invalid_file_type");
    }

    #[test]
    fn rejects_missing_source_at_import() {
        let (_dir, missing) = scratch("gone.png");
        let error = plan_import(missing.to_str().unwrap()).expect_err("不存在的源必须拒绝");
        assert_eq!(error, IMPORT_FAILED);
    }

    #[test]
    fn rejects_oversized_image_at_import() {
        let (_dir, file) = scratch("huge.png");
        let handle = std::fs::File::create(&file).expect("创建临时文件");
        handle
            .set_len(ATTACHMENT_LIMITS.max_image_bytes + 1)
            .expect("扩展文件长度");
        drop(handle);
        let error = plan_import(file.to_str().unwrap()).expect_err("超限图片必须拒绝");
        assert_eq!(error, "too_large");
    }

    #[test]
    fn rejects_oversized_document_at_import() {
        let (_dir, file) = scratch("huge.pdf");
        let handle = std::fs::File::create(&file).expect("创建临时文件");
        handle
            .set_len(ATTACHMENT_LIMITS.max_file_bytes + 1)
            .expect("扩展文件长度");
        drop(handle);
        let error = plan_import(file.to_str().unwrap()).expect_err("超限文档必须拒绝");
        assert_eq!(error, "file_too_large");
    }

    #[test]
    fn accepts_small_document_at_import() {
        let (_dir, file) = scratch("note.md");
        std::fs::write(&file, b"hello").expect("写入临时文件");
        let plan = plan_import(file.to_str().unwrap()).expect("小文件应通过");
        assert_eq!(plan.file_name, "note.md");
        assert_eq!(plan.media_type, "text/markdown");
    }
}
