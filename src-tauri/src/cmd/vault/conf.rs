use log::error;
use tauri::command;

use assistant_vault::conf::VaultConf;

use assistant_vault::dto::VaultConfDto;
use assistant_vault::dto::VaultErrorDto;

// precondition: None
#[command]
pub fn vault_get_conf() -> Result<VaultConfDto, VaultErrorDto> {
    VaultConf::load().map(VaultConfDto::from).map_err(|e| {
        error!("Failed to load vault config: {}", e);
        super::validation_error(format!("Failed to load vault config: {}", e))
    })
}
