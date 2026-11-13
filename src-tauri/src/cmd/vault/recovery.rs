use std::path::PathBuf;

use tauri::{command, AppHandle};

use assistant_config::vault_conf::VaultConf;
use assistant_sync::git_sync::GitSync;
use assistant_sync::types::SyncConfig;
use assistant_vault::dto::{
    PaperErrorPosDto, RecoveryBackupDto, RecoveryEntryDto, RecoveryExportDto, RecoveryKeyDto,
    RecoverySkipDto, VaultErrorDto,
};
use assistant_vault::vault_file::VaultFileManager;

/// 恢复副本的生命周期守卫：无论成功、失败还是提前返回，都删除 `<security>/recovery/`。
struct RecoveryDirGuard {
    security_root: PathBuf,
}

impl Drop for RecoveryDirGuard {
    fn drop(&mut self) {
        if let Err(e) = super::cleanup_recovery_dir(&self.security_root) {
            log::warn!("Failed to clean recovery directory: {}", e);
        }
    }
}

/// 取恢复用的仓库地址：优先用本次传入的值，否则回退到已保存的同步远端。
fn resolve_recovery_url(git_url: Option<String>) -> Result<String, VaultErrorDto> {
    if let Some(url) = git_url
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty())
    {
        return Ok(url);
    }
    VaultConf::load()
        .ok()
        .and_then(|c| c.vault_sync_remote_url)
        .filter(|u| !u.trim().is_empty())
        .ok_or_else(|| {
            super::validation_error(
                "Paper recovery requires a repository URL to read a fresh copy".to_string(),
            )
        })
}

/// clone 一份仓库副本到恢复目录，供恢复读取。
///
/// 调用方负责在 clone 前清空目录，并在作用域结束时通过 `RecoveryDirGuard` 删除副本。
fn clone_recovery_copy(target: &std::path::Path, url: &str) -> Result<(), VaultErrorDto> {
    let git = GitSync::new(
        target.to_path_buf(),
        SyncConfig {
            remote_url: Some(url.to_string()),
            ..Default::default()
        },
    );
    git.clone(url, true).map_err(|e| {
        log::warn!("Recovery clone failed: {}", e);
        VaultErrorDto::from(e)
    })
}

// precondition: Unlocked
#[command]
pub async fn vault_export_recovery_key(
    app: AppHandle,
    master_password: String,
) -> Result<RecoveryKeyDto, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = super::repo_state(&app)?;
        super::with_unlocked(&app, |state| {
            assistant_vault::lifecycle::export_recovery_key(state, repo.manager(), &master_password)
                .map_err(|e| {
                    log::warn!("Recovery key export failed: {}", e);
                    VaultErrorDto::from(e)
                })
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("vault_export_recovery_key task failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub async fn vault_recover_from_paper(
    app: AppHandle,
    paper_key: String,
    git_url: Option<String>,
) -> Result<RecoveryExportDto, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let url = resolve_recovery_url(git_url)?;

        let dirs = crate::app_dirs::AppDirs::resolve()
            .map_err(|e| super::internal_error(format!("Failed to resolve app dirs: {}", e)))?;
        let security_root = super::security_dir(&dirs);
        let recovery_root = super::recovery_dir(&dirs);

        // 守卫先于 clone 建立：clone 失败、恢复失败、提前返回都会删除副本。
        let _guard = RecoveryDirGuard {
            security_root: security_root.clone(),
        };
        super::cleanup_recovery_dir(&security_root).map_err(|e| {
            super::internal_error(format!("Failed to prepare recovery directory: {}", e))
        })?;
        clone_recovery_copy(&recovery_root, &url)?;

        let repo = VaultFileManager::new(recovery_root, Box::new(super::cipher_algo));
        let session = assistant_vault::recovery_key::open_recovery_session(&repo, &paper_key)
            .map_err(|e| {
                log::warn!("Paper recovery failed: {}", e);
                VaultErrorDto::from(e)
            })?;

        let export = session.export_all().map_err(VaultErrorDto::from)?;
        let dek_version = session.dek_version();

        // 恢复态没有 `security.db` 连接：只写 JSONL，解锁后由补录回填。
        let state = super::vault_state(&app)?;
        let _ = state.audit_jsonl(
            assistant_vault::audit::AuditEvent::RecoveryExportUsed,
            Some(&export.entries.len().to_string()),
        );

        let entries = export
            .entries
            .into_iter()
            .map(|entry| {
                let blob = serde_json::to_value(&entry.blob).map_err(|e| {
                    super::internal_error(format!("Failed to serialize entry blob: {}", e))
                })?;
                Ok(RecoveryEntryDto {
                    id: entry.id,
                    category: entry.category,
                    blob,
                })
            })
            .collect::<Result<Vec<_>, VaultErrorDto>>()?;

        let skipped = export
            .skipped
            .into_iter()
            .map(|skip| RecoverySkipDto {
                id: skip.id,
                reason: skip.reason,
            })
            .collect();

        Ok(RecoveryExportDto {
            entries,
            skipped,
            exported_at: assistant_vault::types::now_millis(),
            dek_version,
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("vault_recover_from_paper task failed: {}", e)))?
}

// precondition: None
#[command]
pub fn vault_locate_paper_error(
    paper_key: String,
) -> Result<Option<PaperErrorPosDto>, VaultErrorDto> {
    Ok(
        assistant_vault::recovery_key::locate_checksum_error(&paper_key)
            .map(|(group, index)| PaperErrorPosDto { group, index }),
    )
}

// precondition: AnyState
#[command]
pub fn vault_has_recovery_backup(
    app: AppHandle,
) -> Result<Option<RecoveryBackupDto>, VaultErrorDto> {
    let repo = super::repo_state(&app)?;
    assistant_vault::lifecycle::recovery_backup(repo.manager()).map_err(VaultErrorDto::from)
}
