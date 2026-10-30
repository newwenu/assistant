mod common;

use assistant_config::vault_conf::VaultConf;
use common::conf_path;

#[test]
fn new_returns_defaults() {
    let conf = VaultConf::new();
    assert_eq!(conf.vault_auto_lock_minutes, 5);
    assert!(conf.vault_lock_on_screen_lock);
    assert_eq!(conf.vault_clipboard_clear_seconds, 30);
    assert_eq!(conf.vault_cipher_algorithm, "aes256Gcm");
    assert!(!conf.vault_biometric_enabled);
    assert!(conf.vault_hibp_auto_audit);
    assert_eq!(conf.vault_sync_remote_url, None);
    assert!(!conf.vault_ssh_auto_add);
    assert!(conf.vault_ssh_use_pageant);
    assert!(conf.vault_ssh_use_openssh_agent);
    assert_eq!(conf.vault_ssh_auth_sock_override, "");
    assert_eq!(conf.vault_sync_last_at, None);
    assert!(conf.vault_sync_ssh_hosts.is_empty());
    assert!(conf.validate().is_ok());
}

#[test]
fn serialize_deserialize_roundtrip() {
    let original = VaultConf::new();
    let json = serde_json::to_string(&original).unwrap();
    let restored: VaultConf = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn deserialize_missing_fields_use_defaults() {
    let json = r#"{"vault_auto_lock_minutes":10}"#;
    let conf: VaultConf = serde_json::from_str(json).unwrap();
    assert_eq!(conf.vault_auto_lock_minutes, 10);
    assert_eq!(conf.vault_cipher_algorithm, "aes256Gcm");
    assert_eq!(conf.vault_clipboard_clear_seconds, 30);
    assert!(conf.vault_lock_on_screen_lock);
}

#[test]
fn amend_single_and_multiple_fields() {
    let conf = VaultConf::new();
    let patch = serde_json::json!({
        "vault_auto_lock_minutes": 10,
        "vault_biometric_enabled": true,
        "vault_hibp_auto_audit": false,
        "vault_ssh_auto_add": true
    });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.vault_auto_lock_minutes, 10);
    assert!(merged.vault_biometric_enabled);
    assert!(!merged.vault_hibp_auto_audit);
    assert!(merged.vault_ssh_auto_add);
    assert_eq!(merged.vault_cipher_algorithm, "aes256Gcm");
}

#[test]
fn amend_unknown_key_ignored() {
    let conf = VaultConf::new();
    let patch = serde_json::json!({ "nonexistent": 42, "vault_ssh_auto_add": true });
    let merged = conf.amend(patch).unwrap();
    assert!(merged.vault_ssh_auto_add);
}

#[test]
fn validate_rejects_unknown_cipher() {
    let mut conf = VaultConf::new();
    conf.vault_cipher_algorithm = "aes256Gcm".to_string();
    assert!(conf.validate().is_ok());
    conf.vault_cipher_algorithm = "chacha20Poly1305".to_string();
    assert!(conf.validate().is_ok());
    conf.vault_cipher_algorithm = "des".to_string();
    assert!(conf.validate().is_err());
}

#[test]
fn get_conf_path_uses_vault_config_file() {
    let dir = VaultConf::get_conf_dir().unwrap();
    let path = VaultConf::get_conf_path().unwrap();
    assert!(dir.to_string_lossy().contains("com.assistant"));
    assert!(path.starts_with(&dir));
    assert!(path.to_string_lossy().ends_with("vault-config.json"));
}

#[test]
fn load_from_creates_default_when_missing() {
    let path = conf_path("vault-config.json");
    let conf = VaultConf::load_from(&path).unwrap();
    assert_eq!(conf.vault_auto_lock_minutes, 5);
    assert!(path.exists());
}

#[test]
fn save_to_and_load_from_roundtrip() {
    let path = conf_path("vault-config.json");
    let mut conf = VaultConf::new();
    conf.vault_auto_lock_minutes = 15;
    conf.vault_sync_ssh_hosts = vec!["github.com".to_string()];

    conf.save_to(&path).unwrap();
    let loaded = VaultConf::load_from(&path).unwrap();
    assert_eq!(loaded.vault_auto_lock_minutes, 15);
    assert_eq!(loaded.vault_sync_ssh_hosts, vec!["github.com".to_string()]);
}

#[test]
fn load_from_migrates_legacy_app_conf() {
    let legacy = conf_path("config.json");
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(
        &legacy,
        br#"{"theme":"dark","language":"en-US","vault_auto_lock_minutes":10,
              "vault_cipher_algorithm":"chacha20Poly1305","vault_ssh_auto_add":true}"#,
    )
    .unwrap();

    let vault_path = legacy.parent().unwrap().join("vault-config.json");
    let conf = VaultConf::load_from(&vault_path).unwrap();
    assert_eq!(conf.vault_auto_lock_minutes, 10);
    assert_eq!(conf.vault_cipher_algorithm, "chacha20Poly1305");
    assert!(conf.vault_ssh_auto_add);
    assert!(vault_path.exists());
}
