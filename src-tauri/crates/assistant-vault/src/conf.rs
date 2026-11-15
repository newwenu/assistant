use assistant_config::conf_group::{ConfGroup, LoadPolicy};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 自动锁定分钟数上限（24 小时）。
pub const MAX_AUTO_LOCK_MINUTES: u16 = 24 * 60;
/// 剪贴板自动清除秒数上限（10 分钟）。
pub const MAX_CLIPBOARD_CLEAR_SECONDS: u16 = 600;

fn default_true() -> bool {
    true
}

fn default_vault_auto_lock_minutes() -> u16 {
    5
}

fn default_vault_clipboard_clear_seconds() -> u16 {
    30
}

/// 保险库加密算法（封闭集合）。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VaultCipherAlgorithm {
    #[serde(rename = "aes256Gcm")]
    #[default]
    Aes256Gcm,
    #[serde(rename = "chacha20Poly1305")]
    Chacha20Poly1305,
}

impl VaultCipherAlgorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aes256Gcm => "aes256Gcm",
            Self::Chacha20Poly1305 => "chacha20Poly1305",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VaultConf {
    #[serde(default = "default_vault_auto_lock_minutes")]
    pub vault_auto_lock_minutes: u16,
    #[serde(default = "default_true")]
    pub vault_lock_on_screen_lock: bool,
    #[serde(default = "default_vault_clipboard_clear_seconds")]
    pub vault_clipboard_clear_seconds: u16,
    #[serde(default)]
    pub vault_cipher_algorithm: VaultCipherAlgorithm,
    #[serde(default = "default_true")]
    pub vault_hibp_auto_audit: bool,
    #[serde(default)]
    pub vault_sync_remote_url: Option<String>,
    #[serde(default)]
    pub vault_ssh_auto_add: bool,
    #[serde(default = "default_true")]
    pub vault_ssh_use_pageant: bool,
    #[serde(default = "default_true")]
    pub vault_ssh_use_openssh_agent: bool,
    #[serde(default)]
    pub vault_ssh_auth_sock_override: String,
    #[serde(default)]
    pub vault_sync_last_at: Option<i64>,
    #[serde(default)]
    pub vault_sync_ssh_hosts: Vec<String>,
}

impl VaultConf {
    pub fn new() -> Self {
        Self {
            vault_auto_lock_minutes: 5,
            vault_lock_on_screen_lock: true,
            vault_clipboard_clear_seconds: 30,
            vault_cipher_algorithm: VaultCipherAlgorithm::Aes256Gcm,
            vault_hibp_auto_audit: true,
            vault_sync_remote_url: None,
            vault_ssh_auto_add: false,
            vault_ssh_use_pageant: true,
            vault_ssh_use_openssh_agent: true,
            vault_ssh_auth_sock_override: String::new(),
            vault_sync_last_at: None,
            vault_sync_ssh_hosts: Vec::new(),
        }
    }

    fn validate_auth_sock(&self) -> Result<(), String> {
        let raw = &self.vault_ssh_auth_sock_override;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        if raw.chars().any(|c| c.is_control()) {
            return Err("vault_ssh_auth_sock_override must not contain control characters".into());
        }
        if !Path::new(trimmed).is_absolute() {
            return Err(format!(
                "vault_ssh_auth_sock_override must be an absolute path, got {}",
                trimmed
            ));
        }
        Ok(())
    }

    fn validate_remote_url(&self) -> Result<(), String> {
        let Some(raw) = self.vault_sync_remote_url.as_deref() else {
            return Ok(());
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(
                "vault_sync_remote_url must not contain whitespace or control characters".into(),
            );
        }
        if is_supported_remote_url(trimmed) {
            return Ok(());
        }
        Err(format!(
            "vault_sync_remote_url has unsupported form: {} (expected http(s)://, ssh://, file://, user@host:path or an absolute local path)",
            trimmed
        ))
    }

    fn validate_ssh_hosts(&self) -> Result<(), String> {
        for host in &self.vault_sync_ssh_hosts {
            let trimmed = host.trim();
            if trimmed.is_empty() {
                return Err("vault_sync_ssh_hosts must not contain empty entries".into());
            }
            if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(format!(
                    "vault_sync_ssh_hosts entry must be a bare host name, got {}",
                    trimmed
                ));
            }
            if trimmed.contains('@') || trimmed.contains("://") || trimmed.contains('/') {
                return Err(format!(
                    "vault_sync_ssh_hosts entry must be a bare host name, got {}",
                    trimmed
                ));
            }
        }
        Ok(())
    }
}

impl ConfGroup for VaultConf {
    const GROUP: &'static str = "vault";
    const LOG_TAG: &'static str = "VaultConf";
    const FILE_NAME: &'static str = "vault-config.json";
    const LOAD_POLICY: LoadPolicy = LoadPolicy::Strict;

    /// 首次启用保险库时从旧的 `config.json` 接管 vault 字段，随后主文件成为唯一来源。
    fn adopt_legacy(vault_path: &Path) -> Option<Self> {
        let legacy_path = vault_path.parent()?.join("config.json");
        let contents = std::fs::read_to_string(legacy_path).ok()?;
        let value: serde_json::Value = serde_json::from_str(&contents).ok()?;
        serde_json::from_value(value).ok()
    }

    fn validate(&self) -> Result<(), String> {
        // 加密算法已是封闭枚举，非法值会在反序列化阶段被拒绝。
        if self.vault_auto_lock_minutes > MAX_AUTO_LOCK_MINUTES {
            return Err(format!(
                "vault_auto_lock_minutes must be <= {} (0 disables auto-lock), got {}",
                MAX_AUTO_LOCK_MINUTES, self.vault_auto_lock_minutes
            ));
        }
        if self.vault_clipboard_clear_seconds > MAX_CLIPBOARD_CLEAR_SECONDS {
            return Err(format!(
                "vault_clipboard_clear_seconds must be <= {} (0 keeps clipboard), got {}",
                MAX_CLIPBOARD_CLEAR_SECONDS, self.vault_clipboard_clear_seconds
            ));
        }
        self.validate_auth_sock()?;
        self.validate_remote_url()?;
        self.validate_ssh_hosts()?;
        Ok(())
    }
}

assistant_config::conf_group_shims!(VaultConf);

fn is_absolute_local_path(value: &str) -> bool {
    if value.starts_with('/') || value.starts_with("\\\\") {
        return true;
    }
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn is_scp_style_remote(value: &str) -> bool {
    let Some((left, path)) = value.split_once(':') else {
        return false;
    };
    if path.is_empty() {
        return false;
    }
    let Some((user, host)) = left.split_once('@') else {
        return false;
    };
    !user.is_empty() && !host.is_empty() && !host.contains('/') && !user.contains('/')
}

fn is_supported_remote_url(value: &str) -> bool {
    if is_absolute_local_path(value) || value.starts_with("file://") {
        return true;
    }
    for scheme in ["https://", "http://", "ssh://"] {
        if let Some(rest) = value.strip_prefix(scheme) {
            let authority = rest.split('/').next().unwrap_or("");
            let host = authority.rsplit('@').next().unwrap_or("");
            return !host.is_empty();
        }
    }
    is_scp_style_remote(value)
}

impl Default for VaultConf {
    fn default() -> Self {
        Self::new()
    }
}
