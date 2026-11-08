use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct DisasterRecoveryResultDto {
    pub file_path: String,
    pub entry_count: u32,
    pub integrity_warning: bool,
    pub skipped_ids: Vec<String>,
}
