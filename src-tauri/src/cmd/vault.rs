use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{command, AppHandle, Manager};

use assistant_security::audit::{log as audit_log, AuditEvent};
use assistant_security::biometric::BiometricKeyStore;
use assistant_security::clipboard_guard::ClipboardGuard;
use assistant_security::password_strength::{self, StrengthResult};
use assistant_security::reveal_guard::RevealGuard;
use assistant_security::security_db::SecurityDbState;
use assistant_security::types::{
    RevealResult, VaultEntryCreate, VaultEntryDetail, VaultEntryExport, VaultEntrySummary,
    VaultEntryUpdate, VaultError,
};
use assistant_security::vault_crypto::hibp::HibpClient;
use assistant_security::vault_crypto::totp::{self, TotpParams, TotpResult};

static REVEAL_TTL_SECS: u64 = 30;
static LAST_REVEAL_EXPIRY: AtomicU64 = AtomicU64::new(0);

fn vault_state(app: &AppHandle) -> Result<tauri::State<'_, SecurityDbState>, String> {
    app.state::<SecurityDbState>()
        .try_into()
        .map_err(|_| "SecurityDbState not managed".to_string())
}

fn reveal_guard(app: &AppHandle) -> Result<tauri::State<'_, RevealGuard>, String> {
    app.state::<RevealGuard>()
        .try_into()
        .map_err(|_| "RevealGuard not managed".to_string())
}

fn clipboard_guard(app: &AppHandle) -> Result<tauri::State<'_, ClipboardGuard>, String> {
    app.state::<ClipboardGuard>()
        .try_into()
        .map_err(|_| "ClipboardGuard not managed".to_string())
}

fn get_clipboard_clear_secs(_app: &AppHandle) -> u64 {
    use assistant_config::app_conf::AppConf;
    AppConf::load()
        .map(|c| c.vault_clipboard_clear_seconds as u64)
        .unwrap_or(30)
}

#[command]
pub fn vault_init(app: AppHandle, master_password: String) -> Result<(), String> {
    let state = vault_state(&app)?;
    state.init(&master_password).map_err(|e| e.to_string())
}

#[command]
pub fn vault_unlock(app: AppHandle, master_password: String) -> Result<bool, String> {
    let state = vault_state(&app)?;
    match state.unlock(&master_password) {
        Ok(()) => {
            crate::protocol::events::emit_broadcast(
                &app,
                crate::protocol::events::event_name::VAULT_UNLOCKED,
                (),
            );
            Ok(true)
        }
        Err(VaultError::InvalidPassword) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

#[command]
pub fn vault_is_locked(app: AppHandle) -> Result<bool, String> {
    let state = vault_state(&app)?;
    Ok(state.is_locked())
}

#[command]
pub fn vault_is_initialized(app: AppHandle) -> Result<bool, String> {
    let state = vault_state(&app)?;
    state.is_initialized().map_err(|e| e.to_string())
}

#[command]
pub fn vault_lock(app: AppHandle) -> Result<(), String> {
    let state = vault_state(&app)?;
    state.lock().map_err(|e| e.to_string())?;
    crate::protocol::events::emit_broadcast(
        &app,
        crate::protocol::events::event_name::VAULT_LOCKED,
        (),
    );
    Ok(())
}

#[command]
pub fn vault_list_entries(
    app: AppHandle,
    category: Option<String>,
    search: Option<String>,
    tag: Option<String>,
) -> Result<Vec<VaultEntrySummary>, String> {
    let state = vault_state(&app)?;
    let cat = category.and_then(|c| assistant_security::types::VaultCategory::from_str(&c));
    state
        .list_entries(cat, search.as_deref(), tag.as_deref())
        .map_err(|e| e.to_string())
}

#[command]
pub fn vault_get_entry(app: AppHandle, id: String) -> Result<VaultEntryDetail, String> {
    let state = vault_state(&app)?;
    state.get_entry(&id).map_err(|e| e.to_string())
}

#[command]
pub fn vault_copy_password(app: AppHandle, id: String) -> Result<(), String> {
    let state = vault_state(&app)?;
    let password = state.decrypt_password(&id).map_err(|e| e.to_string())?;

    {
        let guard = clipboard_guard(&app)?;
        let timeout = get_clipboard_clear_secs(&app);
        guard
            .copy_and_guard(&password, timeout)
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
    let state = vault_state(&app)?;
    let value = state
        .decrypt_field(&id, &field_name)
        .map_err(|e| e.to_string())?;

    {
        let guard = clipboard_guard(&app)?;
        let timeout = get_clipboard_clear_secs(&app);
        guard
            .copy_and_guard(&value, timeout)
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
    let guard = reveal_guard(&app)?;
    if let Err(e) = guard.check_and_record() {
        {
            let state = vault_state(&app)?;
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

    let state = vault_state(&app)?;
    let password = state.decrypt_password(&id).map_err(|e| e.to_string())?;

    let expires_at = chrono::Utc::now().timestamp() + REVEAL_TTL_SECS as i64;
    LAST_REVEAL_EXPIRY.store(expires_at as u64, Ordering::SeqCst);

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
        value: password,
        expires_at,
    })
}

#[command]
pub fn vault_reveal_notes(app: AppHandle, id: String) -> Result<RevealResult, String> {
    let guard = reveal_guard(&app)?;
    if let Err(e) = guard.check_and_record() {
        {
            let state = vault_state(&app)?;
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

    let state = vault_state(&app)?;
    let notes = state.decrypt_notes(&id).map_err(|e| e.to_string())?;

    let expires_at = chrono::Utc::now().timestamp() + REVEAL_TTL_SECS as i64;
    LAST_REVEAL_EXPIRY.store(expires_at as u64, Ordering::SeqCst);

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
        value: notes,
        expires_at,
    })
}

#[command]
pub fn vault_create_entry(app: AppHandle, entry: VaultEntryCreate) -> Result<String, String> {
    let state = vault_state(&app)?;
    state.create_entry(entry).map_err(|e| e.to_string())
}

#[command]
pub fn vault_update_entry(
    app: AppHandle,
    id: String,
    entry: VaultEntryUpdate,
) -> Result<(), String> {
    let state = vault_state(&app)?;
    state.update_entry(&id, entry).map_err(|e| e.to_string())
}

#[command]
pub fn vault_delete_entry(app: AppHandle, id: String) -> Result<(), String> {
    let state = vault_state(&app)?;
    state.delete_entry(&id).map_err(|e| e.to_string())
}

#[command]
pub fn vault_change_master_password(
    app: AppHandle,
    current: String,
    new: String,
) -> Result<(), String> {
    let state = vault_state(&app)?;
    state
        .change_master_password(&current, &new)
        .map_err(|e| e.to_string())
}

#[command]
pub fn vault_export(app: AppHandle, format: String) -> Result<String, String> {
    let state = vault_state(&app)?;
    let entries: Vec<VaultEntryExport> = state.export_entries().map_err(|e| e.to_string())?;

    let export_dir = match app.path().app_data_dir() {
        Ok(dir) => dir.join("security").join("exports"),
        Err(e) => return Err(e.to_string()),
    };
    std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let (file_name, content) = match format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
            (
                format!("vault_export_{}.json", timestamp),
                json.into_bytes(),
            )
        }
        "csv" => {
            let mut w = csv::Writer::from_writer(Vec::new());
            for e in &entries {
                w.serialize(e).map_err(|e| e.to_string())?;
            }
            let bytes = w.into_inner().map_err(|e| e.to_string())?;
            (format!("vault_export_{}.csv", timestamp), bytes)
        }
        _ => return Err(format!("Unsupported export format: {}", format)),
    };

    let file_path = export_dir.join(&file_name);
    std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;

    Ok(file_path.to_str().unwrap_or(&file_name).to_string())
}

#[command]
pub fn vault_import(app: AppHandle, path: String, format: String) -> Result<u32, String> {
    let state = vault_state(&app)?;
    let file_path = PathBuf::from(&path);
    let data = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;

    let entries: Vec<VaultEntryCreate> = match format.as_str() {
        "json" => serde_json::from_str(&data).map_err(|e| e.to_string())?,
        "csv" => {
            let mut reader = csv::Reader::from_reader(data.as_bytes());
            let mut result = Vec::new();
            for record in reader.deserialize() {
                let entry: VaultEntryCreate = record.map_err(|e| e.to_string())?;
                result.push(entry);
            }
            result
        }
        _ => return Err(format!("Unsupported import format: {}", format)),
    };

    state.import_entries(entries).map_err(|e| e.to_string())
}

#[command]
pub fn vault_totp_generate(
    app: AppHandle,
    id: String,
    period: Option<u32>,
    digits: Option<u32>,
    algorithm: Option<String>,
) -> Result<TotpResult, String> {
    let state = vault_state(&app)?;
    let totp_secret = state.decrypt_totp_secret(&id).map_err(|e| e.to_string())?;

    let algo = match algorithm.as_deref() {
        Some("SHA256") | Some("sha256") => totp::TotpAlgorithm::Sha256,
        Some("SHA512") | Some("sha512") => totp::TotpAlgorithm::Sha512,
        _ => totp::TotpAlgorithm::Sha1,
    };

    let params = TotpParams {
        digits: digits.unwrap_or(6),
        period: period.unwrap_or(30),
        algorithm: algo,
    };

    let result = totp::generate_totp(&totp_secret, &params).map_err(|e| e.to_string())?;

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(
                conn,
                AuditEvent::VaultCopyField,
                Some(&format!("totp:{}", id)),
            );
        }
    }

    Ok(result)
}

#[command]
pub fn vault_totp_time_remaining(period: Option<u32>) -> Result<u32, String> {
    Ok(totp::time_remaining(period.unwrap_or(30)))
}

#[command]
pub async fn vault_hibp_check(app: AppHandle, id: String) -> Result<u32, String> {
    let state = vault_state(&app)?;
    let password = state.decrypt_password(&id).map_err(|e| e.to_string())?;

    let client = HibpClient::new();
    let count = client
        .check_password(&password)
        .await
        .map_err(|e| e.to_string())?;

    if count > 0 {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(
                conn,
                AuditEvent::HibpLeakFound,
                Some(&format!("{}:{}", id, count)),
            );
        }
    }

    Ok(count)
}

#[command]
pub async fn vault_hibp_audit(app: AppHandle) -> Result<Vec<(String, u32)>, String> {
    let state = vault_state(&app)?;
    let entries = state
        .list_entries(None, None, None)
        .map_err(|e| e.to_string())?;

    let client = HibpClient::new();
    let mut results = Vec::new();

    for entry in &entries {
        if let Ok(password) = state.decrypt_password(&entry.id) {
            if let Ok(count) = client.check_password(&password).await {
                if count > 0 {
                    results.push((entry.id.clone(), count));
                }
            }
        }
    }

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::HibpAudit, None);
        }
    }

    Ok(results)
}

#[command]
pub fn vault_check_master_password_strength(password: String) -> Result<StrengthResult, String> {
    Ok(password_strength::estimate_strength(&password))
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

fn get_key_store() -> Box<dyn BiometricKeyStore> {
    assistant_security::biometric::create_key_store()
}

#[command]
pub fn vault_is_biometric_available() -> Result<bool, String> {
    let key_store = get_key_store();
    key_store.is_available().map_err(|e| e.to_string())
}

#[command]
pub fn vault_is_biometric_enrolled(app: AppHandle) -> Result<bool, String> {
    let state = vault_state(&app)?;
    let key_store = get_key_store();
    state
        .is_biometric_enrolled(key_store.as_ref())
        .map_err(|e| e.to_string())
}

#[command]
pub fn vault_unlock_biometric(app: AppHandle) -> Result<bool, String> {
    let state = vault_state(&app)?;
    let key_store = get_key_store();
    match state.unlock_biometric(key_store.as_ref()) {
        Ok(()) => {
            crate::protocol::events::emit_broadcast(
                &app,
                crate::protocol::events::event_name::VAULT_UNLOCKED,
                (),
            );
            Ok(true)
        }
        Err(VaultError::BiometricCancelled) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

#[command]
pub fn vault_enroll_biometric(app: AppHandle, master_password: String) -> Result<(), String> {
    let state = vault_state(&app)?;

    match state.unlock(&master_password) {
        Ok(()) => {}
        Err(VaultError::AlreadyUnlocked) => {}
        Err(VaultError::InvalidPassword) => return Err("Invalid master password".to_string()),
        Err(e) => return Err(e.to_string()),
    }

    let key_store = get_key_store();
    state
        .enroll_biometric(key_store.as_ref())
        .map_err(|e| e.to_string())
}

#[command]
pub fn vault_disenroll_biometric(app: AppHandle) -> Result<(), String> {
    let state = vault_state(&app)?;
    let key_store = get_key_store();
    state
        .disenroll_biometric(key_store.as_ref())
        .map_err(|e| e.to_string())
}
