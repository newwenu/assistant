use tauri::{command, AppHandle};

use assistant_vault::types::VaultError;

#[command]
pub fn vault_init(app: AppHandle, master_password: String) -> Result<(), String> {
    let state = super::vault_state(&app)?;
    state.init(&master_password).map_err(|e| {
        log::error!("Vault init failed: {}", e);
        e.to_string()
    })?;
    log::info!("Vault initialized");
    Ok(())
}

#[command]
pub fn vault_unlock(app: AppHandle, master_password: String) -> Result<bool, String> {
    let state = super::vault_state(&app)?;
    match state.unlock(&master_password) {
        Ok(()) => {
            super::ssh_auto_add_on_unlock(&app);
            crate::protocol::events::emit_broadcast(
                &app,
                crate::protocol::events::event_name::VAULT_UNLOCKED,
                (),
            );
            Ok(true)
        }
        Err(VaultError::InvalidPassword) => {
            log::warn!("Vault unlock failed: invalid password");
            Ok(false)
        }
        Err(e) => Err(e.to_string()),
    }
}

#[command]
pub fn vault_is_locked(app: AppHandle) -> Result<bool, String> {
    let state = super::vault_state(&app)?;
    Ok(state.is_locked())
}

#[command]
pub fn vault_is_initialized(app: AppHandle) -> Result<bool, String> {
    let state = super::vault_state(&app)?;
    state.is_initialized().map_err(|e| e.to_string())
}

#[command]
pub fn vault_lock(app: AppHandle) -> Result<(), String> {
    super::ssh_remove_all_on_lock(&app);
    {
        let guard = super::clipboard_guard(&app)?;
        if let Err(e) = guard.force_clear() {
            log::warn!("Clipboard force_clear on vault_lock failed: {}", e);
        }
    }
    let state = super::vault_state(&app)?;
    state.lock().map_err(|e| e.to_string())?;
    crate::protocol::events::emit_broadcast(
        &app,
        crate::protocol::events::event_name::VAULT_LOCKED,
        (),
    );
    Ok(())
}

#[command]
pub async fn vault_show_unlock_window(app: AppHandle) -> Result<bool, String> {
    use assistant_security::unlock_dialog::{create_unlock_dialog, UnlockResult};
    use tokio::sync::oneshot;

    let state = super::vault_state(&app)?;
    if !state.is_locked() {
        return Ok(true);
    }

    let dialog = create_unlock_dialog().map_err(|e| e.to_string())?;
    let (tx, rx) = oneshot::channel();
    dialog
        .show(Box::new(move |result| {
            let _ = tx.send(result);
        }))
        .map_err(|e| e.to_string())?;

    let result = rx
        .await
        .map_err(|_| "Unlock dialog channel closed".to_string())?;
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
                    Ok(true)
                }
                Err(VaultError::InvalidPassword) => {
                    log::warn!("Native unlock dialog: invalid password");
                    Ok(false)
                }
                Err(e) => Err(e.to_string()),
            }
        }
        UnlockResult::Cancelled => {
            log::info!("Native unlock dialog: user cancelled");
            Ok(false)
        }
    }
}
