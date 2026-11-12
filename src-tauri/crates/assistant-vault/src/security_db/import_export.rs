use crate::dto::VaultEntryCreate;
use crate::types::{VaultEntryExport, VaultEntrySnapshot, VaultError};
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, Dek};

use super::SecurityDbState;

impl SecurityDbState {
    pub fn export_entries(&self) -> Result<Vec<VaultEntryExport>, VaultError> {
        let ids: Vec<String> = self.with_conn(|conn| {
            let mut stmt = conn
                .prepare("SELECT id FROM vault_entries WHERE deleted_at IS NULL")
                .map_err(|e| VaultError::Database(e.to_string()))?;
            let rows = stmt
                .query_map([], |row| row.get(0))
                .map_err(|e| VaultError::Database(e.to_string()))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })?;

        let mut exports = Vec::new();
        for id in &ids {
            let decrypted = self.decrypt_entry(id)?;
            let totp = decrypted
                .totp_secret
                .as_ref()
                .and_then(|payload| payload.to_str().ok())
                .and_then(crate::shared::totp::TotpConfig::from_stored);
            exports.push(VaultEntryExport {
                category: decrypted.category,
                title: decrypted.title,
                username: decrypted.username,
                url: decrypted.url,
                password: decrypted
                    .password
                    .as_ref()
                    .and_then(|pt| pt.to_str().ok())
                    .map(String::from),
                notes: decrypted
                    .notes
                    .as_ref()
                    .and_then(|pt| pt.to_str().ok())
                    .map(String::from),
                totp_secret: totp
                    .as_ref()
                    .and_then(|config| config.secret().to_str().ok())
                    .map(String::from),
                totp_digits: totp.as_ref().map(|config| config.digits()).unwrap_or(6),
                totp_period: totp.as_ref().map(|config| config.period()).unwrap_or(30),
                totp_algorithm: totp
                    .as_ref()
                    .map(|config| config.algorithm().as_str().to_string())
                    .unwrap_or_else(|| "sha1".to_string()),
                tags: decrypted.tags,
                custom_fields: decrypted
                    .custom_fields
                    .iter()
                    .map(|cf| crate::types::CustomFieldInput {
                        name: cf.name.clone(),
                        value: cf.value.to_str().ok().unwrap_or("").to_string(),
                        is_protected: cf.is_protected,
                    })
                    .collect(),
                is_favorite: decrypted.is_favorite,
                created_at: decrypted.created_at,
                updated_at: decrypted.updated_at,
            });
        }
        Ok(exports)
    }

    pub fn import_entries<F>(
        &self,
        entries: Vec<VaultEntryCreate>,
        algo: CipherAlgorithm,
        mut export: F,
    ) -> Result<u32, VaultError>
    where
        F: FnMut(&Dek, &VaultEntrySnapshot) -> Result<(), VaultError>,
    {
        let existing = self.export_entries()?;
        let mut count = 0u32;
        for entry in entries {
            let is_exact_dup = existing.iter().any(|e| {
                e.category == entry.category
                    && e.title == entry.title
                    && e.username == entry.username
                    && e.url == entry.url
                    && e.password == entry.password
                    && e.notes == entry.notes
                    && e.totp_secret.as_deref()
                        == entry
                            .totp
                            .as_ref()
                            .and_then(|totp| totp.secret.as_deref())
                            .map(str::trim)
                            .filter(|secret| !secret.is_empty())
                    && e.tags == entry.tags
                    && e.is_favorite == entry.is_favorite
                    && custom_fields_eq(&e.custom_fields, &entry.custom_fields)
            });
            if is_exact_dup {
                log::warn!(
                    "Skipping exact duplicate on import: {} ({})",
                    entry.title,
                    entry.category
                );
                continue;
            }
            self.create_entry_tx(entry, algo, |dek, snapshot| export(dek, snapshot))?;
            count += 1;
        }
        Ok(count)
    }
}

fn custom_fields_eq(
    a: &[crate::types::CustomFieldInput],
    b: &[crate::types::CustomFieldInput],
) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a_sorted: Vec<_> = a.iter().collect();
    let mut b_sorted: Vec<_> = b.iter().collect();
    a_sorted.sort_by_key(|f| &f.name);
    b_sorted.sort_by_key(|f| &f.name);
    a_sorted
        .iter()
        .zip(b_sorted.iter())
        .all(|(a, b)| a.name == b.name && a.value == b.value && a.is_protected == b.is_protected)
}
