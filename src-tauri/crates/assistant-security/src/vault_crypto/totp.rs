use data_encoding::BASE32;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

type HmacSha1 = Hmac<Sha1>;
type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

const DEFAULT_STEP: u32 = 30;
const DEFAULT_DIGITS: u32 = 6;
const MAX_DIGITS: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TotpAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

impl Default for TotpAlgorithm {
    fn default() -> Self {
        Self::Sha1
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpParams {
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithm,
}

impl Default for TotpParams {
    fn default() -> Self {
        Self {
            digits: DEFAULT_DIGITS,
            period: DEFAULT_STEP,
            algorithm: TotpAlgorithm::default(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpResult {
    pub code: String,
    pub time_remaining: u32,
}

pub fn generate_totp(secret_b32: &str, params: &TotpParams) -> Result<TotpResult, String> {
    let secret = decode_base32(secret_b32)?;
    let code = compute_totp_code(&secret, params)?;
    let time_remaining = time_remaining(params.period);
    Ok(TotpResult {
        code,
        time_remaining,
    })
}

pub fn time_remaining(period: u32) -> u32 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let period = period as u64;
    let remaining = period - (now % period);
    remaining as u32
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedOtpAuth {
    pub secret: String,
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithm,
    pub issuer: Option<String>,
    pub account: Option<String>,
}

pub fn from_otpauth_uri(uri: &str) -> Result<ParsedOtpAuth, String> {
    if !uri.starts_with("otpauth://totp/") {
        return Err("Invalid otpauth URI: must start with otpauth://totp/".to_string());
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
                algorithm = match value.to_uppercase().as_str() {
                    "SHA256" | "SHA-256" => TotpAlgorithm::Sha256,
                    "SHA512" | "SHA-512" => TotpAlgorithm::Sha512,
                    _ => TotpAlgorithm::Sha1,
                };
            }
            "issuer" => issuer = Some(urldecode(value)),
            _ => {}
        }
    }

    if secret.is_empty() {
        return Err("otpauth URI missing secret parameter".to_string());
    }

    Ok(ParsedOtpAuth {
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

fn decode_base32(input: &str) -> Result<Vec<u8>, String> {
    let sanitized: String = input.chars().filter(|c| !c.is_whitespace()).collect();

    if sanitized.is_empty() {
        return Err("Empty Base32 secret".to_string());
    }

    let upper = sanitized.to_uppercase();

    if upper.contains('=') {
        BASE32
            .decode(upper.as_bytes())
            .map_err(|e| format!("Invalid Base32: {}", e))
    } else {
        let padded_len = ((upper.len() + 7) / 8) * 8;
        let padding_count = padded_len - upper.len();
        let padded = upper + &"=".repeat(padding_count);
        BASE32
            .decode(padded.as_bytes())
            .map_err(|e| format!("Invalid Base32: {}", e))
    }
}

fn compute_totp_code(secret: &[u8], params: &TotpParams) -> Result<String, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let counter = now / (params.period as u64);
    let counter_bytes = counter.to_be_bytes();

    let hmac_result = match params.algorithm {
        TotpAlgorithm::Sha1 => {
            let mut mac = HmacSha1::new_from_slice(secret)
                .map_err(|e| format!("HMAC-SHA1 init failed: {}", e))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::Sha256 => {
            let mut mac = HmacSha256::new_from_slice(secret)
                .map_err(|e| format!("HMAC-SHA256 init failed: {}", e))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::Sha512 => {
            let mut mac = HmacSha512::new_from_slice(secret)
                .map_err(|e| format!("HMAC-SHA512 init failed: {}", e))?;
            mac.update(&counter_bytes);
            mac.finalize().into_bytes().to_vec()
        }
    };

    let offset = (hmac_result.last().ok_or("HMAC result empty")? & 0x0f) as usize;
    if offset + 3 >= hmac_result.len() {
        return Err("HMAC result too short for offset".to_string());
    }

    let binary: u32 = ((hmac_result[offset] as u32 & 0x7f) << 24)
        | ((hmac_result[offset + 1] as u32 & 0xff) << 16)
        | ((hmac_result[offset + 2] as u32 & 0xff) << 8)
        | (hmac_result[offset + 3] as u32 & 0xff);

    let digits = params_digits_clamped(params.digits);
    let power = 10u64.pow(digits);
    let otp = (binary as u64) % power;

    Ok(format!("{:0width$}", otp, width = digits as usize))
}

fn params_digits_clamped(digits: u32) -> u32 {
    digits.clamp(1, MAX_DIGITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_base32_roundtrip() {
        let original = b"Hello!";
        let encoded = BASE32.encode(original);
        let decoded = decode_base32(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_decode_base32_20bytes() {
        let secret: [u8; 20] = [
            0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x21, 0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe,
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
        ];
        let encoded = BASE32.encode(&secret);
        let decoded = decode_base32(&encoded).unwrap();
        assert_eq!(decoded, secret);
    }

    #[test]
    fn test_decode_base32_no_padding() {
        let result = decode_base32("JBSWY3DPEHPK3PXP").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_decode_base32_padding() {
        let result = decode_base32("JBSWY3DP").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_from_otpauth_uri_basic() {
        let uri = "otpauth://totp/TestIssuer:test@example.com?secret=JBSWY3DPEHPK3PXP&issuer=TestIssuer&algorithm=SHA1&digits=6&period=30";
        let parsed = from_otpauth_uri(uri).unwrap();
        assert_eq!(parsed.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(parsed.digits, 6);
        assert_eq!(parsed.period, 30);
        assert_eq!(parsed.algorithm, TotpAlgorithm::Sha1);
        assert_eq!(parsed.issuer.as_deref(), Some("TestIssuer"));
    }

    #[test]
    fn test_time_remaining_within_period() {
        let remaining = time_remaining(30);
        assert!(remaining > 0 && remaining <= 30);
    }

    #[test]
    fn test_generate_totp_format() {
        let params = TotpParams::default();
        let result = generate_totp("JBSWY3DPEHPK3PXP", &params).unwrap();
        assert_eq!(result.code.len(), 6);
        assert!(result.code.chars().all(|c| c.is_ascii_digit()));
        assert!(result.time_remaining > 0 && result.time_remaining <= 30);
    }
}
