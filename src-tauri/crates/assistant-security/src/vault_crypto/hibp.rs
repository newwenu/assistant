use sha1::{Digest, Sha1};
use zeroize::Zeroize;

const HIBP_API_URL: &str = "https://api.pwnedpasswords.com/range/";
const HIBP_USER_AGENT: &str = "Assistant-Security";

pub struct HibpClient {
    http: reqwest::Client,
}

impl HibpClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(HIBP_USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { http }
    }

    pub async fn check_password(&self, password: &str) -> Result<u32, String> {
        let mut hasher = Sha1::new();
        hasher.update(password.as_bytes());
        let hash_bytes = hasher.finalize();
        let hash_hex = hex::encode_upper(hash_bytes);

        let prefix = &hash_hex[..5];
        let suffix = &hash_hex[5..];

        let url = format!("{}{}", HIBP_API_URL, prefix);

        let response = self
            .http
            .get(&url)
            .header("Add-Padding", "true")
            .send()
            .await
            .map_err(|e| format!("HIBP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HIBP API returned status {}", response.status()));
        }

        let body = response
            .text()
            .await
            .map_err(|e| format!("HIBP response read failed: {}", e))?;

        let count = parse_hibp_response(&body, suffix);

        Ok(count)
    }
}

impl Default for HibpClient {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_hibp_response(body: &str, target_suffix: &str) -> u32 {
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(colon_pos) = line.find(':') {
            let suffix = &line[..colon_pos];
            if suffix.eq_ignore_ascii_case(target_suffix) {
                let count_str = &line[colon_pos + 1..];
                if let Ok(count) = count_str.trim().parse::<u32>() {
                    return count;
                }
            }
        }
    }
    0
}

pub fn sha1_hex(password: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    hex::encode_upper(result)
}

pub struct SafePassword(String);

impl SafePassword {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Drop for SafePassword {
    fn drop(&mut self) {
        let bytes = unsafe { self.0.as_bytes_mut() };
        bytes.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hibp_response_found() {
        let body = "00A1B2C3D4E5F6A7B8C9D0E1F2A3:5\n0FF1E2D3C4B5A6978899AABB:123\n";
        let count = parse_hibp_response(body, "0FF1E2D3C4B5A6978899AABB");
        assert_eq!(count, 123);
    }

    #[test]
    fn test_parse_hibp_response_not_found() {
        let body = "00A1B2C3D4E5F6A7B8C9D0E1F2A3:5\n";
        let count = parse_hibp_response(body, "FFFFFFFFFFFFFFFFFFFFFFFFF");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_sha1_hex() {
        let hash = sha1_hex("test");
        assert_eq!(hash.len(), 40);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_safe_password_zeroize() {
        let mut safe = SafePassword::new("sensitive_password".to_string());
        assert_eq!(safe.as_str(), "sensitive_password");
        drop(safe);
    }
}
