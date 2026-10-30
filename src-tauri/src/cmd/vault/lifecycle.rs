use tauri::{command, AppHandle};

use assistant_vault::dto::{VaultErrorCode, VaultErrorDto};
use assistant_vault::types::VaultError;

// precondition: AnyState
#[command]
pub fn vault_init(app: AppHandle, master_password: String) -> Result<(), VaultErrorDto> {
    let state = super::vault_state(&app)?;
    state.init(&master_password).map_err(|e| {
        log::error!("Vault init failed: {}", e);
        VaultErrorDto::from(e)
    })?;
    log::info!("Vault initialized");
    Ok(())
}

// precondition: AnyState
#[command]
pub fn vault_unlock(app: AppHandle, master_password: String) -> Result<(), VaultErrorDto> {
    let state = super::vault_state(&app)?;
    match state.unlock(&master_password) {
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
        Err(e) => Err(VaultErrorDto::from(e)),
    }
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
pub fn vault_lock(app: AppHandle) -> Result<(), VaultErrorDto> {
    let state = super::vault_state(&app)?;
    state.lock().map_err(VaultErrorDto::from)?;
    super::on_locked(&app);
    Ok(())
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
            match state.unlock(password.expose_str()) {
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
