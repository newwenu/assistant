use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrengthResult {
    pub score: u8,
    pub entropy_bits: f64,
    pub crack_time_seconds: u64,
    pub crack_time_display: String,
    pub warnings: Vec<StrengthWarning>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    pub id: String,
    pub event_type: String,
    pub detail: Option<String>,
    pub timestamp: i64,
}
