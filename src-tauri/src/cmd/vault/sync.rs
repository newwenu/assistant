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
        let db = app.state::<assistant_security::security_db::SecurityDbState>();
        let result = match action {
            SyncAction::Pull => engine.pull(&db),
            SyncAction::Push => engine.push(),
            SyncAction::Sync => engine.sync_now(&db),
            SyncAction::ForcePush => engine.force_push(),
        };
        result
            .map(|result| {
                if matches!(
                    result,
                    assistant_sync::SyncResult::Success
                        | assistant_sync::SyncResult::UpToDate
                        | assistant_sync::SyncResult::UnreadableEntries { .. }
                ) {
                    super::persist_sync_last_at();
                }
                result
            })
            .map_err(|e| {
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
pub fn vault_sync_last_time() -> Option<i64> {
    assistant_config::app_conf::AppConf::load()
        .ok()
        .and_then(|c| c.vault_sync_last_at)
}

#[command]
pub fn vault_get_sync_config(app: AppHandle) -> Result<assistant_sync::SyncConfig, String> {
    let engine = super::sync_state(&app)?;
    let mut config = engine.get_config();
    if config.remote_url.is_none() {
        if let Ok(Some(url)) = engine.remote_url(&config.remote_name) {
            config.remote_url = Some(url);
        }
    }
    Ok(config)
}

#[command]
pub fn vault_set_sync_config(
    app: AppHandle,
    config: assistant_sync::SyncConfig,
) -> Result<(), String> {
    let engine = super::sync_state(&app)?;
    if let Some(url) = config.remote_url.as_deref() {
        if !url.trim().is_empty() {
            engine
                .ensure_remote(&config.remote_name, url)
                .map_err(|e| e.to_string())?;
        }
    }
    super::persist_sync_remote_url(config.remote_url.as_deref());
    engine.set_config(config);
    Ok(())
}

#[command]
pub async fn vault_resolve_conflict(
    app: AppHandle,
    path: String,
    resolution: assistant_sync::ConflictResolution,
    remote_copy_suffix: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = app.state::<SyncEngine>();
        let db = app.state::<assistant_security::security_db::SecurityDbState>();
        engine
            .resolve_conflict(&db, &path, resolution, remote_copy_suffix.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub fn vault_sync_pending_removals(
    app: AppHandle,
) -> Result<Option<assistant_sync::PendingRemovals>, String> {
    let engine = super::sync_state(&app)?;
    Ok(engine.pending_removals())
}

#[command]
pub async fn vault_sync_confirm_removals(app: AppHandle, ids: Vec<String>) -> Result<u32, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = app.state::<SyncEngine>();
        let db = app.state::<assistant_security::security_db::SecurityDbState>();
        engine
            .confirm_removals(&db, &ids)
            .map(|count| {
                if count > 0 {
                    super::persist_sync_last_at();
                }
                count
            })
            .map_err(|e| {
                log::warn!("Vault sync confirm removals failed: {}", e);
                e.to_string()
            })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub fn vault_sync_abort(app: AppHandle) -> Result<(), String> {
    let engine = super::sync_state(&app)?;
    engine.abort_repo_state().map(|_| ()).map_err(|e| {
        log::warn!("Vault sync abort failed: {}", e);
        e.to_string()
    })
}

#[command]
pub fn vault_sync_discard(app: AppHandle) -> Result<(), String> {
    let engine = super::sync_state(&app)?;
    engine.discard_local_changes().map_err(|e| {
        log::warn!("Vault sync discard local changes failed: {}", e);
        e.to_string()
    })
}

#[command]
pub fn vault_git_remove_remote(app: AppHandle, remote: String) -> Result<(), String> {
    let engine = super::sync_state(&app)?;
    engine.remove_remote(&remote).map_err(|e| {
        log::warn!("Vault git remote remove failed: {}", e);
        e.to_string()
    })?;
    super::persist_sync_remote_url(None);
    let mut config = engine.get_config();
    if config.remote_name == remote {
        config.remote_url = None;
        engine.set_config(config);
    }
    Ok(())
}

#[command]
pub async fn vault_git_test_connection(
    app: AppHandle,
    url: Option<String>,
) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = app.state::<SyncEngine>();
        engine.test_connection(url.as_deref()).map_err(|e| {
            log::warn!("Vault git test connection failed: {}", e);
            e.to_string()
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[command]
pub async fn vault_git_init(app: AppHandle, url: Option<String>) -> Result<(), String> {
    match url {
        Some(remote_url) => {
            let persisted_url = remote_url.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let engine = app.state::<SyncEngine>();
                engine.clone_repo(&remote_url).map_err(|e| {
                    log::warn!("Vault git clone failed: {}", e);
                    e.to_string()
                })
            })
            .await
            .map_err(|e| e.to_string())??;
            super::persist_sync_remote_url(Some(&persisted_url));
            Ok(())
        }
        None => {
            let engine = super::sync_state(&app)?;
            let db = super::vault_state(&app)?;
            engine.init(&db).map_err(|e| {
                log::warn!("Vault git init failed: {}", e);
                e.to_string()
            })?;
            let config = engine.get_config();
            if let Some(remote_url) = config.remote_url.as_deref() {
                engine
                    .ensure_remote(&config.remote_name, remote_url)
                    .map_err(|e| e.to_string())?;
                super::persist_sync_remote_url(Some(remote_url));
            }
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
        RemoteMode::Add => engine
            .add_remote(&remote, &url)
            .map_err(|e| e.to_string())?,
        RemoteMode::Set => engine
            .set_remote(&remote, &url)
            .map_err(|e| e.to_string())?,
    }
    super::persist_sync_remote_url(Some(&url));
    Ok(())
}

#[command]
pub fn vault_sync_readyness(app: AppHandle) -> Result<assistant_sync::Readyness, String> {
    let engine = super::sync_state(&app)?;
    engine.readyness().map_err(|e| e.to_string())
}

#[command]
pub fn vault_sync_conflicts(app: AppHandle) -> Result<Vec<assistant_sync::ConflictFile>, String> {
    let engine = super::sync_state(&app)?;
    let db = super::vault_state(&app)?;
    engine.conflicts(&db).map_err(|e| e.to_string())
}

#[command]
pub fn vault_history_list(
    app: AppHandle,
    entry_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<assistant_sync::HistoryCommit>, String> {
    let engine = super::sync_state(&app)?;
    engine
        .history_list(entry_id.as_deref(), limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[command]
pub fn vault_history_preview(
    app: AppHandle,
    commit: String,
    entry_id: String,
) -> Result<assistant_sync::EntryPreview, String> {
    let engine = super::sync_state(&app)?;
    let db = super::vault_state(&app)?;
    engine
        .history_preview(&db, &commit, &entry_id)
        .map_err(|e| {
            log::warn!(
                "Vault history preview failed for {}@{}: {}",
                entry_id,
                commit,
                e
            );
            e.to_string()
        })
}

#[command]
pub fn vault_history_restore(
    app: AppHandle,
    commit: String,
    entry_id: String,
) -> Result<String, String> {
    let engine = super::sync_state(&app)?;
    let db = super::vault_state(&app)?;
    let snapshot = engine
        .history_restore(&db, &commit, &entry_id)
        .map_err(|e| {
            log::warn!(
                "Vault history restore failed for {}@{}: {}",
                entry_id,
                commit,
                e
            );
            e.to_string()
        })?;
    log::info!("Vault entry restored from history: {}@{}", entry_id, commit);
    Ok(snapshot.id)
}

#[command]
pub fn vault_conflict_preview(
    app: AppHandle,
    path: String,
) -> Result<assistant_sync::ConflictPreview, String> {
    let engine = super::sync_state(&app)?;
    let db = super::vault_state(&app)?;
    engine.conflict_preview(&db, &path).map_err(|e| {
        log::warn!("Vault conflict preview failed for {}: {}", path, e);
        e.to_string()
    })
}

#[command]
pub fn vault_conflict_reveal(
    app: AppHandle,
    path: String,
    side: String,
    field: String,
) -> Result<String, String> {
    let engine = super::sync_state(&app)?;
    let db = super::vault_state(&app)?;
    engine
        .reveal_conflict_field(&db, &path, &side, &field)
        .map_err(|e| {
            log::warn!(
                "Vault conflict reveal failed for {} ({}): {}",
                path,
                side,
                e
            );
            e.to_string()
        })
}
