use tauri::{command, AppHandle};

use assistant_security::biometric::BiometricKeyStore;
use assistant_security::types::VaultError;

fn get_key_store() -> Box<dyn BiometricKeyStore> {
    assistant_security::biometric::create_key_store()
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
        Ok(enrolled) => {
            log::debug!("Biometric enrolled check: {}", enrolled);
            Ok(enrolled)
        }
        Err(e) => {
            log::warn!("Biometric enrolled check failed: {}", e);
            Err(e.to_string())
        }
    }
}

#[command]
pub fn vault_unlock_biometric(app: AppHandle) -> Result<bool, String> {
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
            Ok(true)
        }
        Err(VaultError::BiometricCancelled) => {
            log::warn!("Vault biometric unlock cancelled by user");
            Ok(false)
        }
        Err(e) => Err(e.to_string()),
    }
}

#[command]
pub fn vault_enroll_biometric(app: AppHandle, master_password: String) -> Result<(), String> {
    let state = super::vault_state(&app)?;

    match state.unlock(&master_password) {
        Ok(()) => {}
        Err(VaultError::AlreadyUnlocked) => {}
        Err(VaultError::InvalidPassword) => return Err("Invalid master password".to_string()),
        Err(e) => return Err(e.to_string()),
    }

    let key_store = get_key_store();
    state
        .enroll_biometric(key_store.as_ref())
        .map_err(|e| e.to_string())?;
    log::info!("Vault biometric enrolled");
    Ok(())
}

#[command]
pub fn vault_disenroll_biometric(app: AppHandle) -> Result<(), String> {
    let state = super::vault_state(&app)?;
    let key_store = get_key_store();
    state
        .disenroll_biometric(key_store.as_ref())
        .map_err(|e| e.to_string())?;
    log::info!("Vault biometric disenrolled");
    Ok(())
}