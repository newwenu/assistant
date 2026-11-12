use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct RecoveryKeyDto {
    pub paper_key: String,
    pub groups: Vec<String>,
    pub dek_version: u32,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct RecoveryExportDto {
    pub entries: Vec<RecoveryEntryDto>,
    pub skipped: Vec<RecoverySkipDto>,
    pub exported_at: i64,
    pub dek_version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct RecoveryEntryDto {
    pub id: String,
    pub category: String,
    pub blob: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct RecoverySkipDto {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct PaperErrorPosDto {
    pub group: u8,
    pub index: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct RecoveryBackupDto {
    pub dek_version: u32,
    pub confirmed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct ExportResultDto {
    pub path: String,
    pub count: u32,
}
