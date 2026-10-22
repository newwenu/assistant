use std::path::PathBuf;

use tauri::{command, AppHandle, Manager};

use assistant_config::app_conf::AppConf;
use assistant_security::audit::{log as audit_log, AuditEvent, AuditLogEntry};
use assistant_security::biometric::BiometricKeyStore;
use assistant_security::clipboard_guard::ClipboardGuard;
use assistant_security::password_strength::{self, StrengthResult};
use assistant_security::reveal_guard::RevealGuard;
use assistant_security::security_db::SecurityDbState;
use assistant_security::types::{
    RevealResult, VaultEntryCreate, VaultEntryCsvRow, VaultEntryDetail, VaultEntryExport,
    VaultEntrySummary, VaultEntryUpdate, VaultError,
};
use assistant_security::vault_crypto::hibp::{HibpClient, SafePassword};
use assistant_security::vault_crypto::totp::{self, TotpParams, TotpResult};
use assistant_security::vault_crypto::types::SecureString;

static REVEAL_TTL_SECS: u64 = 30;

fn ssh_bridge_from_conf() -> Box<dyn assistant_security::ssh_agent::SshAgentBridge> {
    let conf = AppConf::load().ok();
    let use_pageant = conf
        .as_ref()
        .and_then(|c| Some(c.vault_ssh_use_pageant))
        .unwrap_or(true);
    let auth_sock_override = conf.as_ref().and_then(|c| {
        let s = c.vault_ssh_auth_sock_override.clone();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    });
    assistant_security::ssh_agent::create_agent_bridge(use_pageant, auth_sock_override)
}

pub(crate) fn ssh_remove_all_on_lock(app: &AppHandle) {
    let ssh_state = app.state::<crate::setup::SshAgentState>();
    let fingerprints: Vec<String> = {
        let mut fps = ssh_state.added_fingerprints.lock().unwrap();
        fps.drain().collect()
    };
    if fingerprints.is_empty() {
        return;
    }
    let bridge = ssh_bridge_from_conf();
    for fp in &fingerprints {
        if let Err(e) = bridge.remove_key(fp) {
            log::warn!("Failed to remove SSH key {} from agent: {}", fp, e);
        }
    }
    log::info!(
        "Removed {} SSH keys from agent on vault lock",
        fingerprints.len()
    );
}

fn ssh_auto_add_on_unlock(app: &AppHandle) {
    use assistant_config::app_conf::AppConf;
    let conf = match AppConf::load() {
        Ok(c) => c,
        Err(_) => return,
    };
    if !conf.vault_ssh_auto_add {
        return;
    }
    if let Err(e) = vault_ssh_auto_add_all(app.clone()) {
        log::warn!("SSH auto-add on unlock failed: {}", e);
    }
}

fn vault_state(app: &AppHandle) -> Result<tauri::State<'_, SecurityDbState>, String> {
    app.state::<SecurityDbState>()
        .try_into()
        .map_err(|_| "SecurityDbState not managed".to_string())
}

fn cipher_algo() -> assistant_security::vault_crypto::types::CipherAlgorithm {
    let conf = AppConf::load().ok();
    let val = conf
        .as_ref()
        .map(|c| c.vault_cipher_algorithm.as_str())
        .unwrap_or("aes256Gcm");
    SecurityDbState::cipher_algorithm_from_conf(val)
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
    state.init(&master_password).map_err(|e| {
        log::error!("Vault init failed: {}", e);
        e.to_string()
    })?;
    log::info!("Vault initialized");
    Ok(())
}

#[command]
pub fn vault_unlock(app: AppHandle, master_password: String) -> Result<bool, String> {
    let state = vault_state(&app)?;
    match state.unlock(&master_password) {
        Ok(()) => {
            ssh_auto_add_on_unlock(&app);
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
    ssh_remove_all_on_lock(&app);
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
    let cat = category.and_then(|c| assistant_security::types::VaultCategory::parse(&c));
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
    let state = vault_state(&app)?;
    let value = state
        .decrypt_field(&id, &field_name)
        .map_err(|e| e.to_string())?;

    {
        let guard = clipboard_guard(&app)?;
        let timeout = get_clipboard_clear_secs(&app);
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

#[command]
pub fn vault_create_entry(app: AppHandle, entry: VaultEntryCreate) -> Result<String, String> {
    let state = vault_state(&app)?;
    let id = state
        .create_entry(entry, cipher_algo())
        .map_err(|e| e.to_string())?;
    log::info!("Vault entry created: {}", id);
    Ok(id)
}

#[command]
pub fn vault_update_entry(
    app: AppHandle,
    id: String,
    entry: VaultEntryUpdate,
) -> Result<(), String> {
    let state = vault_state(&app)?;
    state
        .update_entry(&id, entry, cipher_algo())
        .map_err(|e| e.to_string())?;
    log::info!("Vault entry updated: {}", id);
    Ok(())
}

#[command]
pub fn vault_delete_entry(app: AppHandle, id: String) -> Result<(), String> {
    let state = vault_state(&app)?;
    state.delete_entry(&id).map_err(|e| e.to_string())?;
    log::info!("Vault entry deleted: {}", id);
    Ok(())
}

#[command]
pub fn vault_change_master_password(
    app: AppHandle,
    current: String,
    new: String,
) -> Result<(), String> {
    let state = vault_state(&app)?;
    state
        .change_master_password(&current, &new, cipher_algo())
        .map_err(|e| e.to_string())?;

    {
        let engine = sync_state(&app)?;
        let master_key = state.get_master_key().map_err(|e| e.to_string())?;
        let mk = master_key.as_ref().ok_or("Vault is locked")?;

        if let Err(e) = engine.clear_emergency_data() {
            log::warn!(
                "Failed to clear emergency data after master password change: {}",
                e
            );
        }

        if let Err(e) = engine.rewrite_vault_meta_with_new_key(mk) {
            log::warn!(
                "Failed to rewrite vault-meta HMAC after master password change: {}",
                e
            );
        }
    }

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::RecoveryKeyInvalidated, None);
        }
    }

    log::info!("Vault master password changed");
    Ok(())
}

#[command]
pub fn vault_export(app: AppHandle, format: String) -> Result<String, String> {
    let state = vault_state(&app)?;
    let entries: Vec<VaultEntryExport> = state.export_entries().map_err(|e| e.to_string())?;
    log::info!("Vault export: {} entries as {}", entries.len(), format);

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
                let row: VaultEntryCsvRow = e.clone().into();
                w.serialize(&row).map_err(|e| e.to_string())?;
            }
            let bytes = w.into_inner().map_err(|e| e.to_string())?;
            (format!("vault_export_{}.csv", timestamp), bytes)
        }
        _ => return Err(format!("Unsupported export format: {}", format)),
    };

    let file_path = export_dir.join(&file_name);
    std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;

    {
        let cleanup_path = file_path.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(300));
            if let Err(e) = std::fs::remove_file(&cleanup_path) {
                log::warn!("Failed to auto-clean export file: {}", e);
            } else {
                log::info!("Auto-cleaned export file: {:?}", cleanup_path);
            }
        });
    }

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
                let row: VaultEntryCsvRow = record.map_err(|e| e.to_string())?;
                result.push(row.into());
            }
            result
        }
        _ => return Err(format!("Unsupported import format: {}", format)),
    };

    let result = state
        .import_entries(entries, cipher_algo())
        .map_err(|e| e.to_string())?;
    log::info!(
        "Vault import: {} entries from {} ({})",
        result,
        path,
        format
    );
    Ok(result)
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

    let result =
        totp::generate_totp(totp_secret.expose_str(), &params).map_err(|e| e.to_string())?;

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
pub fn vault_copy_totp(
    app: AppHandle,
    id: String,
    period: Option<u32>,
    digits: Option<u32>,
    algorithm: Option<String>,
) -> Result<(), String> {
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

    let result =
        totp::generate_totp(totp_secret.expose_str(), &params).map_err(|e| e.to_string())?;

    {
        let guard = clipboard_guard(&app)?;
        let timeout = get_clipboard_clear_secs(&app);
        guard
            .copy_and_guard(&result.code, timeout)
            .map_err(|e| e.to_string())?;
    }

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

    Ok(())
}

#[command]
pub fn vault_totp_time_remaining(period: Option<u32>) -> Result<u32, String> {
    Ok(totp::time_remaining(period.unwrap_or(30)))
}

#[command]
pub fn vault_totp_parse_uri(uri: String) -> Result<totp::ParsedOtpAuth, String> {
    totp::from_otpauth_uri(&uri)
}

#[command]
pub fn vault_query_audit_log(
    app: AppHandle,
    limit: Option<i64>,
) -> Result<Vec<AuditLogEntry>, String> {
    let state = vault_state(&app)?;
    let guard = state.get_conn().map_err(|e| e.to_string())?;
    let conn = guard
        .as_ref()
        .ok_or_else(|| "Vault not initialized".to_string())?;
    assistant_security::audit::query_log(conn, limit.unwrap_or(100)).map_err(|e| e.to_string())
}

#[command]
pub async fn vault_hibp_check(app: AppHandle, id: String) -> Result<u32, String> {
    let state = vault_state(&app)?;
    let password = state.decrypt_password(&id).map_err(|e| e.to_string())?;

    let safe = SafePassword::new(SecureString::into_inner(password));
    let client = HibpClient::new();
    let count = client.check_password(&safe).await.map_err(|e| {
        log::warn!("HIBP check failed for entry {}: {}", id, e);
        e.to_string()
    })?;

    if count > 0 {
        log::warn!(
            "HIBP: password for entry {} found in {} breaches",
            id,
            count
        );
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(
                conn,
                AuditEvent::HibpLeakFound,
                Some(&format!("{}:{}", id, count)),
            );
        }
    } else {
        log::debug!("HIBP: password for entry {} not found in breaches", id);
    }

    Ok(count)
}

#[command]
pub async fn vault_hibp_audit(app: AppHandle) -> Result<Vec<(String, u32)>, String> {
    use futures::stream::{self, StreamExt};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    const HIBP_CONCURRENCY: usize = 2;
    const HIBP_MIN_INTERVAL_MS: u64 = 1500;

    let state = vault_state(&app)?;
    let entries = state
        .list_entries(None, None, None)
        .map_err(|e| e.to_string())?;

    let client = Arc::new(HibpClient::new());
    let interval = Arc::new(tokio::sync::Semaphore::new(1));

    let checks: Vec<(String, String)> = entries
        .iter()
        .filter_map(|entry| {
            state
                .decrypt_password(&entry.id)
                .ok()
                .map(|p| (entry.id.clone(), SecureString::into_inner(p)))
        })
        .collect();

    let results: Vec<(String, u32)> = stream::iter(checks)
        .map(|(id, password)| {
            let client = client.clone();
            let interval = interval.clone();
            async move {
                let _permit = interval.acquire().await.unwrap();
                let safe = SafePassword::new(password);
                let count = client.check_password(&safe).await.unwrap_or(0);
                sleep(Duration::from_millis(HIBP_MIN_INTERVAL_MS)).await;
                drop(_permit);
                (id, count)
            }
        })
        .buffer_unordered(HIBP_CONCURRENCY)
        .filter(|(_, count)| futures::future::ready(*count > 0))
        .collect()
        .await;

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::HibpAudit, None);
        }
    }

    if !results.is_empty() {
        log::warn!(
            "HIBP audit: {} entries with leaked passwords",
            results.len()
        );
    } else {
        log::info!("HIBP audit: no leaked passwords found");
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
    let state = vault_state(&app)?;
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
    let state = vault_state(&app)?;
    let key_store = get_key_store();
    match state.unlock_biometric(key_store.as_ref()) {
        Ok(()) => {
            ssh_auto_add_on_unlock(&app);
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
        .map_err(|e| e.to_string())?;
    log::info!("Vault biometric enrolled");
    Ok(())
}

#[command]
pub fn vault_disenroll_biometric(app: AppHandle) -> Result<(), String> {
    let state = vault_state(&app)?;
    let key_store = get_key_store();
    state
        .disenroll_biometric(key_store.as_ref())
        .map_err(|e| e.to_string())?;
    log::info!("Vault biometric disenrolled");
    Ok(())
}

fn sync_state(
    app: &AppHandle,
) -> Result<tauri::State<'_, assistant_sync::sync_engine::SyncEngine>, String> {
    app.state::<assistant_sync::sync_engine::SyncEngine>()
        .try_into()
        .map_err(|_| "SyncEngine not managed".to_string())
}

#[command]
pub fn vault_sync_pull(app: AppHandle) -> Result<assistant_sync::SyncResult, String> {
    let engine = sync_state(&app)?;
    let result = engine.pull().map_err(|e| {
        log::warn!("Vault sync pull failed: {}", e);
        e.to_string()
    })?;
    log::debug!("Vault sync pull completed");
    Ok(result)
}

#[command]
pub fn vault_sync_push(app: AppHandle) -> Result<assistant_sync::SyncResult, String> {
    let engine = sync_state(&app)?;
    let result = engine.push().map_err(|e| {
        log::warn!("Vault sync push failed: {}", e);
        e.to_string()
    })?;
    log::debug!("Vault sync push completed");
    Ok(result)
}

#[command]
pub fn vault_sync_now(app: AppHandle) -> Result<assistant_sync::SyncResult, String> {
    let engine = sync_state(&app)?;
    let result = engine.sync_now().map_err(|e| {
        log::warn!("Vault sync failed: {}", e);
        e.to_string()
    })?;
    log::debug!("Vault sync completed");
    Ok(result)
}

#[command]
pub fn vault_sync_force_push(app: AppHandle) -> Result<assistant_sync::SyncResult, String> {
    let engine = sync_state(&app)?;
    let result = engine.force_push().map_err(|e| {
        log::warn!("Vault sync force-push failed: {}", e);
        e.to_string()
    })?;
    log::debug!("Vault sync force-push completed");
    Ok(result)
}

#[command]
pub fn vault_get_sync_config(app: AppHandle) -> Result<assistant_sync::SyncConfig, String> {
    let engine = sync_state(&app)?;
    Ok(engine.get_config())
}

#[command]
pub fn vault_set_sync_config(
    app: AppHandle,
    config: assistant_sync::SyncConfig,
) -> Result<(), String> {
    let engine = sync_state(&app)?;
    engine.set_config(config);
    Ok(())
}

#[command]
pub fn vault_resolve_conflict(
    app: AppHandle,
    path: String,
    resolution: assistant_sync::ConflictResolution,
) -> Result<(), String> {
    let engine = sync_state(&app)?;
    engine
        .resolve_conflict(&path, resolution)
        .map_err(|e| e.to_string())
}

#[command]
pub fn vault_git_init(app: AppHandle) -> Result<(), String> {
    let engine = sync_state(&app)?;
    engine.init().map_err(|e| {
        log::warn!("Vault git init failed: {}", e);
        e.to_string()
    })?;
    log::info!("Vault git init completed");
    Ok(())
}

#[command]
pub fn vault_git_clone(app: AppHandle, url: String) -> Result<(), String> {
    let engine = sync_state(&app)?;
    engine.clone_repo(&url).map_err(|e| {
        log::warn!("Vault git clone failed: {}", e);
        e.to_string()
    })?;
    log::info!("Vault git clone completed");
    Ok(())
}

#[command]
pub fn vault_git_add_remote(app: AppHandle, remote: String, url: String) -> Result<(), String> {
    let engine = sync_state(&app)?;
    engine.add_remote(&remote, &url).map_err(|e| e.to_string())
}

#[command]
pub fn vault_git_set_remote(app: AppHandle, remote: String, url: String) -> Result<(), String> {
    let engine = sync_state(&app)?;
    engine.set_remote(&remote, &url).map_err(|e| e.to_string())
}

#[command]
pub fn vault_sync_readyness(app: AppHandle) -> Result<assistant_sync::Readyness, String> {
    let engine = sync_state(&app)?;
    engine.readyness().map_err(|e| e.to_string())
}

#[command]
pub fn vault_sync_prepare(app: AppHandle) -> Result<(), String> {
    let engine = sync_state(&app)?;
    engine.prepare().map_err(|e| e.to_string())
}

#[command]
pub fn vault_sync_finalize(app: AppHandle, message: String) -> Result<(), String> {
    let engine = sync_state(&app)?;
    engine.finalize(&message).map_err(|e| e.to_string())
}

#[command]
pub fn vault_ssh_add_to_agent(
    app: AppHandle,
    id: String,
    lifetime: Option<u32>,
) -> Result<String, String> {
    let state = vault_state(&app)?;
    let private_key = state
        .decrypt_ssh_private_key(&id)
        .map_err(|e| e.to_string())?;

    let meta = assistant_security::ssh_agent::validate::validate_private_key(private_key.expose())
        .map_err(|e| e.to_string())?;

    let bridge = ssh_bridge_from_conf();
    bridge
        .add_key(private_key.expose(), &meta.fingerprint, lifetime)
        .map_err(|e| e.to_string())?;

    {
        let ssh_state = app.state::<crate::setup::SshAgentState>();
        let mut fps = ssh_state.added_fingerprints.lock().unwrap();
        fps.insert(meta.fingerprint.clone());
    }

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::SshKeyLoaded, Some(&id));
        }
    }

    log::info!(
        "SSH key added to agent: {} (entry: {})",
        meta.fingerprint,
        id
    );
    Ok(meta.fingerprint)
}

#[command]
pub fn vault_ssh_remove_from_agent(app: AppHandle, fingerprint: String) -> Result<(), String> {
    let bridge = ssh_bridge_from_conf();
    bridge.remove_key(&fingerprint).map_err(|e| e.to_string())?;

    {
        let ssh_state = app.state::<crate::setup::SshAgentState>();
        let mut fps = ssh_state.added_fingerprints.lock().unwrap();
        fps.remove(&fingerprint);
    }

    {
        let state = vault_state(&app)?;
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::SshKeyRemoved, Some(&fingerprint));
        }
    }

    log::info!("SSH key removed from agent: {}", fingerprint);
    Ok(())
}

#[command]
pub fn vault_ssh_list_agent(
    app: AppHandle,
) -> Result<Vec<assistant_security::ssh_agent::SshKeyInfo>, String> {
    let bridge = ssh_bridge_from_conf();
    let mut keys = bridge.list_keys().map_err(|e| e.to_string())?;

    let ssh_state = app.state::<crate::setup::SshAgentState>();
    let vault_fps = ssh_state.added_fingerprints.lock().unwrap();
    for key in &mut keys {
        key.added_by_vault = vault_fps.contains(&key.fingerprint);
    }

    Ok(keys)
}

#[command]
pub fn vault_ssh_auto_add_all(app: AppHandle) -> Result<Vec<String>, String> {
    use assistant_config::app_conf::AppConf;
    let conf = AppConf::load().map_err(|e| e.to_string())?;
    if !conf.vault_ssh_auto_add {
        return Ok(Vec::new());
    }

    let state = vault_state(&app)?;
    let ssh_entries: Vec<VaultEntrySummary> = state
        .list_entries(
            Some(assistant_security::types::VaultCategory::SshKey),
            None,
            None,
        )
        .map_err(|e| e.to_string())?;

    let bridge = ssh_bridge_from_conf();
    let mut added = Vec::new();

    for entry in &ssh_entries {
        if let Ok(private_key) = state.decrypt_ssh_private_key(&entry.id) {
            if let Ok(meta) =
                assistant_security::ssh_agent::validate::validate_private_key(private_key.expose())
            {
                if bridge
                    .add_key(private_key.expose(), &meta.fingerprint, None)
                    .is_ok()
                {
                    {
                        let ssh_state = app.state::<crate::setup::SshAgentState>();
                        let mut fps = ssh_state.added_fingerprints.lock().unwrap();
                        fps.insert(meta.fingerprint.clone());
                    }
                    added.push(meta.fingerprint);
                }
            }
        }
    }

    if !added.is_empty() {
        log::info!("SSH auto-add: {} keys added to agent", added.len());
    }

    Ok(added)
}

#[command]
pub fn vault_ssh_is_agent_running() -> Result<bool, String> {
    let bridge = ssh_bridge_from_conf();
    bridge.is_agent_running().map_err(|e| e.to_string())
}

#[command]
pub fn vault_generate_recovery_key(app: AppHandle) -> Result<String, String> {
    let state = vault_state(&app)?;
    let master_key = state.get_master_key().map_err(|e| e.to_string())?;
    let mk = master_key.as_ref().ok_or("Vault is locked")?;
    let bootstrap_key = state.get_bootstrap_key().map_err(|e| e.to_string())?;
    let bk = bootstrap_key.as_ref().ok_or("Vault is locked")?;

    let (recovery_key, emergency) = assistant_security::recovery_key::generate_recovery_key(mk, bk)
        .map_err(|e| e.to_string())?;

    let engine = sync_state(&app)?;
    engine
        .write_emergency_data(mk, &emergency)
        .map_err(|e| e.to_string())?;

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::RecoveryKeyGenerated, None);
        }
    }

    log::warn!("Recovery key generated and emergency data written");
    Ok(recovery_key)
}

#[command]
pub fn vault_unlock_with_recovery_key(
    app: AppHandle,
    recovery_key: String,
) -> Result<bool, String> {
    let state = vault_state(&app)?;
    if !state.is_locked() {
        return Err("Vault is already unlocked".to_string());
    }

    let engine = sync_state(&app)?;
    let emergency = engine.read_emergency_data().map_err(|e| e.to_string())?;
    let emergency = emergency.ok_or("Recovery key not set")?;

    let emergency_data = assistant_security::recovery_key::EmergencyData {
        token: emergency.token,
        salt: emergency.salt,
    };

    let result = assistant_security::recovery_key::unlock_with_recovery_key(
        &recovery_key,
        &emergency_data,
        &state,
    )
    .map_err(|e| {
        log::warn!("Vault unlock with recovery key failed: {}", e);
        e.to_string()
    })?;

    if result {
        ssh_auto_add_on_unlock(&app);
        crate::protocol::events::emit_broadcast(
            &app,
            crate::protocol::events::event_name::VAULT_UNLOCKED,
            (),
        );
        log::warn!("Vault unlocked via recovery key");
    }

    Ok(result)
}

#[command]
pub fn vault_has_recovery_key(app: AppHandle) -> Result<bool, String> {
    let engine = sync_state(&app)?;
    let emergency = engine.read_emergency_data().map_err(|e| e.to_string())?;
    Ok(emergency.is_some())
}

#[command]
pub fn vault_verify_vault_meta_integrity(
    app: AppHandle,
) -> Result<assistant_sync::vault_file::IntegrityResult, String> {
    let state = vault_state(&app)?;
    let master_key = state.get_master_key().map_err(|e| e.to_string())?;
    let mk = master_key.as_ref().ok_or("Vault is locked")?;

    let engine = sync_state(&app)?;
    engine
        .verify_vault_meta_integrity(mk)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisasterRecoveryResult {
    pub file_path: String,
    pub entry_count: u32,
    pub integrity_warning: bool,
}

#[command]
pub fn vault_disaster_recovery_export(
    app: AppHandle,
    git_url: Option<String>,
    recovery_key: String,
    export_path: String,
) -> Result<DisasterRecoveryResult, String> {
    let engine = sync_state(&app)?;

    if let Some(url) = &git_url {
        engine.clone_repo(url).map_err(|e| {
            log::warn!("Disaster recovery: clone failed: {}", e);
            e.to_string()
        })?;
    } else {
        engine.prepare().map_err(|e| {
            log::warn!("Disaster recovery: prepare failed: {}", e);
            e.to_string()
        })?;
        let _ = engine.pull();
    }

    let emergency = engine.read_emergency_data().map_err(|e| e.to_string())?;
    let emergency = emergency.ok_or("Recovery key not set in .vault-meta")?;

    let emergency_data = assistant_security::recovery_key::EmergencyData {
        token: emergency.token,
        salt: emergency.salt,
    };

    let (_bootstrap_key, master_key) =
        assistant_security::recovery_key::decrypt_recovery_keys(&recovery_key, &emergency_data)
            .map_err(|e| {
                log::warn!("Disaster recovery: decrypt failed: {}", e);
                e.to_string()
            })?;

    let mut integrity_warning = false;
    match engine.verify_vault_meta_integrity(&master_key) {
        Ok(result) => {
            if !result.valid {
                integrity_warning = true;
                log::warn!("Disaster recovery: HMAC mismatch — {}", result.message);
            }
        }
        Err(e) => {
            integrity_warning = true;
            log::warn!("Disaster recovery: integrity check error: {}", e);
        }
    }

    let exports = engine.disaster_recovery_export(&master_key).map_err(|e| {
        log::warn!("Disaster recovery: export failed: {}", e);
        e.to_string()
    })?;

    let entry_count = exports.len() as u32;

    let json = serde_json::to_string_pretty(&exports).map_err(|e| e.to_string())?;
    std::fs::write(&export_path, &json).map_err(|e| e.to_string())?;

    log::info!(
        "Disaster recovery export: {} entries to {}",
        entry_count,
        export_path
    );

    Ok(DisasterRecoveryResult {
        file_path: export_path,
        entry_count,
        integrity_warning,
    })
}
