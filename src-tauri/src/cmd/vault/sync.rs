use serde::Deserialize;
use tauri::{command, AppHandle, Manager};

use assistant_sync::sync_engine::SyncEngine;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncAction {
    Pull,
    Push,
    Sync,
    ForcePush,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteMode {
    Add,
    Set,
}

#[command]
pub async fn vault_sync(
    app: AppHandle,
    action: SyncAction,
) -> Result<assistant_sync::SyncResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = app.state::<SyncEngine>();
        let result = match action {
            SyncAction::Pull => engine.pull(),
            SyncAction::Push => engine.push(),
            SyncAction::Sync => engine.sync_now(),
            SyncAction::ForcePush => engine.force_push(),
        };
        result.map_err(|e| {
            match action {
                SyncAction::Pull => log::warn!("Vault sync pull failed: {}", e),
                SyncAction::Push => log::warn!("Vault sync push failed: {}", e),
                SyncAction::Sync => log::warn!("Vault sync failed: {}", e),
                SyncAction::ForcePush => log::warn!("Vault sync force-push failed: {}", e),
            }
            e.to_string()
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub fn vault_get_sync_config(app: AppHandle) -> Result<assistant_sync::SyncConfig, String> {
    let engine = super::sync_state(&app)?;
    Ok(engine.get_config())
}

#[command]
pub fn vault_set_sync_config(
    app: AppHandle,
    config: assistant_sync::SyncConfig,
) -> Result<(), String> {
    let engine = super::sync_state(&app)?;
    engine.set_config(config);
    Ok(())
}

#[command]
pub fn vault_resolve_conflict(
    app: AppHandle,
    path: String,
    resolution: assistant_sync::ConflictResolution,
) -> Result<(), String> {
    let engine = super::sync_state(&app)?;
    engine
        .resolve_conflict(&path, resolution)
        .map_err(|e| e.to_string())
}

#[command]
pub async fn vault_git_init(app: AppHandle, url: Option<String>) -> Result<(), String> {
    match url {
        Some(remote_url) => {
            tauri::async_runtime::spawn_blocking(move || {
                let engine = app.state::<SyncEngine>();
                engine.clone_repo(&remote_url).map_err(|e| {
                    log::warn!("Vault git clone failed: {}", e);
                    e.to_string()
                })
            })
            .await
            .map_err(|e| e.to_string())?
        }
        None => {
            let engine = super::sync_state(&app)?;
            engine.init().map_err(|e| {
                log::warn!("Vault git init failed: {}", e);
                e.to_string()
            })?;
            log::info!("Vault git init completed");
            Ok(())
        }
    }
}

#[command]
pub fn vault_git_remote(
    app: AppHandle,
    remote: String,
    url: String,
    mode: RemoteMode,
) -> Result<(), String> {
    let engine = super::sync_state(&app)?;
    match mode {
        RemoteMode::Add => engine.add_remote(&remote, &url).map_err(|e| e.to_string()),
        RemoteMode::Set => engine.set_remote(&remote, &url).map_err(|e| e.to_string()),
    }
}

#[command]
pub fn vault_sync_readyness(app: AppHandle) -> Result<assistant_sync::Readyness, String> {
    let engine = super::sync_state(&app)?;
    engine.readyness().map_err(|e| e.to_string())
}

#[command]
pub fn vault_sync_prepare(app: AppHandle) -> Result<(), String> {
    let engine = super::sync_state(&app)?;
    engine.prepare().map_err(|e| e.to_string())
}

#[command]
pub fn vault_sync_finalize(app: AppHandle, message: String) -> Result<(), String> {
    let engine = super::sync_state(&app)?;
    engine.finalize(&message).map_err(|e| e.to_string())
}