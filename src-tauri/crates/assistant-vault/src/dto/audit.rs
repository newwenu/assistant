use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct StrengthResult {
    pub score: u8,
    pub entropy_bits: f64,
    pub crack_time_seconds: u64,
    pub crack_time_display: String,
    pub warnings: Vec<StrengthWarning>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum StrengthWarning {
    TooShort,
    NoUppercase,
    NoLowercase,
    NoDigits,
    NoSpecialChars,
    CommonPattern,
    RepeatedChars,
    SequentialChars,
    DictionaryWord,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct AuditLogEntry {
    pub id: String,
    pub event_type: String,
    pub detail: Option<String>,
    pub timestamp: i64,
}
