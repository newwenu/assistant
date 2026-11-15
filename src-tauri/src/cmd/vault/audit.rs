use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{command, AppHandle, Emitter};

use assistant_crypto::vault_crypto::totp;
use assistant_crypto::vault_crypto::types::SecureString;
use assistant_security::hibp::{HibpClient, SafePassword};
use assistant_vault::audit::{log as audit_log, AuditEvent};
use assistant_vault::dto::{
    AuditLogEntry, HibpAuditEntryDto, ParsedOtpAuthDto, StrengthResult, TotpCodeDto, VaultErrorDto,
};
use assistant_vault::password_strength;
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::shared::TotpConfig;
use assistant_vault::types::VaultError;

static HIBP_AUDIT_CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
struct HibpProgress {
    done: u32,
    total: u32,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_totp_config(state: &SecurityDbState, id: &str) -> Result<TotpConfig, VaultErrorDto> {
    let snapshot = state.snapshot(id).map_err(VaultErrorDto::from)?;
    snapshot
        .totp_secret
        .as_deref()
        .and_then(TotpConfig::from_stored)
        .ok_or_else(|| {
            VaultErrorDto::from(VaultError::Validation(
                "TOTP is not configured for this entry".to_string(),
            ))
        })
}

// precondition: Unlocked
#[command]
pub fn vault_totp_generate(app: AppHandle, id: String) -> Result<TotpCodeDto, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let config = load_totp_config(state, &id)?;
        let code = config.code_at(unix_now()).map_err(VaultErrorDto::from)?;

        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        if let Some(conn) = guard.as_ref() {
            audit_log(
                conn,
                AuditEvent::VaultCopyField,
                Some(&format!("totp:{}", id)),
            );
        }

        Ok(TotpCodeDto {
            code: code.code,
            time_remaining: code.time_remaining,
            period: config.period(),
        })
    })
}

// precondition: Unlocked
#[command]
pub fn vault_copy_totp(app: AppHandle, id: String) -> Result<(), VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let config = load_totp_config(state, &id)?;
        let code = config.code_at(unix_now()).map_err(VaultErrorDto::from)?;

        {
            let guard = super::clipboard_guard(&app)?;
            let timeout = super::get_clipboard_clear_secs(&app);
            guard
                .copy_and_guard(&code.code, timeout)
                .map_err(VaultErrorDto::from)?;
        }

        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        if let Some(conn) = guard.as_ref() {
            // `vault_copy_field` 语义为「字段明文被读出」，复制 TOTP 亦属读出（规格 §2.3 R16）：
            // 前端文案不得一律写「复制」。
            audit_log(
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
pub fn vault_totp_parse_uri(uri: String) -> Result<ParsedOtpAuthDto, VaultErrorDto> {
    totp::parse_otpauth(&uri)
        .map(ParsedOtpAuthDto::from)
        .map_err(VaultErrorDto::from)
}

// precondition: Unlocked
#[command]
pub fn vault_query_audit_log(
    app: AppHandle,
    limit: Option<i64>,
    since: Option<i64>,
    event_types: Option<Vec<String>>,
) -> Result<Vec<AuditLogEntry>, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        let conn = guard
            .as_ref()
            .ok_or_else(|| VaultErrorDto::from(VaultError::NotInitialized))?;
        let limit = assistant_vault::audit::clamp_query_limit(limit);
        assistant_vault::audit::query_log(conn, limit, since, event_types.as_deref())
            .map_err(VaultErrorDto::from)
    })
}

// precondition: Unlocked
#[command]
#[allow(deprecated)] // 明文立即移交 SafePassword，由它负责 Drop 清零
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
                audit_log(
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

// precondition: None
#[command]
pub fn vault_hibp_audit_cancel() {
    HIBP_AUDIT_CANCEL.store(true, Ordering::SeqCst);
}

// precondition: Unlocked
#[command]
#[allow(deprecated)] // 明文立即移交 SafePassword，由它负责 Drop 清零
pub async fn vault_hibp_audit(app: AppHandle) -> Result<Vec<HibpAuditEntryDto>, VaultErrorDto> {
    use futures::stream::{self, StreamExt};
    use tokio::time::{sleep, Duration};

    HIBP_AUDIT_CANCEL.store(false, Ordering::SeqCst);

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

    let total = checks.len() as u32;
    let client = Arc::new(HibpClient::new());
    let interval = Arc::new(tokio::sync::Semaphore::new(1));
    let done = Arc::new(AtomicU32::new(0));
    let progress_app = app.clone();

    let results: Vec<(String, u32)> = stream::iter(checks)
        .map(move |(id, password)| {
            let client = client.clone();
            let interval = interval.clone();
            let done = done.clone();
            let progress_app = progress_app.clone();
            async move {
                if HIBP_AUDIT_CANCEL.load(Ordering::SeqCst) {
                    return (id, 0);
                }
                let _permit = match interval.acquire().await {
                    Ok(p) => p,
                    Err(_) => return (id, 0),
                };
                let safe = SafePassword::new(password);
                let count = client.check_password(&safe).await.unwrap_or(0);
                sleep(Duration::from_millis(HIBP_MIN_INTERVAL_MS)).await;
                drop(_permit);
                let finished = done.fetch_add(1, Ordering::SeqCst) + 1;
                let _ = progress_app.emit(
                    "vault:hibp-progress",
                    HibpProgress {
                        done: finished,
                        total,
                    },
                );
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
            audit_log(conn, AuditEvent::HibpAudit, None);
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
