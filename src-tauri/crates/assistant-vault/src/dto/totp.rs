use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TotpAlgorithmDto {
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpResultDto {
    pub code: String,
    pub time_remaining: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedOtpAuthDto {
    pub secret: String,
    pub digits: u32,
    pub period: u32,
    pub algorithm: TotpAlgorithmDto,
    pub issuer: Option<String>,
    pub account: Option<String>,
}
