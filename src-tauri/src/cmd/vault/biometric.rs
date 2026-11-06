use tauri::{command, AppHandle};

use assistant_vault::biometric::BiometricKeyStore;
use assistant_vault::dto::VaultErrorDto;
use assistant_vault::types::VaultError;

pub(super) fn get_key_store() -> Box<dyn BiometricKeyStore> {
    assistant_vault::biometric::create_key_store()
}

// precondition: None
#[command]
pub async fn vault_is_biometric_available() -> Result<bool, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let key_store = get_key_store();
        match key_store.is_available() {
            Ok(available) => {
                log::info!("Biometric availability check: {}", available);
                Ok(available)
            }
            Err(e) => {
                log::error!("Biometric availability check failed: {}", e);
                Err(VaultErrorDto::from(e))
            }
        }
    })
    .await
    .map_err(|e| super::internal_error(format!("Biometric availability task join failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub async fn vault_is_biometric_enrolled(app: AppHandle) -> Result<bool, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = super::vault_state(&app)?;
        let key_store = get_key_store();
        state
            .is_biometric_enrolled(key_store.as_ref())
            .map_err(|e| {
                log::warn!("Biometric enrolled check failed: {}", e);
                VaultErrorDto::from(e)
            })
    })
    .await
    .map_err(|e| super::internal_error(format!("Biometric enrolled task join failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub async fn vault_unlock_biometric(app: AppHandle) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = super::vault_state(&app)?;
        let key_store = get_key_store();
        match state.unlock_biometric(key_store.as_ref()) {
            Ok(()) => {
                super::ssh_auto_add_on_unlock(&app);
                crate::protocol::events::emit_broadcast(
                    &app,
                    crate::protocol::events::event_name::VAULT_UNLOCKED,
                    (),
                );
                Ok(())
            }
            Err(VaultError::BiometricCancelled) => {
                log::warn!("Vault biometric unlock cancelled by user");
                Err(VaultErrorDto::from(VaultError::BiometricCancelled))
            }
            Err(e) => Err(VaultErrorDto::from(e)),
        }
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault biometric unlock task join failed: {}", e)))?
}

// precondition: Unlocked
#[command]
pub async fn vault_enroll_biometric(app: AppHandle) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        super::with_unlocked(&app, |state| {
            let key_store = get_key_store();
            let app_clone = app.clone();
            state
                .enroll_biometric_with_hint(key_store.as_ref(), || {
                    crate::protocol::events::emit_broadcast(
                        &app_clone,
                        crate::protocol::events::event_name::BIOMETRIC_ENROLL_STEP,
                        "credential-created",
                    );
                })
                .map_err(VaultErrorDto::from)?;
            log::info!("Vault biometric enrolled");
            Ok(())
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault biometric enroll task join failed: {}", e)))?
}

// precondition: Unlocked
#[command]
pub async fn vault_disenroll_biometric(app: AppHandle) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        super::with_unlocked(&app, |state| {
            let key_store = get_key_store();
            state
                .disenroll_biometric(key_store.as_ref())
                .map_err(VaultErrorDto::from)?;
            log::info!("Vault biometric disenrolled");
            Ok(())
        })
    })
    .await
    .map_err(|e| {
        super::internal_error(format!("Vault biometric disenroll task join failed: {}", e))
    })?
}
