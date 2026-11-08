use serde::{Deserialize, Serialize};
use ts_rs::TS;

use assistant_crypto::vault_crypto::totp::TotpAlgorithm;

use crate::shared::totp::TotpConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum TotpAlgorithmDto {
    #[default]
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "vault.contract.ts")]
pub struct TotpInputDto {
    #[ts(optional = nullable)]
    pub secret: Option<String>,
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithmDto,
}

impl Default for TotpInputDto {
    fn default() -> Self {
        Self {
            secret: None,
            digits: 6,
            period: 30,
            algorithm: TotpAlgorithmDto::Sha1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct TotpCodeDto {
    pub code: String,
    pub time_remaining: u32,
    pub period: u32,
}

pub(crate) fn algorithm_from_dto(value: TotpAlgorithmDto) -> TotpAlgorithm {
    match value {
        TotpAlgorithmDto::Sha1 => TotpAlgorithm::Sha1,
        TotpAlgorithmDto::Sha256 => TotpAlgorithm::Sha256,
        TotpAlgorithmDto::Sha512 => TotpAlgorithm::Sha512,
    }
}

pub(crate) fn algorithm_dto_from_str(value: &str) -> TotpAlgorithmDto {
    match value.to_ascii_uppercase().as_str() {
        "SHA256" | "SHA-256" => TotpAlgorithmDto::Sha256,
        "SHA512" | "SHA-512" => TotpAlgorithmDto::Sha512,
        _ => TotpAlgorithmDto::Sha1,
    }
}

/// 把录入参数序列化为存储 payload；secret 为空或缺失时返回 `None`（表示无 TOTP / 清除）。
pub(crate) fn payload_from_input(input: Option<&TotpInputDto>) -> Option<String> {
    let input = input?;
    let secret = input
        .secret
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    Some(
        TotpConfig::from_parts(
            secret,
            input.digits,
            input.period,
            algorithm_from_dto(input.algorithm),
        )
        .to_stored(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct ParsedOtpAuthDto {
    pub secret: String,
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithmDto,
    pub issuer: Option<String>,
    pub account: Option<String>,
}
