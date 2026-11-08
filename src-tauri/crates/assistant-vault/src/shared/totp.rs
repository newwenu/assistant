use serde::{Deserialize, Serialize};

use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::totp::{self, TotpAlgorithm, TotpCode, TotpSpec};
use assistant_crypto::vault_crypto::types::VaultPlaintext;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTotpPayload {
    secret: String,
    digits: u32,
    period: u32,
    algorithm: String,
}

#[derive(Debug)]
pub struct TotpConfig {
    secret: VaultPlaintext,
    digits: u32,
    period: u32,
    algorithm: TotpAlgorithm,
}

impl TotpConfig {
    pub fn new(secret: VaultPlaintext, digits: u32, period: u32, algorithm: TotpAlgorithm) -> Self {
        let spec = TotpSpec::new(digits, period, algorithm);
        Self {
            secret,
            digits: spec.digits,
            period: spec.period,
            algorithm: spec.algorithm,
        }
    }

    pub fn from_parts(secret: &str, digits: u32, period: u32, algorithm: TotpAlgorithm) -> Self {
        Self::new(VaultPlaintext::new_str(secret), digits, period, algorithm)
    }

    /// 解析存储 payload；旧格式（纯 Base32）一律判为无效，返回 `None`。
    pub fn from_stored(stored: &str) -> Option<Self> {
        let trimmed = stored.trim();
        if !trimmed.starts_with('{') {
            return None;
        }

        let payload: StoredTotpPayload = serde_json::from_str(trimmed).ok()?;
        if payload.secret.trim().is_empty() {
            return None;
        }
        let algorithm = payload.algorithm.parse().unwrap_or_default();
        Some(Self::from_parts(
            payload.secret.trim(),
            payload.digits,
            payload.period,
            algorithm,
        ))
    }

    pub fn to_stored(&self) -> String {
        let payload = StoredTotpPayload {
            secret: self.secret.to_str().unwrap_or_default().to_string(),
            digits: self.digits,
            period: self.period,
            algorithm: self.algorithm.as_str().to_string(),
        };
        serde_json::to_string(&payload).unwrap_or_default()
    }

    pub fn secret(&self) -> &VaultPlaintext {
        &self.secret
    }

    pub fn digits(&self) -> u32 {
        self.digits
    }

    pub fn period(&self) -> u32 {
        self.period
    }

    pub fn algorithm(&self) -> TotpAlgorithm {
        self.algorithm
    }

    pub fn code_at(&self, unix_time: u64) -> Result<TotpCode, VaultError> {
        let secret = self
            .secret
            .to_str()
            .map_err(|_| VaultError::Validation("TOTP secret is not valid UTF-8".to_string()))?;
        let bytes = totp::decode_base32(secret)?;
        totp::generate_at(
            &bytes,
            &TotpSpec::new(self.digits, self.period, self.algorithm),
            unix_time,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_plain_secret_is_invalid() {
        assert!(TotpConfig::from_stored("JBSWY3DPEHPK3PXP").is_none());
    }

    #[test]
    fn valid_payload_is_parsed() {
        let payload =
            r#"{"secret":"JBSWY3DPEHPK3PXP","digits":8,"period":60,"algorithm":"sha256"}"#;
        let config = TotpConfig::from_stored(payload).expect("valid payload");
        assert_eq!(config.digits(), 8);
        assert_eq!(config.period(), 60);
        assert_eq!(config.algorithm(), TotpAlgorithm::Sha256);
    }

    #[test]
    fn broken_payload_is_invalid() {
        assert!(TotpConfig::from_stored("{not json").is_none());
        assert!(TotpConfig::from_stored(
            r#"{"secret":"","digits":6,"period":30,"algorithm":"sha1"}"#
        )
        .is_none());
    }

    #[test]
    fn stored_round_trip() {
        let config = TotpConfig::from_parts("JBSWY3DPEHPK3PXP", 7, 45, TotpAlgorithm::Sha512);
        let stored = config.to_stored();
        let parsed = TotpConfig::from_stored(&stored).expect("round trip");
        assert_eq!(parsed.digits(), 7);
        assert_eq!(parsed.period(), 45);
        assert_eq!(parsed.algorithm(), TotpAlgorithm::Sha512);
    }
}
