use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::conf::{VaultCipherAlgorithm, VaultConf};

/// 保险库加密算法（封闭集合，线格式与前端联合一致）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum VaultCipherAlgorithmDto {
    Aes256Gcm,
    Chacha20Poly1305,
}

impl From<VaultCipherAlgorithm> for VaultCipherAlgorithmDto {
    fn from(value: VaultCipherAlgorithm) -> Self {
        match value {
            VaultCipherAlgorithm::Aes256Gcm => Self::Aes256Gcm,
            VaultCipherAlgorithm::Chacha20Poly1305 => Self::Chacha20Poly1305,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "vault.contract.ts")]
pub struct VaultConfDto {
    pub vault_auto_lock_minutes: u16,
    pub vault_lock_on_screen_lock: bool,
    pub vault_clipboard_clear_seconds: u16,
    pub vault_cipher_algorithm: VaultCipherAlgorithmDto,
    pub vault_hibp_auto_audit: bool,
    pub vault_sync_remote_url: Option<String>,
    pub vault_ssh_auto_add: bool,
    pub vault_ssh_use_pageant: bool,
    pub vault_ssh_use_openssh_agent: bool,
    pub vault_ssh_auth_sock_override: String,
    pub vault_sync_last_at: Option<i64>,
    pub vault_sync_ssh_hosts: Vec<String>,
}

impl From<VaultConf> for VaultConfDto {
    fn from(c: VaultConf) -> Self {
        Self {
            vault_auto_lock_minutes: c.vault_auto_lock_minutes,
            vault_lock_on_screen_lock: c.vault_lock_on_screen_lock,
            vault_clipboard_clear_seconds: c.vault_clipboard_clear_seconds,
            vault_cipher_algorithm: c.vault_cipher_algorithm.into(),
            vault_hibp_auto_audit: c.vault_hibp_auto_audit,
            vault_sync_remote_url: c.vault_sync_remote_url,
            vault_ssh_auto_add: c.vault_ssh_auto_add,
            vault_ssh_use_pageant: c.vault_ssh_use_pageant,
            vault_ssh_use_openssh_agent: c.vault_ssh_use_openssh_agent,
            vault_ssh_auth_sock_override: c.vault_ssh_auth_sock_override,
            vault_sync_last_at: c.vault_sync_last_at,
            vault_sync_ssh_hosts: c.vault_sync_ssh_hosts,
        }
    }
}
