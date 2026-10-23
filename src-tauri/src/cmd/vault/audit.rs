use tauri::{command, AppHandle};

use assistant_security::audit::{log as audit_log, AuditEvent, AuditLogEntry};
use assistant_security::password_strength::{self, StrengthResult};
use assistant_security::vault_crypto::hibp::{HibpClient, SafePassword};
use assistant_security::vault_crypto::totp::{self, TotpParams, TotpResult};
use assistant_security::vault_crypto::types::SecureString;

#[command]
pub fn vault_totp_generate(
    app: AppHandle,
    id: String,
    period: Option<u32>,
    digits: Option<u32>,
    algorithm: Option<String>,
) -> Result<TotpResult, String> {
    let state = super::vault_state(&app)?;
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
    let state = super::vault_state(&app)?;
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
        let guard = super::clipboard_guard(&app)?;
        let timeout = super::get_clipboard_clear_secs(&app);
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
    let state = super::vault_state(&app)?;
    let guard = state.get_conn().map_err(|e| e.to_string())?;
    let conn = guard
        .as_ref()
        .ok_or_else(|| "Vault not initialized".to_string())?;
    assistant_security::audit::query_log(conn, limit.unwrap_or(100)).map_err(|e| e.to_string())
}

#[command]
pub async fn vault_hibp_check(app: AppHandle, id: String) -> Result<u32, String> {
    let state = super::vault_state(&app)?;
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

    let state = super::vault_state(&app)?;
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
