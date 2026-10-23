use crate::types::{VaultEntryCreate, VaultEntryExport, VaultError};
use crate::vault_crypto::types::CipherAlgorithm;

use super::SecurityDbState;

impl SecurityDbState {
    pub fn export_entries(&self) -> Result<Vec<VaultEntryExport>, VaultError> {
        let conn_guard = self.conn.lock().unwrap();
        let conn = conn_guard.as_ref().ok_or(VaultError::Locked)?;

        let ids: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT id FROM vault_entries WHERE deleted_at IS NULL")
                .map_err(|e| VaultError::Database(e.to_string()))?;
            let rows = stmt
                .query_map([], |row| row.get(0))
                .map_err(|e| VaultError::Database(e.to_string()))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        drop(conn_guard);

        let mut exports = Vec::new();
        for id in &ids {
            let decrypted = self.decrypt_entry(id)?;
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
                totp_secret: decrypted
                    .totp_secret
                    .as_ref()
                    .and_then(|pt| pt.to_str().ok())
                    .map(String::from),
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

    pub fn import_entries(
        &self,
        entries: Vec<VaultEntryCreate>,
        algo: CipherAlgorithm,
    ) -> Result<u32, VaultError> {
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
                    && e.totp_secret == entry.totp_secret
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
            self.create_entry(entry, algo)?;
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
