use tauri::{command, AppHandle};

use assistant_sync::types::{ConflictResolution, SyncConfig};
use assistant_vault::audit::{log as audit_log, AuditEvent};
use assistant_vault::dto::{
    ConflictFileDto, ConflictPreviewDto, ConflictResolutionDto, EntryPreviewDto, HistoryCommitDto,
    PendingRemovalsDto, ReadynessDto, RemoteModeDto, RemoteStatusDto, RevealResult, SyncActionDto,
    SyncConfigDto, SyncResultDto, VaultErrorDto,
};

// precondition: Unlocked
#[command]
pub async fn vault_sync(
    app: AppHandle,
    action: SyncActionDto,
) -> Result<SyncResultDto, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        super::with_unlocked(&app, |db| {
            let engine = super::sync_state(&app)?;
            let result = match action {
                SyncActionDto::Pull => engine.pull(db),
                SyncActionDto::Push => engine.push(db),
                SyncActionDto::Sync => engine.sync_now(db),
                SyncActionDto::ForcePush => engine.force_push(db),
            };
            match result {
                Ok(result) => {
                    if matches!(
                        result,
                        assistant_sync::SyncResult::Success
                            | assistant_sync::SyncResult::UnreadableEntries { .. }
                    ) {
                        super::persist_sync_last_at();
                    }
                    SyncResultDto::try_from(result)
                }
                Err(e) => {
                    match action {
                        SyncActionDto::Pull => log::warn!("Vault sync pull failed: {}", e),
                        SyncActionDto::Push => log::warn!("Vault sync push failed: {}", e),
                        SyncActionDto::Sync => log::warn!("Vault sync failed: {}", e),
                        SyncActionDto::ForcePush => {
                            log::warn!("Vault sync force-push failed: {}", e)
                        }
                    }
                    Err(VaultErrorDto::from(e))
                }
            }
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault sync task join failed: {}", e)))?
}

// precondition: None
#[command]
pub fn vault_sync_last_time() -> Result<Option<i64>, VaultErrorDto> {
    let conf = assistant_vault::conf::VaultConf::load()
        .map_err(|e| super::internal_error(format!("Failed to load vault config: {}", e)))?;
    Ok(conf.vault_sync_last_at)
}

// precondition: AnyState
#[command]
pub fn vault_get_sync_config(app: AppHandle) -> Result<SyncConfigDto, VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    let mut config = engine.get_config();
    if config.remote_url.is_none() {
        if let Ok(Some(url)) = engine.remote_url(&config.remote_name) {
            config.remote_url = Some(url);
        }
    }
    Ok(SyncConfigDto::from(config))
}

// precondition: AnyState
#[command]
pub fn vault_set_sync_config(app: AppHandle, config: SyncConfigDto) -> Result<(), VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    if let Some(url) = config.remote_url.as_deref() {
        if !url.trim().is_empty() {
            engine
                .ensure_remote(&config.remote_name, url)
                .map_err(VaultErrorDto::from)?;
        }
    }
    super::persist_sync_remote_url(config.remote_url.as_deref());
    engine.set_config(SyncConfig::from(config));
    Ok(())
}

// precondition: Unlocked
#[command]
pub async fn vault_resolve_conflict(
    app: AppHandle,
    path: String,
    resolution: ConflictResolutionDto,
    remote_copy_suffix: Option<String>,
) -> Result<(), VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        super::with_unlocked(&app, |db| {
            let engine = super::sync_state(&app)?;
            engine
                .resolve_conflict(
                    db,
                    &path,
                    ConflictResolution::from(resolution),
                    remote_copy_suffix.as_deref(),
                )
                .map_err(VaultErrorDto::from)
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault conflict resolve task join failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub fn vault_sync_pending_removals(
    app: AppHandle,
) -> Result<Option<PendingRemovalsDto>, VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    Ok(engine.pending_removals().map(PendingRemovalsDto::from))
}

// precondition: Unlocked
#[command]
pub async fn vault_sync_confirm_removals(
    app: AppHandle,
    ids: Vec<String>,
) -> Result<u32, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        super::with_unlocked(&app, |db| {
            let engine = super::sync_state(&app)?;
            engine
                .confirm_removals(db, &ids)
                .inspect(|count| {
                    if *count > 0 {
                        super::persist_sync_last_at();
                    }
                })
                .map_err(|e| {
                    log::warn!("Vault sync confirm removals failed: {}", e);
                    VaultErrorDto::from(e)
                })
        })
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault confirm removals task join failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub fn vault_sync_abort(app: AppHandle) -> Result<(), VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    engine.abort_repo_state().map(|_| ()).map_err(|e| {
        log::warn!("Vault sync abort failed: {}", e);
        VaultErrorDto::from(e)
    })
}

// precondition: AnyState
#[command]
pub fn vault_sync_discard(app: AppHandle) -> Result<(), VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    engine.discard_local_changes().map_err(|e| {
        log::warn!("Vault sync discard local changes failed: {}", e);
        VaultErrorDto::from(e)
    })
}

// precondition: AnyState
#[command]
pub fn vault_git_remove_remote(app: AppHandle, remote: String) -> Result<(), VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    engine.remove_remote(&remote).map_err(|e| {
        log::warn!("Vault git remote remove failed: {}", e);
        VaultErrorDto::from(e)
    })?;
    super::persist_sync_remote_url(None);
    let mut config = engine.get_config();
    if config.remote_name == remote {
        config.remote_url = None;
        engine.set_config(config);
    }
    Ok(())
}

// precondition: AnyState
#[command]
pub async fn vault_git_test_connection(
    app: AppHandle,
    url: Option<String>,
) -> Result<usize, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = super::sync_state(&app)?;
        engine.test_connection(url.as_deref()).map_err(|e| {
            log::warn!("Vault git test connection failed: {}", e);
            VaultErrorDto::from(e)
        })
    })
    .await
    .map_err(|e| {
        super::internal_error(format!("Vault git test connection task join failed: {}", e))
    })?
}

// precondition: Unlocked
#[command]
pub async fn vault_git_init(app: AppHandle, url: Option<String>) -> Result<(), VaultErrorDto> {
    match url {
        Some(remote_url) => {
            let persisted_url = remote_url.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let engine = super::sync_state(&app)?;
                engine.clone_repo(&remote_url).map_err(|e| {
                    log::warn!("Vault git clone failed: {}", e);
                    VaultErrorDto::from(e)
                })
            })
            .await
            .map_err(|e| {
                super::internal_error(format!("Vault git clone task join failed: {}", e))
            })??;
            super::persist_sync_remote_url(Some(&persisted_url));
            Ok(())
        }
        None => super::with_unlocked(&app, |db| {
            let engine = super::sync_state(&app)?;
            engine.init(db).map_err(|e| {
                log::warn!("Vault git init failed: {}", e);
                VaultErrorDto::from(e)
            })?;
            let config = engine.get_config();
            if let Some(remote_url) = config.remote_url.as_deref() {
                engine
                    .ensure_remote(&config.remote_name, remote_url)
                    .map_err(VaultErrorDto::from)?;
                super::persist_sync_remote_url(Some(remote_url));
            }
            log::info!("Vault git init completed");
            Ok(())
        }),
    }
}

// precondition: AnyState
#[command]
pub fn vault_git_remote(
    app: AppHandle,
    remote: String,
    url: String,
    mode: RemoteModeDto,
) -> Result<(), VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    match mode {
        RemoteModeDto::Add => engine
            .add_remote(&remote, &url)
            .map_err(VaultErrorDto::from)?,
        RemoteModeDto::Set => engine
            .set_remote(&remote, &url)
            .map_err(VaultErrorDto::from)?,
    }
    super::persist_sync_remote_url(Some(&url));
    Ok(())
}

// precondition: AnyState
#[command]
pub async fn vault_sync_remote_status(
    app: AppHandle,
    fetch: Option<bool>,
) -> Result<RemoteStatusDto, VaultErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = super::sync_state(&app)?;
        let status = engine
            .remote_status(fetch.unwrap_or(false))
            .map_err(VaultErrorDto::from)?;
        Ok(RemoteStatusDto::from(status))
    })
    .await
    .map_err(|e| super::internal_error(format!("Vault remote status task join failed: {}", e)))?
}

// precondition: AnyState
#[command]
pub fn vault_sync_readyness(app: AppHandle) -> Result<ReadynessDto, VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    engine
        .readyness()
        .map(ReadynessDto::from)
        .map_err(VaultErrorDto::from)
}

// precondition: Unlocked
#[command]
pub fn vault_sync_conflicts(app: AppHandle) -> Result<Vec<ConflictFileDto>, VaultErrorDto> {
    super::with_unlocked(&app, |db| {
        let engine = super::sync_state(&app)?;
        engine
            .conflicts(db)
            .map(|files| files.into_iter().map(ConflictFileDto::from).collect())
            .map_err(VaultErrorDto::from)
    })
}

// precondition: AnyState
#[command]
pub fn vault_history_list(
    app: AppHandle,
    entry_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<HistoryCommitDto>, VaultErrorDto> {
    let engine = super::sync_state(&app)?;
    engine
        .history_list(entry_id.as_deref(), limit.unwrap_or(50))
        .map(|commits| commits.into_iter().map(HistoryCommitDto::from).collect())
        .map_err(VaultErrorDto::from)
}

// precondition: Unlocked
#[command]
pub fn vault_history_preview(
    app: AppHandle,
    commit: String,
    entry_id: String,
) -> Result<EntryPreviewDto, VaultErrorDto> {
    super::with_unlocked(&app, |db| {
        let engine = super::sync_state(&app)?;
        engine
            .history_preview(db, &commit, &entry_id)
            .map(EntryPreviewDto::from)
            .map_err(|e| {
                log::warn!(
                    "Vault history preview failed for {}@{}: {}",
                    entry_id,
                    commit,
                    e
                );
                VaultErrorDto::from(e)
            })
    })
}

// precondition: Unlocked
#[command]
pub fn vault_history_restore(
    app: AppHandle,
    commit: String,
    entry_id: String,
) -> Result<String, VaultErrorDto> {
    super::with_unlocked(&app, |db| {
        let engine = super::sync_state(&app)?;
        let snapshot = engine
            .history_restore(db, &commit, &entry_id)
            .map_err(|e| {
                log::warn!(
                    "Vault history restore failed for {}@{}: {}",
                    entry_id,
                    commit,
                    e
                );
                VaultErrorDto::from(e)
            })?;
        log::info!("Vault entry restored from history: {}@{}", entry_id, commit);
        Ok(snapshot.id)
    })
}

// precondition: Unlocked
#[command]
pub fn vault_conflict_preview(
    app: AppHandle,
    path: String,
) -> Result<ConflictPreviewDto, VaultErrorDto> {
    super::with_unlocked(&app, |db| {
        let engine = super::sync_state(&app)?;
        engine
            .conflict_preview(db, &path)
            .map(ConflictPreviewDto::from)
            .map_err(|e| {
                log::warn!("Vault conflict preview failed for {}: {}", path, e);
                VaultErrorDto::from(e)
            })
    })
}

// precondition: Unlocked
#[command]
pub fn vault_conflict_reveal(
    app: AppHandle,
    path: String,
    side: String,
    field: String,
) -> Result<RevealResult, VaultErrorDto> {
    super::with_unlocked(&app, |db| {
        let engine = super::sync_state(&app)?;

        let guard = super::reveal_guard(&app)?;
        if let Err(e) = guard.check_and_record() {
            if let Ok(conn_guard) = db.get_conn() {
                if let Some(conn) = conn_guard.as_ref() {
                    let detail = assistant_vault::sync_engine::conflict_audit_detail(
                        &path, &side, &field, false,
                    );
                    audit_log(conn, AuditEvent::VaultRevealRateLimited, Some(&detail));
                }
            }
            return Err(VaultErrorDto::from(e));
        }

        let value = engine
            .reveal_conflict_field(db, &path, &side, &field)
            .map_err(|e| {
                log::warn!(
                    "Vault conflict reveal failed for {} ({}): {}",
                    path,
                    side,
                    e
                );
                VaultErrorDto::from(e)
            })?;

        let expires_at =
            chrono::Utc::now().timestamp_millis() + (super::REVEAL_TTL_SECS as i64) * 1000;

        if let Ok(conn_guard) = db.get_conn() {
            if let Some(conn) = conn_guard.as_ref() {
                let detail =
                    assistant_vault::sync_engine::conflict_audit_detail(&path, &side, &field, true);
                audit_log(conn, AuditEvent::VaultReveal, Some(&detail));
            }
        }

        Ok(RevealResult { value, expires_at })
    })
}
