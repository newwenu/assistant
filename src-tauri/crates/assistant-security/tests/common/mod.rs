use assistant_security::security_db::SecurityDbState;
use std::path::{Path, PathBuf};

pub const STRONG_PW: &str = "Str0ng#P@ssw0rd!2025";

pub fn temp_db_path() -> PathBuf {
    let dir = std::env::temp_dir().join("assistant-security-test");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("test_vault_{}.db", uuid::Uuid::new_v4()))
}

pub fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
    let backup_dir = path.parent().unwrap().join("backups");
    if backup_dir.exists() {
        let _ = std::fs::remove_dir_all(&backup_dir);
    }
}

pub fn init_vault(path: &Path, password: &str) -> SecurityDbState {
    let _ = std::fs::remove_file(path);
    let db = SecurityDbState::new(path.to_str().unwrap());
    db.init(password).unwrap();
    db
}
