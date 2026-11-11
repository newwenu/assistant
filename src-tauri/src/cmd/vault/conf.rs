use log::error;
use tauri::command;

use assistant_config::vault_conf::VaultConf;

use crate::protocol::VaultConfDto;

// precondition: None
#[command]
pub fn vault_get_conf() -> Result<VaultConfDto, String> {
    VaultConf::load().map(VaultConfDto::from).map_err(|e| {
        error!("Failed to load vault config: {}", e);
        e.to_string()
    })
}
