use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SiblingInfoDto {
    pub id: String,
    pub sibling_index: i64,
    pub content_preview: String,
}
