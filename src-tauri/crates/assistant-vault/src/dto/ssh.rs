use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyInfoDto {
    pub fingerprint: String,
    pub key_type: String,
    pub comment: String,
    pub bits: u32,
    pub added_by_vault: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyMetaDto {
    pub key_type: String,
    pub bits: u32,
    pub fingerprint: String,
    pub comment: String,
    pub public_key: String,
}
