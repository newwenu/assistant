use tauri::{command, AppHandle, Manager};

use assistant_security::types::VaultError;

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
pub fn vault_show_unlock_window(app: AppHandle) -> Result<(), String> {
    use tauri::WebviewUrl;

    if let Some(win) = app.get_webview_window("vault-unlock") {
        let _ = win.set_focus();
        return Ok(());
    }

    let builder = tauri::WebviewWindowBuilder::new(
        &app,
        "vault-unlock",
        WebviewUrl::App("/vault-unlock".into()),
    )
    .title("Vault Unlock")
    .inner_size(400.0, 200.0)
    .center()
    .resizable(false)
    .decorations(true)
    .always_on_top(true)
    .skip_taskbar(true);

    #[cfg(not(debug_assertions))]
    let builder = builder.devtools(false);

    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}