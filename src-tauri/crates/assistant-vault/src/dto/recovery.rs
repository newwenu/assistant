use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisasterRecoveryResultDto {
    pub file_path: String,
    pub entry_count: u32,
    pub integrity_warning: bool,
    pub skipped_ids: Vec<String>,
}
