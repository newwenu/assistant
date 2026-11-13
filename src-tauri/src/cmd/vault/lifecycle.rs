use tauri::{command, AppHandle};

use assistant_vault::dto::{RecoveryKeyDto, VaultErrorCode, VaultErrorDto};
use assistant_vault::security_db::LockReason;
use assistant_vault::types::VaultError;

const RESET_CONFIRM_TOKEN: &str = "RESET-VAULT";

// precondition: AnyState
#[command]
pub async fn vault_init(app: AppHandle, master_password: String) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = super::vault_state(&app)?;
        let repo = super::repo_state(&app)?;
        assistant_vault::lifecycle::initialize(state.inner(), repo.manager(), &master_password)
            .map_err(|e| {
                log::error!("Vault init failed: {}", e);
                VaultErrorDto::from(e)
            })?;
        log::info!("Vault initialized");
        Ok(())
    })
    .await
    .map_err(|e| super::internal_error(format!("vault_init task failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub async fn vault_unlock(app: AppHandle, master_password: String) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = super::vault_state(&app)?;
        let repo = super::repo_state(&app)?;
        match assistant_vault::lifecycle::unlock(state.inner(), repo.manager(), &master_password) {
            Ok(()) => {
                super::ssh_auto_add_on_unlock(&app);
                crate::protocol::events::emit_broadcast(
                    &app,
                    crate::protocol::events::event_name::VAULT_UNLOCKED,
                    (),
                );
                Ok(())
            }
            Err(VaultError::InvalidPassword) => {
                log::warn!("Vault unlock failed: invalid password");
                Err(VaultErrorDto::from(VaultError::InvalidPassword))
            }
            Err(e) => {
                let version_detail = match e {
                    VaultError::UnsupportedVaultVersion => Some("unsupported_version"),
                    VaultError::VaultVersionTooNew => Some("version_too_new"),
                    _ => None,
                };
                if let Some(detail) = version_detail {
                    // 版本拒绝时库尚未打开：只写 JSONL。
                    let _ = state.audit_jsonl(
                        assistant_vault::audit::AuditEvent::VaultUnsupportedVersion,
                        Some(detail),
                    );
                }
                Err(VaultErrorDto::from(e))
            }
        }
    })
    .await
    .map_err(|e| super::internal_error(format!("vault_unlock task failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub fn vault_is_locked(app: AppHandle) -> Result<bool, VaultErrorDto> {
    let state = super::vault_state(&app)?;
    Ok(state.is_locked())
}

// precondition: AnyState
#[command]
pub fn vault_is_initialized(app: AppHandle) -> Result<bool, VaultErrorDto> {
    let state = super::vault_state(&app)?;
    state.is_initialized().map_err(VaultErrorDto::from)
}

// precondition: AnyState
#[command]
pub async fn vault_lock(app: AppHandle) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = super::vault_state(&app)?;
        state
            .lock_session(LockReason::User)
            .map_err(VaultErrorDto::from)?;
        super::on_locked(&app);
        Ok(())
    })
    .await
    .map_err(|e| super::internal_error(format!("vault_lock task failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub async fn vault_show_unlock_window(app: AppHandle) -> Result<(), VaultErrorDto> {
    use assistant_security::unlock_dialog::{create_unlock_dialog, UnlockResult};
    use tokio::sync::oneshot;

    let state = super::vault_state(&app)?;
    if !state.is_locked() {
        return Ok(());
    }

    let dialog = create_unlock_dialog()
        .map_err(|e| super::internal_error(format!("Failed to create unlock dialog: {}", e)))?;
    let (tx, rx) = oneshot::channel();
    dialog
        .show(Box::new(move |result| {
            let _ = tx.send(result);
        }))
        .map_err(|e| super::internal_error(format!("Failed to show unlock dialog: {}", e)))?;

    let result = rx
        .await
        .map_err(|_| super::internal_error("Unlock dialog channel closed"))?;
    match result {
        UnlockResult::Confirmed(password) => {
            let state = super::vault_state(&app)?;
            let repo = super::repo_state(&app)?;
            match assistant_vault::lifecycle::unlock(
                state.inner(),
                repo.manager(),
                password.expose_str(),
            ) {
                Ok(()) => {
                    super::ssh_auto_add_on_unlock(&app);
                    crate::protocol::events::emit_broadcast(
                        &app,
                        crate::protocol::events::event_name::VAULT_UNLOCKED,
                        (),
                    );
                    Ok(())
                }
                Err(VaultError::InvalidPassword) => {
                    log::warn!("Native unlock dialog: invalid password");
                    Err(VaultErrorDto::from(VaultError::InvalidPassword))
                }
                Err(e) => Err(VaultErrorDto::from(e)),
            }
        }
        UnlockResult::Cancelled => {
            log::info!("Native unlock dialog: user cancelled");
            Err(VaultErrorDto::new(
                VaultErrorCode::Cancelled,
                "Unlock dialog cancelled by user",
            ))
        }
    }
}

// precondition: Unlocked
#[command]
pub async fn vault_rotate_dek(app: AppHandle) -> Result<RecoveryKeyDto, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = super::repo_state(&app)?;
        let key_store = super::biometric::get_key_store();
        super::with_unlocked(&app, |state| {
            assistant_vault::lifecycle::rotate_dek(state, repo.manager(), key_store.as_ref())
                .map_err(VaultErrorDto::from)
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("vault_rotate_dek task failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub async fn vault_rebuild_local_db(
    app: AppHandle,
    master_password: String,
) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = super::vault_state(&app)?;
        let repo = super::repo_state(&app)?;
        match assistant_vault::lifecycle::rebuild_local_db(
            state.inner(),
            repo.manager(),
            &master_password,
        ) {
            Ok(()) => {
                super::ssh_auto_add_on_unlock(&app);
                crate::protocol::events::emit_broadcast(
                    &app,
                    crate::protocol::events::event_name::VAULT_UNLOCKED,
                    (),
                );
                log::info!("Local security db rebuilt and vault unlocked");
                Ok(())
            }
            Err(VaultError::InvalidPassword) => {
                log::warn!("Local db rebuild failed: invalid password");
                Err(VaultErrorDto::from(VaultError::InvalidPassword))
            }
            Err(e) => {
                log::error!("Local db rebuild failed: {}", e);
                Err(VaultErrorDto::from(e))
            }
        }
    })
    .await
    .map_err(|e| super::internal_error(format!("vault_rebuild_local_db task failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub async fn vault_reset_repository(
    app: AppHandle,
    confirm_token: String,
) -> Result<(), VaultErrorDto> {
    if confirm_token != RESET_CONFIRM_TOKEN {
        return Err(super::validation_error(
            "Repository reset requires explicit confirmation token",
        ));
    }

    tauri::async_runtime::spawn_blocking(move || {
        let dirs = crate::app_dirs::AppDirs::resolve()
            .map_err(|e| super::internal_error(format!("Failed to resolve app dirs: {}", e)))?;
        let state = super::vault_state(&app)?;

        state
            .lock_session(LockReason::Reset)
            .map_err(VaultErrorDto::from)?;
        super::on_locked(&app);

        let key_store = super::biometric::get_key_store();

        let vault_dir = super::repo_dir(&dirs);
        let meta_path = vault_dir.join(".vault-meta");
        let backup_path = super::meta_backup_path(&dirs);
        if meta_path.exists() {
            if let Some(parent) = backup_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    super::internal_error(format!("Failed to prepare meta backup dir: {}", e))
                })?;
            }
            std::fs::copy(&meta_path, &backup_path).map_err(|e| {
                super::internal_error(format!("Failed to back up vault meta: {}", e))
            })?;
        }

        let targets = vec![
            meta_path,
            super::security_db_path(&dirs),
            vault_dir.join("entries"),
            super::security_dir(&dirs).join("audit"),
            super::recovery_dir(&dirs),
        ];

        // 先清空 keystore（失败即中止），再删文件；库已删时重置事件只能落 JSONL。
        assistant_vault::lifecycle::reset_repository(&state, key_store.as_ref(), &targets)
            .map_err(VaultErrorDto::from)?;

        Ok(())
    })
    .await
    .map_err(|e| super::internal_error(format!("vault_reset_repository task failed: {}", e)))?
}
