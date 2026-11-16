use assistant_crypto::vault_crypto::totp::{ParsedOtpAuthSpec, TotpAlgorithm};

use crate::dto::hibp::HibpAuditEntryDto;
use crate::dto::lifecycle::LockReasonDto;
use crate::dto::totp::{ParsedOtpAuthDto, TotpAlgorithmDto};
use crate::security_db::LockReason;

impl From<TotpAlgorithm> for TotpAlgorithmDto {
    fn from(v: TotpAlgorithm) -> Self {
        match v {
            TotpAlgorithm::Sha1 => Self::Sha1,
            TotpAlgorithm::Sha256 => Self::Sha256,
            TotpAlgorithm::Sha512 => Self::Sha512,
        }
    }
}

impl From<LockReasonDto> for LockReason {
    fn from(v: LockReasonDto) -> Self {
        match v {
            LockReasonDto::User => Self::User,
            LockReasonDto::AutoLock => Self::AutoLock,
            LockReasonDto::ScreenLock => Self::ScreenLock,
            LockReasonDto::AppExit => Self::AppExit,
            LockReasonDto::Reset => Self::Reset,
            LockReasonDto::Panic => Self::Panic,
        }
    }
}

impl From<ParsedOtpAuthSpec> for ParsedOtpAuthDto {
    fn from(v: ParsedOtpAuthSpec) -> Self {
        let ParsedOtpAuthSpec {
            secret,
            digits,
            period,
            algorithm,
            issuer,
            account,
        } = v;
        Self {
            secret,
            digits,
            period,
            algorithm: TotpAlgorithmDto::from(algorithm),
            issuer,
            account,
        }
    }
}

impl From<(String, u32)> for HibpAuditEntryDto {
    fn from(v: (String, u32)) -> Self {
        let (entry_id, match_count) = v;
        Self {
            entry_id,
            match_count,
        }
    }
}
