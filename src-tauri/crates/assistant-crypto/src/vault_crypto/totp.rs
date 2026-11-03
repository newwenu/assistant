use std::str::FromStr;

use data_encoding::BASE32;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use crate::error::VaultError;

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

const DEFAULT_STEP: u32 = 30;
const DEFAULT_DIGITS: u32 = 6;
const MAX_DIGITS: u32 = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TotpAlgorithm {
    #[default]
    Sha1,
    Sha256,
    Sha512,
}

impl TotpAlgorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
        }
    }
}

impl FromStr for TotpAlgorithm {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "SHA1" | "SHA-1" => Ok(Self::Sha1),
            "SHA256" | "SHA-256" => Ok(Self::Sha256),
            "SHA512" | "SHA-512" => Ok(Self::Sha512),
            other => Err(format!("unknown TOTP algorithm: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TotpSpec {
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithm,
}

impl TotpSpec {
    pub fn new(digits: u32, period: u32, algorithm: TotpAlgorithm) -> Self {
        Self {
            digits: digits.clamp(1, MAX_DIGITS),
            period: period.max(1),
            algorithm,
        }
    }
}

impl Default for TotpSpec {
    fn default() -> Self {
        Self {
            digits: DEFAULT_DIGITS,
            period: DEFAULT_STEP,
            algorithm: TotpAlgorithm::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpCode {
    pub code: String,
    pub time_remaining: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedOtpAuthSpec {
    pub secret: String,
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithm,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

pub fn generate_at(secret: &[u8], spec: &TotpSpec, unix_time: u64) -> Result<TotpCode, VaultError> {
    let code = compute_totp_code(secret, spec, unix_time)?;
    Ok(TotpCode {
        code,
        time_remaining: time_remaining_at(unix_time, spec.period),
    })
}

pub fn time_remaining_at(unix_time: u64, period: u32) -> u32 {
    let period = period.max(1) as u64;
    (period - (unix_time % period)) as u32
}

pub fn parse_otpauth(uri: &str) -> Result<ParsedOtpAuthSpec, VaultError> {
    if !uri.starts_with("otpauth://totp/") {
        return Err(VaultError::Validation(
            "Invalid otpauth URI: must start with otpauth://totp/".to_string(),
        ));
    }

    let without_scheme = &uri["otpauth://totp/".len()..];

    let (path_part, query_part) = match without_scheme.find('?') {
        Some(pos) => (&without_scheme[..pos], &without_scheme[pos + 1..]),
        None => (without_scheme, ""),
    };

    let mut issuer: Option<String> = None;
    let mut account: Option<String> = None;

    if let Some(colon_pos) = path_part.find(':') {
        issuer = Some(urldecode(&path_part[..colon_pos]));
        account = Some(urldecode(&path_part[colon_pos + 1..]));
    } else if !path_part.is_empty() {
        account = Some(urldecode(path_part));
    }

    let mut secret = String::new();
    let mut digits = DEFAULT_DIGITS;
    let mut period = DEFAULT_STEP;
    let mut algorithm = TotpAlgorithm::default();

    for pair in query_part.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.find('=') {
            Some(pos) => (&pair[..pos], &pair[pos + 1..]),
            None => continue,
        };
        match key {
            "secret" => secret = value.to_string(),
            "digits" => {
                if let Ok(d) = value.parse::<u32>() {
                    digits = d.clamp(1, MAX_DIGITS);
                }
            }
            "period" => {
                if let Ok(p) = value.parse::<u32>() {
                    period = p.max(1);
                }
            }
            "algorithm" => {
                algorithm = TotpAlgorithm::from_str(value).unwrap_or_default();
            }
            "issuer" => issuer = Some(urldecode(value)),
            _ => {}
        }
    }

    if secret.is_empty() {
        return Err(VaultError::Validation(
            "otpauth URI missing secret parameter".to_string(),
        ));
    }

    Ok(ParsedOtpAuthSpec {
        secret,
        digits,
        period,
        algorithm,
        issuer,
        account,
    })
}

fn urldecode(s: &str) -> String {
    let mut buf = Vec::with_capacity(s.len());
    let src = s.as_bytes();
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'%' && i + 2 < src.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                buf.push(b);
                i += 3;
                continue;
            }
        }
        buf.push(src[i]);
        i += 1;
    }
    String::from_utf8(buf).unwrap_or_else(|_| s.to_string())
}

pub fn decode_base32(input: &str) -> Result<Vec<u8>, VaultError> {
    let sanitized: String = input.chars().filter(|c| !c.is_whitespace()).collect();

    if sanitized.is_empty() {
        return Err(VaultError::Validation("Empty Base32 secret".to_string()));
    }

    let upper = sanitized.to_uppercase();

    let decoded = if upper.contains('=') {
        BASE32.decode(upper.as_bytes())
    } else {
        let padded_len = upper.len().div_ceil(8) * 8;
        let padding_count = padded_len - upper.len();
        let padded = format!("{}{}", upper, "=".repeat(padding_count));
        BASE32.decode(padded.as_bytes())
    };

    decoded.map_err(|e| VaultError::Validation(format!("Invalid Base32: {}", e)))
}

fn compute_totp_code(secret: &[u8], spec: &TotpSpec, unix_time: u64) -> Result<String, VaultError> {
    let period = spec.period.max(1) as u64;
    let counter = unix_time / period;
    let counter_bytes = counter.to_be_bytes();

    let hmac_result = match spec.algorithm {
        TotpAlgorithm::Sha1 => {
            let mut mac = HmacSha1::new_from_slice(secret)
                .map_err(|e| VaultError::Crypto(format!("HMAC-SHA1 init failed: {}", e)))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::Sha256 => {
            let mut mac = HmacSha256::new_from_slice(secret)
                .map_err(|e| VaultError::Crypto(format!("HMAC-SHA256 init failed: {}", e)))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::Sha512 => {
            let mut mac = HmacSha512::new_from_slice(secret)
                .map_err(|e| VaultError::Crypto(format!("HMAC-SHA512 init failed: {}", e)))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
    };

    let offset = (hmac_result
        .last()
        .ok_or_else(|| VaultError::Crypto("HMAC result empty".to_string()))?
        & 0x0f) as usize;
    if offset + 3 >= hmac_result.len() {
        return Err(VaultError::Crypto(
            "HMAC result too short for offset".to_string(),
        ));
    }

    let binary: u32 = ((hmac_result[offset] as u32 & 0x7f) << 24)
        | ((hmac_result[offset + 1] as u32 & 0xff) << 16)
        | ((hmac_result[offset + 2] as u32 & 0xff) << 8)
        | (hmac_result[offset + 3] as u32 & 0xff);

    let digits = spec.digits.clamp(1, MAX_DIGITS);
    let power = 10u64.pow(digits);
    let otp = (binary as u64) % power;

    Ok(format!("{:0width$}", otp, width = digits as usize))
}
