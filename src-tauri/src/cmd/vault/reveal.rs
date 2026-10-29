use tauri::{command, AppHandle};

use assistant_crypto::vault_crypto::types::SecureString;
use assistant_vault::audit::{log as audit_log, AuditEvent};
use assistant_vault::types::RevealResult;

static REVEAL_TTL_SECS: u64 = 30;

#[command]
pub fn vault_copy_password(app: AppHandle, id: String) -> Result<(), String> {
    let state = super::vault_state(&app)?;
    let password = state.decrypt_password(&id).map_err(|e| e.to_string())?;

    {
        let guard = super::clipboard_guard(&app)?;
        let timeout = super::get_clipboard_clear_secs(&app);
        guard
            .copy_and_guard(password.expose_str(), timeout)
            .map_err(|e| e.to_string())?;
    }

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::VaultCopyPassword, Some(&id));
        }
    }

    Ok(())
}

#[command]
pub fn vault_copy_field(app: AppHandle, id: String, field_name: String) -> Result<(), String> {
    let state = super::vault_state(&app)?;
    let value = state
        .decrypt_field(&id, &field_name)
        .map_err(|e| e.to_string())?;

    {
        let guard = super::clipboard_guard(&app)?;
        let timeout = super::get_clipboard_clear_secs(&app);
        guard
            .copy_and_guard(value.expose_str(), timeout)
            .map_err(|e| e.to_string())?;
    }

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::VaultCopyField, Some(&id));
        }
    }

    Ok(())
}

#[command]
pub fn vault_reveal_password(app: AppHandle, id: String) -> Result<RevealResult, String> {
    let guard = super::reveal_guard(&app)?;
    if let Err(e) = guard.check_and_record() {
        {
            let state = super::vault_state(&app)?;
            let conn_guard = state.get_conn().map_err(|e| e.to_string())?;
            if let Some(conn) = conn_guard.as_ref() {
                let _ = audit_log(conn, AuditEvent::VaultRevealRateLimited, Some(&id));
            }
        }
        crate::protocol::events::emit_broadcast(
            &app,
            crate::protocol::events::event_name::VAULT_REVEAL_RATE_LIMITED,
            &id,
        );
        return Err(e.to_string());
    }

    let state = super::vault_state(&app)?;
    let password = state.decrypt_password(&id).map_err(|e| e.to_string())?;

    let expires_at = chrono::Utc::now().timestamp_millis() + (REVEAL_TTL_SECS as i64) * 1000;

    {
        let conn_guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = conn_guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::VaultReveal, Some(&id));
        }
    }

    crate::protocol::events::emit_broadcast(
        &app,
        crate::protocol::events::event_name::VAULT_REVEAL,
        &id,
    );

    Ok(RevealResult {
        value: SecureString::into_inner(password),
        expires_at,
    })
}

#[command]
pub fn vault_reveal_notes(app: AppHandle, id: String) -> Result<RevealResult, String> {
    let guard = super::reveal_guard(&app)?;
    if let Err(e) = guard.check_and_record() {
        {
            let state = super::vault_state(&app)?;
            let conn_guard = state.get_conn().map_err(|e| e.to_string())?;
            if let Some(conn) = conn_guard.as_ref() {
                let _ = audit_log(conn, AuditEvent::VaultRevealRateLimited, Some(&id));
            }
        }
        crate::protocol::events::emit_broadcast(
            &app,
            crate::protocol::events::event_name::VAULT_REVEAL_RATE_LIMITED,
            &id,
        );
        return Err(e.to_string());
    }

    let state = super::vault_state(&app)?;
    let notes = state.decrypt_notes(&id).map_err(|e| e.to_string())?;

    let expires_at = chrono::Utc::now().timestamp_millis() + (REVEAL_TTL_SECS as i64) * 1000;

    {
        let conn_guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = conn_guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::VaultReveal, Some(&id));
        }
    }

    crate::protocol::events::emit_broadcast(
        &app,
        crate::protocol::events::event_name::VAULT_REVEAL,
        &id,
    );

    Ok(RevealResult {
        value: SecureString::into_inner(notes),
        expires_at,
    })
}
