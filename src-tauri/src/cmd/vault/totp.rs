use tauri::{command, AppHandle};

use assistant_crypto::vault_crypto::totp;
use assistant_vault::audit::{log as audit_log, AuditEvent};
use assistant_vault::dto::{ParsedOtpAuthDto, TotpCodeDto, VaultErrorDto};
use assistant_vault::security_db::SecurityDbState;
use assistant_vault::shared::TotpConfig;
use assistant_vault::types::VaultError;

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
        let code = config
            .code_at(super::unix_now())
            .map_err(VaultErrorDto::from)?;

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
        let code = config
            .code_at(super::unix_now())
            .map_err(VaultErrorDto::from)?;

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
