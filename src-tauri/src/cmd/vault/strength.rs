use tauri::command;

use assistant_vault::dto::{StrengthResult, VaultErrorDto};
use assistant_vault::password_strength;

// precondition: None
#[command]
pub fn vault_check_master_password_strength(
    password: String,
) -> Result<StrengthResult, VaultErrorDto> {
    Ok(password_strength::estimate_strength(&password))
}
