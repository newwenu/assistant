use tauri::{command, AppHandle, Manager};

use assistant_security::audit::{log as audit_log, AuditEvent};
use assistant_security::types::VaultEntrySummary;

use super::SshAgentState;

#[command]
pub fn vault_ssh_add_to_agent(
    app: AppHandle,
    id: String,
    lifetime: Option<u32>,
) -> Result<String, String> {
    let state = super::vault_state(&app)?;
    let private_key = state
        .decrypt_ssh_private_key(&id)
        .map_err(|e| e.to_string())?;

    let meta = assistant_security::ssh_agent::validate::validate_private_key(private_key.expose())
        .map_err(|e| e.to_string())?;

    let bridge = super::ssh_bridge_from_conf();
    bridge
        .add_key(private_key.expose(), &meta.fingerprint, lifetime)
        .map_err(|e| e.to_string())?;

    {
        let ssh_state = app.state::<SshAgentState>();
        let mut fps = ssh_state
            .added_fingerprints
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        fps.insert(meta.fingerprint.clone());
    }

    {
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::SshKeyLoaded, Some(&id));
        }
    }

    log::info!(
        "SSH key added to agent: {} (entry: {})",
        meta.fingerprint,
        id
    );
    Ok(meta.fingerprint)
}

#[command]
pub fn vault_ssh_remove_from_agent(app: AppHandle, fingerprint: String) -> Result<(), String> {
    let bridge = super::ssh_bridge_from_conf();
    bridge.remove_key(&fingerprint).map_err(|e| e.to_string())?;

    {
        let ssh_state = app.state::<SshAgentState>();
        let mut fps = ssh_state
            .added_fingerprints
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        fps.remove(&fingerprint);
    }

    {
        let state = super::vault_state(&app)?;
        let guard = state.get_conn().map_err(|e| e.to_string())?;
        if let Some(conn) = guard.as_ref() {
            let _ = audit_log(conn, AuditEvent::SshKeyRemoved, Some(&fingerprint));
        }
    }

    log::info!("SSH key removed from agent: {}", fingerprint);
    Ok(())
}

#[command]
pub fn vault_ssh_list_agent(
    app: AppHandle,
) -> Result<Vec<assistant_security::ssh_agent::SshKeyInfo>, String> {
    let bridge = super::ssh_bridge_from_conf();
    let mut keys = bridge.list_keys().map_err(|e| e.to_string())?;

    let ssh_state = app.state::<SshAgentState>();
    let vault_fps = ssh_state
        .added_fingerprints
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    for key in &mut keys {
        key.added_by_vault = vault_fps.contains(&key.fingerprint);
    }

    Ok(keys)
}

#[command]
pub fn vault_ssh_auto_add_all(app: AppHandle) -> Result<Vec<String>, String> {
    use assistant_config::app_conf::AppConf;
    let conf = AppConf::load().map_err(|e| e.to_string())?;
    if !conf.vault_ssh_auto_add {
        return Ok(Vec::new());
    }

    let state = super::vault_state(&app)?;
    let ssh_entries: Vec<VaultEntrySummary> = state
        .list_entries(
            Some(assistant_security::types::VaultCategory::SshKey),
            None,
            None,
        )
        .map_err(|e| e.to_string())?;

    let bridge = super::ssh_bridge_from_conf();
    let mut added = Vec::new();

    for entry in &ssh_entries {
        if let Ok(private_key) = state.decrypt_ssh_private_key(&entry.id) {
            if let Ok(meta) =
                assistant_security::ssh_agent::validate::validate_private_key(private_key.expose())
            {
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
}

#[command]
pub fn vault_ssh_is_agent_running() -> Result<bool, String> {
    let bridge = super::ssh_bridge_from_conf();
    bridge.is_agent_running().map_err(|e| e.to_string())
}

#[command]
pub fn vault_ssh_parse_key(
    private_key: String,
) -> Result<assistant_security::ssh_agent::SshKeyMeta, String> {
    assistant_security::ssh_agent::validate::validate_private_key(private_key.as_bytes())
        .map_err(|e| e.to_string())
}

#[command]
pub fn vault_ssh_key_meta(
    app: AppHandle,
    id: String,
) -> Result<assistant_security::ssh_agent::SshKeyMeta, String> {
    let state = super::vault_state(&app)?;
    let private_key = state
        .decrypt_ssh_private_key(&id)
        .map_err(|e| e.to_string())?;
    assistant_security::ssh_agent::validate::validate_private_key(private_key.expose())
        .map_err(|e| e.to_string())
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
