use tauri::{command, AppHandle, Manager};

use assistant_vault::biometric::BiometricKeyStore;
use assistant_vault::types::VaultError;

use super::SecurityDbState;

pub(super) fn get_key_store() -> Box<dyn BiometricKeyStore> {
    assistant_vault::biometric::create_key_store()
}

#[command]
pub fn vault_is_biometric_available() -> Result<bool, String> {
    let key_store = get_key_store();
    match key_store.is_available() {
        Ok(available) => {
            log::info!("Biometric availability check: {}", available);
            Ok(available)
        }
        Err(e) => {
            log::error!("Biometric availability check failed: {}", e);
            Err(e.to_string())
        }
    }
}

#[command]
pub fn vault_is_biometric_enrolled(app: AppHandle) -> Result<bool, String> {
    let state = super::vault_state(&app)?;
    let key_store = get_key_store();
    match state.is_biometric_enrolled(key_store.as_ref()) {
        Ok(enrolled) => Ok(enrolled),
        Err(e) => {
            log::warn!("Biometric enrolled check failed: {}", e);
            Err(e.to_string())
        }
    }
}

#[command]
pub async fn vault_unlock_biometric(app: AppHandle) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<SecurityDbState>();
        let key_store = get_key_store();
        match state.unlock_biometric(key_store.as_ref()) {
            Ok(()) => {
                super::ssh_auto_add_on_unlock(&app);
                crate::protocol::events::emit_broadcast(
                    &app,
                    crate::protocol::events::event_name::VAULT_UNLOCKED,
                    (),
                );
                Ok(true)
            }
            Err(VaultError::BiometricCancelled) => {
                log::warn!("Vault biometric unlock cancelled by user");
                Ok(false)
            }
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub async fn vault_enroll_biometric(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<SecurityDbState>();
        if state.is_locked() {
            return Err(VaultError::Locked.to_string());
        }

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
            .map_err(|e| e.to_string())?;
        log::info!("Vault biometric enrolled");
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub async fn vault_disenroll_biometric(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<SecurityDbState>();
        let key_store = get_key_store();
        state
            .disenroll_biometric(key_store.as_ref())
            .map_err(|e| e.to_string())?;
        log::info!("Vault biometric disenrolled");
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
