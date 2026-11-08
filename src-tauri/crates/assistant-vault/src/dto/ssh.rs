use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct SshKeyInfoDto {
    pub fingerprint: String,
    pub key_type: String,
    pub comment: String,
    pub bits: u32,
    pub added_by_vault: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub struct SshKeyMetaDto {
    pub key_type: String,
    pub bits: u32,
    pub fingerprint: String,
    pub comment: String,
    pub public_key: String,
}
