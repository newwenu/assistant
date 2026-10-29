use tauri::{AppHandle, Manager};

use assistant_config::app_conf::AppConf;
use assistant_security::clipboard_guard::ClipboardGuard;
use assistant_security::reveal_guard::RevealGuard;
use assistant_security::screen_lock::ScreenLockMonitor;
use assistant_security::security_db::SecurityDbState;
use assistant_sync::sync_engine::SyncEngine;

use crate::protocol::events::{emit_broadcast, event_name};

pub struct SshAgentState {
    pub added_fingerprints: std::sync::Mutex<std::collections::HashSet<String>>,
}

mod audit;
mod biometric;
mod entries;
mod lifecycle;
mod recovery;
mod reveal;
mod ssh;
mod sync;

pub use audit::*;
pub use biometric::*;
pub use entries::*;
pub use lifecycle::*;
pub use recovery::*;
pub use reveal::*;
pub use ssh::*;
pub use sync::*;

pub(crate) fn persist_sync_remote_url(url: Option<&str>) {
    let mut conf = match AppConf::load() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to load AppConf for sync remote url: {}", e);
            return;
        }
    };
    let next = url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty());
    if conf.vault_sync_remote_url == next {
        return;
    }
    conf.vault_sync_remote_url = next;
    if let Err(e) = conf.save() {
        log::warn!("Failed to persist sync remote url: {}", e);
    }
}

pub(crate) fn persist_sync_last_at() {
    let mut conf = match AppConf::load() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to load AppConf for sync last time: {}", e);
            return;
        }
    };
    let now = chrono::Utc::now().timestamp_millis();
    conf.vault_sync_last_at = Some(now);
    if let Err(e) = conf.save() {
        log::warn!("Failed to persist sync last time: {}", e);
    }
}

pub(crate) fn apply_ssh_persist_hosts(conf: &AppConf) {
    assistant_sync::git_sync::ssh::set_ssh_persist_hosts(conf.vault_sync_ssh_hosts.clone());
}

pub(crate) fn ssh_bridge_from_conf() -> Box<dyn assistant_security::ssh_agent::SshAgentBridge> {
    let conf = AppConf::load().ok();
    let use_pageant = conf
        .as_ref()
        .and_then(|c| Some(c.vault_ssh_use_pageant))
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
    use assistant_config::app_conf::AppConf;
    let conf = match AppConf::load() {
        Ok(c) => c,
        Err(_) => return,
    };
    if !conf.vault_ssh_auto_add {
        return;
    }
    if let Err(e) = ssh::vault_ssh_auto_add_all(app.clone()) {
        log::warn!("SSH auto-add on unlock failed: {}", e);
    }
}

pub(crate) fn vault_state(app: &AppHandle) -> Result<tauri::State<'_, SecurityDbState>, String> {
    app.state::<SecurityDbState>()
        .try_into()
        .map_err(|_| "SecurityDbState not managed".to_string())
}

pub(crate) fn cipher_algo() -> assistant_security::vault_crypto::types::CipherAlgorithm {
    let conf = AppConf::load().ok();
    let val = conf
        .as_ref()
        .map(|c| c.vault_cipher_algorithm.as_str())
        .unwrap_or("aes256Gcm");
    SecurityDbState::cipher_algorithm_from_conf(val)
}

pub(crate) fn reveal_guard(app: &AppHandle) -> Result<tauri::State<'_, RevealGuard>, String> {
    app.state::<RevealGuard>()
        .try_into()
        .map_err(|_| "RevealGuard not managed".to_string())
}

pub(crate) fn clipboard_guard(app: &AppHandle) -> Result<tauri::State<'_, ClipboardGuard>, String> {
    app.state::<ClipboardGuard>()
        .try_into()
        .map_err(|_| "ClipboardGuard not managed".to_string())
}

pub(crate) fn get_clipboard_clear_secs(_app: &AppHandle) -> u64 {
    use assistant_config::app_conf::AppConf;
    AppConf::load()
        .map(|c| c.vault_clipboard_clear_seconds as u64)
        .unwrap_or(30)
}

pub(crate) fn sync_state(app: &AppHandle) -> Result<tauri::State<'_, SyncEngine>, String> {
    app.state::<SyncEngine>()
        .try_into()
        .map_err(|_| "SyncEngine not managed".to_string())
}

pub fn setup(handle: &AppHandle) {
    let security_db_path = match handle.path().app_data_dir() {
        Ok(data_dir) => data_dir.join("security").join("security.db"),
        Err(e) => {
            log::error!("Failed to resolve app data dir for security db: {}", e);
            std::path::PathBuf::from("security/security.db")
        }
    };
    handle.manage(SecurityDbState::new(
        security_db_path.to_str().unwrap_or("security/security.db"),
    ));
    handle.manage(RevealGuard::new());
    handle.manage(ClipboardGuard::new());
    handle.manage(SshAgentState {
        added_fingerprints: std::sync::Mutex::new(std::collections::HashSet::new()),
    });
    log::info!("SecurityDbState + RevealGuard + ClipboardGuard + SshAgentState initialized");

    let vault_dir = match handle.path().app_data_dir() {
        Ok(data_dir) => data_dir.join("security").join("vault"),
        Err(_) => std::path::PathBuf::from("security/vault"),
    };
    let sync_config = AppConf::load()
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
    handle.manage(SyncEngine::new(vault_dir, sync_config));
    log::info!("SyncEngine initialized");

    spawn_screen_lock_watcher(handle);
}

fn spawn_screen_lock_watcher(handle: &AppHandle) {
    let monitor = ScreenLockMonitor::new();
    let app_handle = handle.clone();
    let on_lock = {
        let app_handle = app_handle.clone();
        move || {
            ssh_remove_all_on_lock(&app_handle);
            {
                let guard = app_handle.state::<ClipboardGuard>();
                if let Err(e) = guard.force_clear() {
                    log::warn!("Clipboard force_clear on screen lock failed: {}", e);
                }
            }
            let lock_on_screen = AppConf::load()
                .map(|c| c.vault_lock_on_screen_lock)
                .unwrap_or(true);
            if lock_on_screen {
                let state = app_handle.state::<SecurityDbState>();
                if let Err(e) = state.lock() {
                    log::error!("Screen lock triggered vault_lock but failed: {}", e);
                } else {
                    log::info!("Screen lock triggered vault_lock");
                    let initialized = state.is_initialized().unwrap_or(false);
                    if initialized {
                        let app_for_dialog = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) =
                                crate::cmd::vault_show_unlock_window(app_for_dialog).await
                            {
                                log::warn!("Screen lock unlock dialog failed: {}", e);
                            }
                        });
                    }
                }
                let _ = emit_broadcast(&app_handle, event_name::VAULT_LOCKED, ());
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

pub fn cleanup_on_exit(app: &AppHandle) {
    let guard = app.state::<ClipboardGuard>();
    let _ = guard.force_clear();
}

pub fn on_app_conf_changed(prev: &AppConf, conf: &AppConf) {
    if prev.vault_sync_ssh_hosts != conf.vault_sync_ssh_hosts {
        apply_ssh_persist_hosts(conf);
    }
}
