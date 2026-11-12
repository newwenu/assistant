use rusqlite::{params, Connection, OptionalExtension};

use assistant_crypto::vault_crypto::derive_entry_key;
use assistant_crypto::vault_crypto::types::Dek;

use crate::security_db::SecurityDbState;
use crate::types::{VaultCategory, VaultEntrySnapshot, VaultError};

use super::field_crypto::{
    decrypt_custom_fields, decrypt_opt, FIELD_NOTES, FIELD_PASSWORD, FIELD_TOTP,
};
use super::query_ids;

impl SecurityDbState {
    pub fn export_all_snapshots(&self) -> Result<Vec<VaultEntrySnapshot>, VaultError> {
        self.with_conn_and_dek(|conn, dek, dek_version| {
            let ids = query_ids(
                conn,
                "SELECT id FROM vault_entries WHERE deleted_at IS NULL ORDER BY created_at",
            )?;
            ids.iter()
                .map(|id| read_snapshot(conn, dek, dek_version, id))
                .collect::<Result<Vec<_>, VaultError>>()
        })
    }

    pub fn snapshot(&self, id: &str) -> Result<VaultEntrySnapshot, VaultError> {
        self.with_conn_and_dek(|conn, dek, dek_version| read_snapshot(conn, dek, dek_version, id))
    }
}

pub(super) fn read_snapshot(
    conn: &Connection,
    dek: &Dek,
    dek_version: u32,
    id: &str,
) -> Result<VaultEntrySnapshot, VaultError> {
    let row = conn
        .query_row(
            "SELECT id, title, username, url, category, encrypted_password, encrypted_notes, \
             encrypted_totp_secret, encrypted_custom_fields, tags, is_favorite, revision, \
             created_at, updated_at \
             FROM vault_entries WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, i64>(12)?,
                    row.get::<_, i64>(13)?,
                ))
            },
        )
        .optional()
        .map_err(|e| VaultError::Database(e.to_string()))?;

    let (
        id,
        title,
        username,
        url,
        category,
        enc_password,
        enc_notes,
        enc_totp,
        enc_custom_fields,
        tags_json,
        is_favorite,
        revision,
        created_at,
        updated_at,
    ) = row.ok_or_else(|| VaultError::NotFound(id.to_string()))?;

    let entry_key = derive_entry_key(dek, &id);

    let password = decrypt_opt(enc_password.as_deref(), &entry_key, FIELD_PASSWORD, &id)?;
    let notes = decrypt_opt(enc_notes.as_deref(), &entry_key, FIELD_NOTES, &id)?;
    let totp_secret = decrypt_opt(enc_totp.as_deref(), &entry_key, FIELD_TOTP, &id)?;
    let custom_fields = decrypt_custom_fields(enc_custom_fields.as_deref(), &entry_key, &id)?;

    Ok(VaultEntrySnapshot {
        id,
        category: VaultCategory::parse(&category).unwrap_or_default(),
        revision,
        dek_version,
        title,
        username,
        url,
        password,
        notes,
        totp_secret,
        custom_fields,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        is_favorite: is_favorite != 0,
        created_at,
        updated_at,
    })
}
