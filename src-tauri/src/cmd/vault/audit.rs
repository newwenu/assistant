use tauri::{command, AppHandle};

use assistant_crypto::vault_crypto::totp::{self, TotpParams};
use assistant_crypto::vault_crypto::types::SecureString;
use assistant_security::hibp::{HibpClient, SafePassword};
use assistant_vault::audit::{log as audit_log, AuditEvent};
use assistant_vault::dto::{
    AuditLogEntry, HibpAuditEntryDto, ParsedOtpAuthDto, StrengthResult, TotpResultDto,
    VaultErrorDto,
};
use assistant_vault::password_strength;
use assistant_vault::types::VaultError;

// precondition: Unlocked
#[command]
pub fn vault_totp_generate(
    app: AppHandle,
    id: String,
    period: Option<u32>,
    digits: Option<u32>,
    algorithm: Option<String>,
) -> Result<TotpResultDto, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let totp_secret = state
            .decrypt_totp_secret(&id)
            .map_err(VaultErrorDto::from)?;

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

        let result = totp::generate_totp(totp_secret.expose_str(), &params)
            .map_err(super::validation_error)?;

        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(
                conn,
                AuditEvent::VaultCopyField,
                Some(&format!("totp:{}", id)),
            );
        }

        Ok(TotpResultDto::from(result))
    })
}

// precondition: Unlocked
#[command]
pub fn vault_copy_totp(
    app: AppHandle,
    id: String,
    period: Option<u32>,
    digits: Option<u32>,
    algorithm: Option<String>,
) -> Result<(), VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let totp_secret = state
            .decrypt_totp_secret(&id)
            .map_err(VaultErrorDto::from)?;

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

        let result = totp::generate_totp(totp_secret.expose_str(), &params)
            .map_err(super::validation_error)?;

        {
            let guard = super::clipboard_guard(&app)?;
            let timeout = super::get_clipboard_clear_secs(&app);
            guard
                .copy_and_guard(&result.code, timeout)
                .map_err(VaultErrorDto::from)?;
        }

        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(
                conn,
                AuditEvent::VaultCopyField,
                Some(&format!("totp:{}", id)),
            );
        }

        Ok(())
    })
}

// precondition: None
#[command]
pub fn vault_totp_time_remaining(period: Option<u32>) -> Result<u32, VaultErrorDto> {
    Ok(totp::time_remaining(period.unwrap_or(30)))
}

// precondition: None
#[command]
pub fn vault_totp_parse_uri(uri: String) -> Result<ParsedOtpAuthDto, VaultErrorDto> {
    totp::from_otpauth_uri(&uri)
        .map(ParsedOtpAuthDto::from)
        .map_err(super::validation_error)
}

// precondition: Unlocked
#[command]
pub fn vault_query_audit_log(
    app: AppHandle,
    limit: Option<i64>,
) -> Result<Vec<AuditLogEntry>, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        let conn = guard
            .as_ref()
            .ok_or_else(|| VaultErrorDto::from(VaultError::NotInitialized))?;
        assistant_vault::audit::query_log(conn, limit.unwrap_or(100)).map_err(VaultErrorDto::from)
    })
}

// precondition: Unlocked
#[command]
pub async fn vault_hibp_check(app: AppHandle, id: String) -> Result<u32, VaultErrorDto> {
    let password = super::with_unlocked(&app, |state| {
        state.decrypt_password(&id).map_err(VaultErrorDto::from)
    })?;

    let safe = SafePassword::new(SecureString::into_inner(password));
    let client = HibpClient::new();
    let count = client.check_password(&safe).await.map_err(|e| {
        log::warn!("HIBP check failed for entry {}: {}", id, e);
        super::internal_error(format!("HIBP check failed: {}", e))
    })?;

    if count > 0 {
        log::warn!(
            "HIBP: password for entry {} found in {} breaches",
            id,
            count
        );
        super::with_unlocked(&app, |state| {
            let guard = state.get_conn().map_err(VaultErrorDto::from)?;
            if let Some(conn) = guard.as_ref() {
                let _ = audit_log(
                    conn,
                    AuditEvent::HibpLeakFound,
                    Some(&format!("{}:{}", id, count)),
                );
            }
            Ok(())
        })?;
    } else {
        log::debug!("HIBP: password for entry {} not found in breaches", id);
    }

    Ok(count)
}

// precondition: Unlocked
#[command]
pub async fn vault_hibp_audit(app: AppHandle) -> Result<Vec<HibpAuditEntryDto>, VaultErrorDto> {
    use futures::stream::{self, StreamExt};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    const HIBP_CONCURRENCY: usize = 2;
    const HIBP_MIN_INTERVAL_MS: u64 = 1500;

    let checks: Vec<(String, String)> = super::with_unlocked(&app, |state| {
        let entries = state
            .list_entries(None, None, None)
            .map_err(VaultErrorDto::from)?;
        Ok(entries
            .iter()
            .filter_map(|entry| {
                state
                    .decrypt_password(&entry.id)
                    .ok()
                    .map(|p| (entry.id.clone(), SecureString::into_inner(p)))
            })
            .collect())
    })?;

    let client = Arc::new(HibpClient::new());
    let interval = Arc::new(tokio::sync::Semaphore::new(1));

    let results: Vec<(String, u32)> = stream::iter(checks)
        .map(|(id, password)| {
            let client = client.clone();
            let interval = interval.clone();
            async move {
                let _permit = match interval.acquire().await {
                    Ok(p) => p,
                    Err(_) => return (id, 0),
                };
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

    super::with_unlocked(&app, |state| {
        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::HibpAudit, None);
        }
        Ok(())
    })?;

    if !results.is_empty() {
        log::warn!(
            "HIBP audit: {} entries with leaked passwords",
            results.len()
        );
    } else {
        log::info!("HIBP audit: no leaked passwords found");
    }

    Ok(results
        .into_iter()
        .map(|(entry_id, match_count)| HibpAuditEntryDto {
            entry_id,
            match_count,
        })
        .collect())
}

// precondition: None
#[command]
pub fn vault_check_master_password_strength(
    password: String,
) -> Result<StrengthResult, VaultErrorDto> {
    Ok(password_strength::estimate_strength(&password))
}
