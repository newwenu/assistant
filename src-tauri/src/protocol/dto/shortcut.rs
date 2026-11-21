use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutWarningPayloadDto {
    pub id: String,
    pub binding: String,
    pub error: String,
}
