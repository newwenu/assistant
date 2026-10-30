use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HibpAuditEntryDto {
    pub entry_id: String,
    pub match_count: u32,
}
