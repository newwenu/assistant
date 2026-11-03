use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub mod totp;

pub use totp::TotpConfig;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "vault.contract.ts")]
pub enum VaultCategory {
    #[default]
    Login,
    Note,
    Card,
    Identity,
    SshKey,
    Custom,
}

impl fmt::Display for VaultCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login => write!(f, "login"),
            Self::Note => write!(f, "note"),
            Self::Card => write!(f, "card"),
            Self::Identity => write!(f, "identity"),
            Self::SshKey => write!(f, "sshKey"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl VaultCategory {
    pub fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }
}

impl std::str::FromStr for VaultCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "login" => Ok(Self::Login),
            "note" => Ok(Self::Note),
            "card" => Ok(Self::Card),
            "identity" => Ok(Self::Identity),
            "sshKey" => Ok(Self::SshKey),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("unknown VaultCategory: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "vault.contract.ts")]
pub struct CustomFieldInput {
    pub name: String,
    pub value: String,
    pub is_protected: bool,
}
