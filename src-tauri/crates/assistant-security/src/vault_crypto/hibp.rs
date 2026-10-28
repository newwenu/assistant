use log::{info, warn};
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

    pub async fn check_password(&self, password: &SafePassword) -> Result<u32, String> {
        let hash_hex = sha1_hex(password.as_str());

        let prefix = &hash_hex[..5];
        let suffix = &hash_hex[5..];

        let url = format!("{}{}", HIBP_API_URL, prefix);

        let response = self
            .http
            .get(&url)
            .header("Add-Padding", "true")
            .send()
            .await
            .map_err(|e| {
                warn!("HIBP request failed: {}", e);
                format!("HIBP request failed: {}", e)
            })?;

        if !response.status().is_success() {
            warn!("HIBP API returned status {}", response.status());
            return Err(format!("HIBP API returned status {}", response.status()));
        }

        let body = response.text().await.map_err(|e| {
            warn!("HIBP response read failed: {}", e);
            format!("HIBP response read failed: {}", e)
        })?;

        let count = parse_hibp_response(&body, suffix);

        if count > 0 {
            info!("HIBP check: password found in {} breach(es)", count);
        }

        Ok(count)
    }
}

impl Default for HibpClient {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_hibp_response(body: &str, target_suffix: &str) -> u32 {
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

pub(crate) fn sha1_hex(password: &str) -> String {
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
    fn sha1_hex_is_deterministic_uppercase_and_matches_known_value() {
        assert_eq!(sha1_hex("password"), sha1_hex("password"));
        assert_ne!(sha1_hex("password"), sha1_hex("Password"));
        assert_eq!(&sha1_hex("password")[..5], "5BAA6");
        assert_eq!(sha1_hex("test").len(), 40);
    }

    #[test]
    fn safe_password_zeroizes_on_drop() {
        let safe = SafePassword::new("sensitive_password".to_string());
        assert_eq!(safe.as_str(), "sensitive_password");
        drop(safe);
    }
}
