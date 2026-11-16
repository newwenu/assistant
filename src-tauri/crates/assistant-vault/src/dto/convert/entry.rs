use crate::dto::totp::TotpAlgorithmDto;
use crate::dto::{VaultEntryDetail, VaultEntrySummary};
use crate::types::VaultEntrySnapshot;

impl From<&VaultEntrySnapshot> for VaultEntryDetail {
    fn from(v: &VaultEntrySnapshot) -> Self {
        let totp = v
            .totp_secret
            .as_deref()
            .and_then(crate::shared::totp::TotpConfig::from_stored);
        let totp_algorithm = TotpAlgorithmDto::from(
            totp.as_ref()
                .map(|config| config.algorithm())
                .unwrap_or_default(),
        );
        Self {
            id: v.id.clone(),
            category: v.category,
            title: v.title.clone(),
            username: v.username.clone(),
            url: v.url.clone(),
            has_password: v.password.is_some(),
            has_notes: v.notes.is_some(),
            has_totp: totp.is_some(),
            totp_digits: totp.as_ref().map(|config| config.digits()).unwrap_or(6),
            totp_period: totp.as_ref().map(|config| config.period()).unwrap_or(30),
            totp_algorithm,
            custom_field_names: v
                .custom_fields
                .iter()
                .map(|field| field.name.clone())
                .collect(),
            tags: v.tags.clone(),
            is_favorite: v.is_favorite,
            revision: v.revision,
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}

impl From<&VaultEntrySnapshot> for VaultEntrySummary {
    fn from(v: &VaultEntrySnapshot) -> Self {
        Self {
            id: v.id.clone(),
            category: v.category,
            title: v.title.clone(),
            username: v.username.clone(),
            url: v.url.clone(),
            tags: v.tags.clone(),
            is_favorite: v.is_favorite,
            updated_at: v.updated_at,
        }
    }
}
