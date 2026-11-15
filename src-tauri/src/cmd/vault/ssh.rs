use tauri::{command, AppHandle, Manager};

use assistant_vault::audit::{log as audit_log, AuditEvent};
use assistant_vault::dto::{SshKeyInfoDto, SshKeyMetaDto, VaultEntrySummary, VaultErrorDto};

use super::SshAgentState;

fn key_info_dto(info: assistant_security::ssh_agent::SshKeyInfo) -> SshKeyInfoDto {
    let assistant_security::ssh_agent::SshKeyInfo {
        fingerprint,
        key_type,
        comment,
        bits,
        added_by_vault,
    } = info;
    SshKeyInfoDto {
        fingerprint,
        key_type,
        comment,
        bits,
        added_by_vault,
    }
}

fn key_meta_dto(meta: assistant_security::ssh_agent::SshKeyMeta) -> SshKeyMetaDto {
    let assistant_security::ssh_agent::SshKeyMeta {
        key_type,
        bits,
        fingerprint,
        comment,
        public_key,
    } = meta;
    SshKeyMetaDto {
        key_type,
        bits,
        fingerprint,
        comment,
        public_key,
    }
}

// precondition: Unlocked
#[command]
pub fn vault_ssh_add_to_agent(
    app: AppHandle,
    id: String,
    lifetime: Option<u32>,
) -> Result<String, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let private_key = state
            .decrypt_ssh_private_key(&id)
            .map_err(VaultErrorDto::from)?;

        let meta =
            assistant_security::ssh_agent::validate::validate_private_key(private_key.expose())
                .map_err(VaultErrorDto::from)?;

        let bridge = super::ssh_bridge_from_conf();
        bridge
            .add_key(private_key.expose(), &meta.fingerprint, lifetime)
            .map_err(VaultErrorDto::from)?;

        {
            let ssh_state = app.state::<SshAgentState>();
            let mut fps = ssh_state
                .added_fingerprints
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            fps.insert(meta.fingerprint.clone());
        }

        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        if let Some(conn) = guard.as_ref() {
            audit_log(conn, AuditEvent::SshKeyLoaded, Some(&id));
        }

        log::info!(
            "SSH key added to agent: {} (entry: {})",
            meta.fingerprint,
            id
        );
        Ok(meta.fingerprint)
    })
}

// precondition: Unlocked
#[command]
pub fn vault_ssh_remove_from_agent(
    app: AppHandle,
    fingerprint: String,
) -> Result<(), VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let bridge = super::ssh_bridge_from_conf();
        bridge
            .remove_key(&fingerprint)
            .map_err(VaultErrorDto::from)?;

        {
            let ssh_state = app.state::<SshAgentState>();
            let mut fps = ssh_state
                .added_fingerprints
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            fps.remove(&fingerprint);
        }

        let guard = state.get_conn().map_err(VaultErrorDto::from)?;
        if let Some(conn) = guard.as_ref() {
            // `fingerprint` 是未校验的前端入参：写入前收敛到 `SHA256:{digest}` 形态，否则写 `unknown`。
            let detail = assistant_vault::audit::sanitized_fingerprint(&fingerprint);
            audit_log(conn, AuditEvent::SshKeyRemoved, Some(&detail));
        }

        log::info!("SSH key removed from agent: {}", fingerprint);
        Ok(())
    })
}

// precondition: AnyState
#[command]
pub fn vault_ssh_list_agent(app: AppHandle) -> Result<Vec<SshKeyInfoDto>, VaultErrorDto> {
    let bridge = super::ssh_bridge_from_conf();
    let mut keys = bridge.list_keys().map_err(VaultErrorDto::from)?;

    let ssh_state = app.state::<SshAgentState>();
    let vault_fps = ssh_state
        .added_fingerprints
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    for key in &mut keys {
        key.added_by_vault = vault_fps.contains(&key.fingerprint);
    }

    Ok(keys.into_iter().map(key_info_dto).collect())
}

// precondition: Unlocked
#[command]
pub fn vault_ssh_auto_add_all(app: AppHandle) -> Result<Vec<String>, VaultErrorDto> {
    let conf = assistant_config::vault_conf::VaultConf::load()
        .map_err(|e| super::internal_error(format!("Failed to load vault config: {}", e)))?;
    if !conf.vault_ssh_auto_add {
        return Ok(Vec::new());
    }

    super::with_unlocked(&app, |state| {
        let ssh_entries: Vec<VaultEntrySummary> = state
            .list_entries(
                Some(assistant_vault::types::VaultCategory::SshKey),
                None,
                None,
            )
            .map_err(VaultErrorDto::from)?;

        let bridge = super::ssh_bridge_from_conf();
        let mut added = Vec::new();

        for entry in &ssh_entries {
            if let Ok(private_key) = state.decrypt_ssh_private_key(&entry.id) {
                if let Ok(meta) = assistant_security::ssh_agent::validate::validate_private_key(
                    private_key.expose(),
                ) {
                    if bridge
                        .add_key(private_key.expose(), &meta.fingerprint, None)
                        .is_ok()
                    {
                        {
                            let ssh_state = app.state::<SshAgentState>();
                            let mut fps = ssh_state
                                .added_fingerprints
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            fps.insert(meta.fingerprint.clone());
                        }
                        added.push(meta.fingerprint);
                    }
                }
            }
        }

        if !added.is_empty() {
            log::info!("SSH auto-add: {} keys added to agent", added.len());
        }

        Ok(added)
    })
}

// precondition: None
#[command]
pub fn vault_ssh_is_agent_running() -> Result<bool, VaultErrorDto> {
    let bridge = super::ssh_bridge_from_conf();
    bridge.is_agent_running().map_err(VaultErrorDto::from)
}

// precondition: None
#[command]
pub fn vault_ssh_parse_key(private_key: String) -> Result<SshKeyMetaDto, VaultErrorDto> {
    assistant_security::ssh_agent::validate::validate_private_key(private_key.as_bytes())
        .map(key_meta_dto)
        .map_err(VaultErrorDto::from)
}

// precondition: Unlocked
#[command]
pub fn vault_ssh_key_meta(app: AppHandle, id: String) -> Result<SshKeyMetaDto, VaultErrorDto> {
    super::with_unlocked(&app, |state| {
        let private_key = state
            .decrypt_ssh_private_key(&id)
            .map_err(VaultErrorDto::from)?;
        assistant_security::ssh_agent::validate::validate_private_key(private_key.expose())
            .map(key_meta_dto)
            .map_err(VaultErrorDto::from)
    })
}

pub(super) fn entry_fingerprint(app: &AppHandle, id: &str) -> Option<String> {
    let state = super::vault_state(app).ok()?;
    let private_key = state.decrypt_ssh_private_key(id).ok()?;
    assistant_security::ssh_agent::validate::validate_private_key(private_key.expose())
        .ok()
        .map(|meta| meta.fingerprint)
}

pub(super) fn auto_add_entry(app: &AppHandle, id: &str) {
    let bridge = super::ssh_bridge_from_conf();
    if !bridge.is_agent_running().unwrap_or(false) {
        return;
    }

    let state = match super::vault_state(app) {
        Ok(state) => state,
        Err(_) => return,
    };
    let private_key = match state.decrypt_ssh_private_key(id) {
        Ok(key) => key,
        Err(_) => return,
    };
    let meta =
        match assistant_security::ssh_agent::validate::validate_private_key(private_key.expose()) {
            Ok(meta) => meta,
            Err(_) => return,
        };

    if let Err(e) = bridge.add_key(private_key.expose(), &meta.fingerprint, None) {
        log::warn!(
            "SSH auto-add on create failed ({}): {}",
            meta.fingerprint,
            e
        );
        return;
    }

    let ssh_state = app.state::<SshAgentState>();
    ssh_state
        .added_fingerprints
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(meta.fingerprint.clone());

    log::info!(
        "SSH key auto-added on entry create: {} (entry: {})",
        meta.fingerprint,
        id
    );
}

pub(super) fn auto_remove_fingerprint(app: &AppHandle, fingerprint: &str) {
    let bridge = super::ssh_bridge_from_conf();
    if let Err(e) = bridge.remove_key(fingerprint) {
        log::warn!("SSH auto-remove on delete failed ({}): {}", fingerprint, e);
        return;
    }

    let ssh_state = app.state::<SshAgentState>();
    ssh_state
        .added_fingerprints
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(fingerprint);

    log::info!("SSH key auto-removed on entry delete: {}", fingerprint);
}
