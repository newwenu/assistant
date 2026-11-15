mod common;

use assistant_vault::conf::{VaultCipherAlgorithm, VaultConf};
use common::conf_path;

#[test]
fn new_returns_defaults() {
    let conf = VaultConf::new();
    assert_eq!(conf.vault_auto_lock_minutes, 5);
    assert!(conf.vault_lock_on_screen_lock);
    assert_eq!(conf.vault_clipboard_clear_seconds, 30);
    assert_eq!(conf.vault_cipher_algorithm, VaultCipherAlgorithm::Aes256Gcm);
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
    assert_eq!(conf.vault_cipher_algorithm, VaultCipherAlgorithm::Aes256Gcm);
    assert_eq!(conf.vault_clipboard_clear_seconds, 30);
    assert!(conf.vault_lock_on_screen_lock);
}

#[test]
fn amend_single_and_multiple_fields() {
    let conf = VaultConf::new();
    let patch = serde_json::json!({
        "vault_auto_lock_minutes": 10,
        "vault_hibp_auto_audit": false,
        "vault_ssh_auto_add": true
    });
    let merged = conf.amend(patch).unwrap();
    assert_eq!(merged.vault_auto_lock_minutes, 10);
    assert!(!merged.vault_hibp_auto_audit);
    assert!(merged.vault_ssh_auto_add);
    assert_eq!(
        merged.vault_cipher_algorithm,
        VaultCipherAlgorithm::Aes256Gcm
    );
}

#[test]
fn amend_unknown_key_rejected() {
    let conf = VaultConf::new();
    let patch = serde_json::json!({ "nonexistent": 42, "vault_ssh_auto_add": true });
    let err = conf.amend(patch).unwrap_err();
    assert!(err.contains("nonexistent"), "unexpected error: {}", err);
}

#[test]
fn deserialize_rejects_unknown_cipher() {
    let json = r#"{"vault_cipher_algorithm":"des"}"#;
    assert!(serde_json::from_str::<VaultConf>(json).is_err());
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
    assert_eq!(
        conf.vault_cipher_algorithm,
        VaultCipherAlgorithm::Chacha20Poly1305
    );
    assert!(conf.vault_ssh_auto_add);
    assert!(vault_path.exists());
}

#[test]
fn t_c3_zero_auto_lock_is_valid() {
    let mut conf = VaultConf::new();
    conf.vault_auto_lock_minutes = 0;
    assert!(conf.validate().is_ok());
}

#[test]
fn t_c4_excessive_auto_lock_is_rejected() {
    let mut conf = VaultConf::new();
    conf.vault_auto_lock_minutes = u16::MAX;
    let err = conf.validate().unwrap_err();
    assert!(
        err.contains("vault_auto_lock_minutes"),
        "unexpected: {}",
        err
    );

    let mut boundary = VaultConf::new();
    boundary.vault_auto_lock_minutes = 24 * 60;
    assert!(boundary.validate().is_ok());
}

#[test]
fn t_c4_out_of_range_auto_lock_fails_deserialization() {
    let path = conf_path("vault-config.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let raw = br#"{"vault_auto_lock_minutes": 99999}"#;
    std::fs::write(&path, raw).unwrap();
    assert!(VaultConf::load_from(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), raw.to_vec());
}

#[test]
fn t_c5_zero_clipboard_clear_is_valid_but_excessive_rejected() {
    let mut conf = VaultConf::new();
    conf.vault_clipboard_clear_seconds = 0;
    assert!(conf.validate().is_ok());

    let mut excessive = VaultConf::new();
    excessive.vault_clipboard_clear_seconds = 601;
    let err = excessive.validate().unwrap_err();
    assert!(
        err.contains("vault_clipboard_clear_seconds"),
        "unexpected: {}",
        err
    );

    let mut boundary = VaultConf::new();
    boundary.vault_clipboard_clear_seconds = 600;
    assert!(boundary.validate().is_ok());
}

#[test]
fn t_c6_unknown_cipher_is_rejected_on_load() {
    let path = conf_path("vault-config.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, br#"{"vault_cipher_algorithm":"des"}"#).unwrap();
    assert!(VaultConf::load_from(&path).is_err());
}

#[test]
fn t_c12_corrupt_config_reports_error_and_is_not_overwritten() {
    let path = conf_path("vault-config.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let corrupt = br#"{"vault_auto_lock_minutes": 5, broken json"#;
    std::fs::write(&path, corrupt).unwrap();

    let err = VaultConf::load_from(&path).unwrap_err();
    assert!(err.to_string().contains("corrupt"), "unexpected: {}", err);
    assert_eq!(std::fs::read(&path).unwrap(), corrupt.to_vec());
}

#[test]
fn t_c12_invalid_value_is_not_overwritten() {
    let path = conf_path("vault-config.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let raw = br#"{"vault_auto_lock_minutes": 2000}"#;
    std::fs::write(&path, raw).unwrap();

    let err = VaultConf::load_from(&path).unwrap_err();
    assert!(
        err.to_string().contains("vault_auto_lock_minutes"),
        "unexpected: {}",
        err
    );
    assert_eq!(std::fs::read(&path).unwrap(), raw.to_vec());
}

#[test]
fn remote_url_forms_are_validated() {
    let mut conf = VaultConf::new();
    for url in [
        "https://github.com/user/vault.git",
        "https://user:token@github.com/user/vault.git",
        "http://git.example.com/vault.git",
        "ssh://git@github.com/user/vault.git",
        "git@github.com:user/vault.git",
    ] {
        conf.vault_sync_remote_url = Some(url.to_string());
        assert!(conf.validate().is_ok(), "should accept {}", url);
    }

    for url in [
        "ftp://example.com/vault.git",
        "github.com/user/vault.git",
        "https://",
    ] {
        conf.vault_sync_remote_url = Some(url.to_string());
        assert!(conf.validate().is_err(), "should reject {}", url);
    }

    conf.vault_sync_remote_url = Some("https://github.com/a b/vault.git".to_string());
    assert!(conf.validate().is_err());

    conf.vault_sync_remote_url = Some("   ".to_string());
    assert!(conf.validate().is_ok());
}

#[test]
fn auth_sock_override_requires_absolute_path() {
    let mut conf = VaultConf::new();
    conf.vault_ssh_auth_sock_override = String::new();
    assert!(conf.validate().is_ok());

    conf.vault_ssh_auth_sock_override = std::env::temp_dir()
        .join("agent.sock")
        .to_string_lossy()
        .into_owned();
    assert!(conf.validate().is_ok());

    conf.vault_ssh_auth_sock_override = "relative/agent.sock".to_string();
    assert!(conf.validate().is_err());
}

#[test]
fn ssh_hosts_reject_bare_name_violations() {
    let mut conf = VaultConf::new();
    conf.vault_sync_ssh_hosts = vec!["github.com".to_string(), "gitlab.com".to_string()];
    assert!(conf.validate().is_ok());

    conf.vault_sync_ssh_hosts = vec!["".to_string()];
    assert!(conf.validate().is_err());

    conf.vault_sync_ssh_hosts = vec!["git hub.com".to_string()];
    assert!(conf.validate().is_err());

    conf.vault_sync_ssh_hosts = vec!["https://github.com".to_string()];
    assert!(conf.validate().is_err());
}

#[test]
fn amend_rejects_invalid_auto_lock_through_validate() {
    let conf = VaultConf::new();
    let merged = conf
        .amend(serde_json::json!({ "vault_auto_lock_minutes": 2000 }))
        .unwrap();
    assert!(merged.validate().is_err());
}

fn crate_source_files() -> Vec<std::path::PathBuf> {
    fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let entries = std::fs::read_dir(dir).expect("vault crate src directory is readable");
        for entry in entries {
            let path = entry.expect("vault crate src entry is readable").path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    let mut files = Vec::new();
    collect(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    files.sort();
    files
}

#[test]
fn vault_crate_does_not_reference_core_conf_groups() {
    let offenders: Vec<String> = crate_source_files()
        .into_iter()
        .filter_map(|file| {
            let text = std::fs::read_to_string(&file).ok()?;
            let hits: Vec<&str> = ["app_conf", "orb_conf", "mascot_conf"]
                .into_iter()
                .filter(|marker| text.contains(marker))
                .collect();
            if hits.is_empty() {
                None
            } else {
                Some(format!("{}: {:?}", file.display(), hits))
            }
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "assistant-vault 不得引用核心配置组（R9 禁止跨组依赖）: {:?}",
        offenders
    );
}
