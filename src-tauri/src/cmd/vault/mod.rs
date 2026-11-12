use tauri::{AppHandle, Manager};

use assistant_config::vault_conf::VaultConf;
use assistant_security::clipboard_guard::ClipboardGuard;
use assistant_security::reveal_guard::RevealGuard;
use assistant_security::screen_lock::ScreenLockMonitor;
use assistant_vault::dto::VaultErrorDto;
use assistant_vault::security_db::{LockReason, SecurityDbState};
use assistant_vault::sync_engine::SyncEngine;

use crate::app_dirs::AppDirs;
use crate::protocol::events::{emit_broadcast, event_name};

// 可选能力自持目录名：随 `feature = "vault"` 编译，只从 `AppDirs` 取 data 基根。
// 不加 `vault_` 前缀：`assistant-vault/tests/conf_keys.rs` 会把 `pub vault_*` 当作配置键比对。
const SECURITY_DIR: &str = "security";
const SECURITY_DB_FILE: &str = "security.db";
const VAULT_DIR: &str = "vault";
/// 纸面恢复的临时仓库副本目录：clone 到这里读取，结束后整体删除，从不触碰本机仓库。
const RECOVERY_DIR: &str = "recovery";
/// 重置前的 `.vault-meta` 备份文件名，存放于 `<security>/`（仓库之外）。
///
/// 备份含 `slots[].wrapped_dek`：写进 `<security>/vault/` 会同时撞上
/// `check_repository_layout` 白名单与同步白名单（后者会把备份推上远端）。
const META_BACKUP_FILE: &str = "vault-meta.bak";

pub(crate) fn security_dir(dirs: &AppDirs) -> std::path::PathBuf {
    dirs.data().join(SECURITY_DIR)
}

pub(crate) fn security_db_path(dirs: &AppDirs) -> std::path::PathBuf {
    security_dir(dirs).join(SECURITY_DB_FILE)
}

pub(crate) fn repo_dir(dirs: &AppDirs) -> std::path::PathBuf {
    security_dir(dirs).join(VAULT_DIR)
}

/// 重置前的 meta 备份路径 `<security>/vault-meta.bak`，与仓库同级、不在仓库内。
pub(crate) fn meta_backup_path(dirs: &AppDirs) -> std::path::PathBuf {
    security_dir(dirs).join(META_BACKUP_FILE)
}

/// 把旧实现留在仓库根目录的 `.vault-meta.bak` 迁出仓库。
///
/// 早期重置流程把备份写进 `<security>/vault/`，命中 `check_repository_layout`
/// 白名单校验后会让后续每次写条目都报 `RepositoryContainsForbiddenFile`。
/// 这里只迁移、不丢弃内容；失败仅告警，不阻断启动。
fn relocate_legacy_meta_backup(dirs: &AppDirs) -> std::io::Result<()> {
    let legacy = repo_dir(dirs).join(".vault-meta.bak");
    if !legacy.is_file() {
        return Ok(());
    }
    let target = meta_backup_path(dirs);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&legacy, &target)?;
    std::fs::remove_file(&legacy)?;
    log::info!("Relocated legacy .vault-meta.bak out of the vault repository");
    Ok(())
}

/// 纸面恢复专用的仓库副本目录 `<security>/recovery/`。
///
/// 恢复只读这里，不读也不改本机仓库 `<security>/vault/`；因此换机 / 本机仓库损坏时同样可用。
pub(crate) fn recovery_dir(dirs: &AppDirs) -> std::path::PathBuf {
    security_dir(dirs).join(RECOVERY_DIR)
}

/// 删除 `<security>/recovery/`。
///
/// 两种调用时机：恢复开始前清掉上一次崩溃残留；恢复结束（成功/失败）后清理本次副本。
pub(crate) fn cleanup_recovery_dir(security_root: &std::path::Path) -> std::io::Result<()> {
    let path = security_root.join(RECOVERY_DIR);
    if path.is_dir() {
        std::fs::remove_dir_all(&path)?;
    } else if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

pub(crate) const REVEAL_TTL_SECS: u64 = 30;

pub struct SshAgentState {
    pub added_fingerprints: std::sync::Mutex<std::collections::HashSet<String>>,
}

mod audit;
mod biometric;
mod conf;
mod entries;
mod lifecycle;
mod recovery;
mod reveal;
mod ssh;
mod sync;

pub use audit::*;
pub use biometric::*;
pub use conf::*;
pub use entries::*;
pub use lifecycle::*;
pub use recovery::*;
pub use reveal::*;
pub use ssh::*;
pub use sync::*;

pub(crate) fn persist_sync_remote_url(url: Option<&str>) {
    let mut conf = match VaultConf::load() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to load VaultConf for sync remote url: {}", e);
            return;
        }
    };
    let next = url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty());
    if conf.vault_sync_remote_url == next {
        return;
    }
    conf.vault_sync_remote_url = next;
    if let Err(e) = conf.validate() {
        log::warn!("Refusing to persist invalid sync remote url: {}", e);
        return;
    }
    if let Err(e) = conf.save() {
        log::warn!("Failed to persist sync remote url: {}", e);
    }
}

pub(crate) fn persist_sync_last_at() {
    let mut conf = match VaultConf::load() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to load VaultConf for sync last time: {}", e);
            return;
        }
    };
    let now = chrono::Utc::now().timestamp_millis();
    conf.vault_sync_last_at = Some(now);
    if let Err(e) = conf.save() {
        log::warn!("Failed to persist sync last time: {}", e);
    }
}

pub(crate) fn apply_ssh_persist_hosts(conf: &VaultConf) {
    assistant_sync::git_sync::ssh::set_ssh_persist_hosts(conf.vault_sync_ssh_hosts.clone());
}

pub(crate) fn ssh_bridge_from_conf() -> Box<dyn assistant_security::ssh_agent::SshAgentBridge> {
    let conf = VaultConf::load().ok();
    let use_pageant = conf
        .as_ref()
        .map(|c| c.vault_ssh_use_pageant)
        .unwrap_or(true);
    let auth_sock_override = conf.as_ref().and_then(|c| {
        let s = c.vault_ssh_auth_sock_override.clone();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    });
    assistant_security::ssh_agent::create_agent_bridge(use_pageant, auth_sock_override)
}

pub(crate) fn ssh_remove_all_on_lock(app: &AppHandle) {
    let ssh_state = app.state::<SshAgentState>();
    let fingerprints: Vec<String> = {
        let mut fps = ssh_state
            .added_fingerprints
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        fps.drain().collect()
    };
    if fingerprints.is_empty() {
        return;
    }
    let bridge = ssh_bridge_from_conf();
    for fp in &fingerprints {
        if let Err(e) = bridge.remove_key(fp) {
            log::warn!("Failed to remove SSH key {} from agent: {}", fp, e);
        }
    }
    log::info!(
        "Removed {} SSH keys from agent on vault lock",
        fingerprints.len()
    );
}

pub(crate) fn ssh_auto_add_on_unlock(app: &AppHandle) {
    let conf = match VaultConf::load() {
        Ok(c) => c,
        Err(_) => return,
    };
    if !conf.vault_ssh_auto_add {
        return;
    }
    if let Err(e) = ssh::vault_ssh_auto_add_all(app.clone()) {
        log::warn!("SSH auto-add on unlock failed: {:?}", e);
    }
}

pub(crate) fn internal_error(message: impl Into<String>) -> VaultErrorDto {
    VaultErrorDto::from(assistant_vault::types::VaultError::Other(message.into()))
}

pub(crate) fn validation_error(message: impl Into<String>) -> VaultErrorDto {
    VaultErrorDto::from(assistant_vault::types::VaultError::Validation(
        message.into(),
    ))
}

pub(crate) fn vault_state(
    app: &AppHandle,
) -> Result<tauri::State<'_, SecurityDbState>, VaultErrorDto> {
    Ok(app.state::<SecurityDbState>())
}

pub(crate) fn with_unlocked<T>(
    app: &AppHandle,
    f: impl FnOnce(&SecurityDbState) -> Result<T, VaultErrorDto>,
) -> Result<T, VaultErrorDto> {
    let state = vault_state(app)?;
    drop(state.get_conn().map_err(VaultErrorDto::from)?);
    f(state.inner())
}

pub(crate) fn on_locked(app: &AppHandle) {
    if let Ok(guard) = clipboard_guard(app) {
        if let Err(e) = guard.force_clear() {
            log::warn!("Clipboard force_clear on vault lock failed: {}", e);
        }
    }
    ssh_remove_all_on_lock(app);
    if let Ok(guard) = reveal_guard(app) {
        guard.clear();
    }
    emit_broadcast(app, event_name::VAULT_LOCKED, ());
}

pub(crate) fn cipher_algo() -> assistant_crypto::vault_crypto::types::CipherAlgorithm {
    let conf = VaultConf::load().ok();
    let val = conf
        .as_ref()
        .map(|c| c.vault_cipher_algorithm.as_str())
        .unwrap_or("aes256Gcm");
    SecurityDbState::cipher_algorithm_from_conf(val)
}

pub(crate) fn reveal_guard(
    app: &AppHandle,
) -> Result<tauri::State<'_, RevealGuard>, VaultErrorDto> {
    Ok(app.state::<RevealGuard>())
}

pub(crate) fn clipboard_guard(
    app: &AppHandle,
) -> Result<tauri::State<'_, ClipboardGuard>, VaultErrorDto> {
    Ok(app.state::<ClipboardGuard>())
}

pub(crate) fn get_clipboard_clear_secs(_app: &AppHandle) -> u64 {
    VaultConf::load()
        .map(|c| c.vault_clipboard_clear_seconds as u64)
        .unwrap_or(30)
}

pub(crate) fn sync_state(app: &AppHandle) -> Result<tauri::State<'_, SyncEngine>, VaultErrorDto> {
    Ok(app.state::<SyncEngine>())
}

pub fn vault_setup(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let dirs = AppDirs::resolve()?;
    if let Err(e) = cleanup_recovery_dir(&security_dir(&dirs)) {
        log::warn!("Failed to remove stale recovery directory: {}", e);
    }
    if let Err(e) = relocate_legacy_meta_backup(&dirs) {
        log::warn!("Failed to relocate legacy .vault-meta.bak: {}", e);
    }
    let db_path = security_db_path(&dirs);
    let db_path_str = db_path.to_str().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "安全库路径不是 UTF-8")
    })?;
    handle.manage(SecurityDbState::new(db_path_str));
    handle.manage(RevealGuard::new());
    handle.manage(ClipboardGuard::new());
    handle.manage(SshAgentState {
        added_fingerprints: std::sync::Mutex::new(std::collections::HashSet::new()),
    });
    log::info!("SecurityDbState + RevealGuard + ClipboardGuard + SshAgentState initialized");

    let vault_dir = repo_dir(&dirs);
    let sync_config = VaultConf::load()
        .ok()
        .map(|c| {
            assistant_sync::git_sync::ssh::set_ssh_persist_hosts(c.vault_sync_ssh_hosts.clone());
            c.vault_sync_remote_url
        })
        .and_then(|url| {
            url.map(|url| assistant_sync::SyncConfig {
                remote_url: Some(url),
                ..Default::default()
            })
        })
        .unwrap_or_default();
    handle.manage(SyncEngine::new(
        vault_dir,
        sync_config,
        Box::new(cipher_algo),
    ));
    log::info!("SyncEngine initialized");

    spawn_screen_lock_watcher(handle);
    Ok(())
}

fn spawn_screen_lock_watcher(handle: &AppHandle) {
    let monitor = ScreenLockMonitor::new();
    let app_handle = handle.clone();
    let on_lock = {
        let app_handle = app_handle.clone();
        move || {
            let lock_on_screen = VaultConf::load()
                .map(|c| c.vault_lock_on_screen_lock)
                .unwrap_or(true);
            if lock_on_screen {
                let state = app_handle.state::<SecurityDbState>();
                if let Err(e) = state.lock_session(LockReason::ScreenLock) {
                    log::error!("Screen lock triggered vault_lock but failed: {}", e);
                } else {
                    log::info!("Screen lock triggered vault_lock");
                    on_locked(&app_handle);
                    let initialized = state.is_initialized().unwrap_or(false);
                    if initialized {
                        let app_for_dialog = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) =
                                crate::cmd::vault::vault_show_unlock_window(app_for_dialog).await
                            {
                                log::warn!(
                                    "Screen lock unlock dialog failed: [{:?}] {}",
                                    e.code,
                                    e.message
                                );
                            }
                        });
                    }
                }
            } else {
                log::info!("Screen lock detected but vault_lock_on_screen_lock is disabled");
            }
        }
    };
    let on_unlock = {
        let app_handle = app_handle.clone();
        move || {
            log::info!("Screen unlocked");
            let state = app_handle.state::<SecurityDbState>();
            log::info!("Screen unlocked, vault locked state: {}", state.is_locked());
        }
    };
    monitor.spawn_listener(on_lock, on_unlock);
    handle.manage(monitor);
    log::info!("ScreenLockMonitor initialized");
}

/// 退出路径：先锁库（写 `app_exit` 审计并清零密钥），再清剪贴板 / SSH / 揭示态。
pub fn vault_cleanup_on_exit(app: &AppHandle) {
    let state = app.state::<SecurityDbState>();
    if let Err(e) = state.lock_session(LockReason::AppExit) {
        log::warn!("Vault lock on exit failed: {}", e);
    }
    on_locked(app);
}

pub fn apply_vault_side_effects(prev: &VaultConf, conf: &VaultConf) {
    if prev.vault_sync_ssh_hosts != conf.vault_sync_ssh_hosts {
        apply_ssh_persist_hosts(conf);
    }
}

pub fn apply_conf(_app: &tauri::AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let conf = VaultConf::load().map_err(|e| e.to_string())?;
    let prev = conf.clone();
    let merged = conf.amend(patch)?;
    merged.validate()?;
    merged.save().map_err(|e| e.to_string())?;
    apply_vault_side_effects(&prev, &merged);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_stale_recovery_directory_tree() {
        let dir = tempfile::tempdir().unwrap();
        let security_root = dir.path().join("security");
        let recovery = security_root.join("recovery");
        std::fs::create_dir_all(recovery.join("entries")).unwrap();
        std::fs::write(recovery.join("entries/e1.enc"), b"ciphertext").unwrap();

        cleanup_recovery_dir(&security_root).unwrap();
        assert!(!recovery.exists(), "recovery 目录必须被整体删除");
        assert!(security_root.exists(), "security 根目录必须保留");
    }

    #[test]
    fn cleanup_is_noop_when_recovery_absent() {
        let dir = tempfile::tempdir().unwrap();
        let security_root = dir.path().join("security");
        std::fs::create_dir_all(&security_root).unwrap();

        cleanup_recovery_dir(&security_root).unwrap();
        assert!(security_root.exists());
    }
}
