use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::Rng;
use sha2::{Digest, Sha256};

use crate::audit::{log_dual, AuditEvent};
use crate::types::VaultError;
use crate::vault_file::types::{KdfParams, VaultMetaFileV2, VaultSlot};
use crate::vault_file::VaultFileManager;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, DbKey, Dek};
use assistant_crypto::vault_crypto::{
    build_slot_aad, derive_entry_key, derive_kek, open_dek, seal_dek,
};

use super::entries::field_crypto::{
    decrypt_custom_fields, decrypt_opt, encrypt_custom_fields, encrypt_opt, FIELD_NOTES,
    FIELD_PASSWORD, FIELD_TOTP,
};
use super::{
    entries, meta, open_connection, schema, Anchor, SecurityDbState, BACKUP_PREFIX, BACKUP_SUFFIX,
    MAX_BACKUPS,
};

const PASSWORD_SLOT_TYPE: &str = "password";
const KEK_SALT_LEN: usize = 16;

impl SecurityDbState {
    pub fn change_master_password(
        &self,
        repo: &VaultFileManager,
        current: &str,
        new: &str,
    ) -> Result<VaultMetaFileV2, VaultError> {
        let strength = crate::password_strength::estimate_strength(new);
        if strength.score < 2 {
            return Err(VaultError::Validation(format!(
                "New master password too weak (score {}/4, need ≥2). {}",
                strength.score, strength.crack_time_display
            )));
        }

        let meta_v2 = repo.read_vault_meta_v2()?;
        let slot = meta_v2
            .slots
            .iter()
            .find(|slot| slot.slot_type == PASSWORD_SLOT_TYPE)
            .ok_or_else(|| VaultError::VaultCorrupted("password slot not found".to_string()))?
            .clone();

        let old_salt = decode_salt(&slot.kdf)?;
        let kek_old = derive_kek(current, &old_salt, slot.kdf.m, slot.kdf.t, slot.kdf.p)?;
        let wrapped_old = B64
            .decode(&slot.wrapped_dek)
            .map_err(|e| VaultError::VaultCorrupted(format!("wrappedDek decode failed: {}", e)))?;
        let aad_old = build_slot_aad(
            &slot.payload_kind,
            &slot.slot_type,
            &slot.id,
            meta_v2.dek_version,
        );
        let dek = open_dek(&kek_old, &wrapped_old, &aad_old)?;

        let new_salt: [u8; KEK_SALT_LEN] = rand::thread_rng().gen();
        let kek_new = derive_kek(new, &new_salt, slot.kdf.m, slot.kdf.t, slot.kdf.p)?;

        let next_slot_id = next_password_slot_id(&meta_v2.slots);
        let aad_new = build_slot_aad(
            &slot.payload_kind,
            &slot.slot_type,
            &next_slot_id,
            meta_v2.dek_version,
        );
        let wrapped_new = seal_dek(&kek_new, &dek, &aad_new)?;

        let new_slot = VaultSlot {
            id: next_slot_id,
            slot_type: slot.slot_type.clone(),
            payload_kind: slot.payload_kind.clone(),
            kdf: KdfParams {
                algo: slot.kdf.algo.clone(),
                salt: B64.encode(new_salt),
                m: slot.kdf.m,
                t: slot.kdf.t,
                p: slot.kdf.p,
                v: slot.kdf.v,
            },
            wrapped_dek: B64.encode(wrapped_new),
            created_at: crate::types::now_millis(),
        };

        let mut meta_new = meta_v2.clone();
        meta_new.slots = vec![new_slot];

        let db_path = self.db_path().to_string();
        let meta_path = repo.meta_path();
        let audit_writer = self.audit_writer();

        let (committed_meta, meta_hash) = self.with_session_mut(|session| {
            if dek.expose() != session.dek.expose() {
                return Err(VaultError::VaultCorrupted(
                    "password slot dek does not match session dek".to_string(),
                ));
            }

            repo.write_vault_meta_v2(&mut meta_new, &session.meta_key)?;
            let meta_hash = sha256_hex_file(&meta_path)?;

            let revision = meta::get_anchor(&session.conn)
                .map_err(VaultError::Database)?
                .map(|anchor| anchor.revision)
                .unwrap_or(0)
                .saturating_add(1);
            let anchor = Anchor {
                meta_hash: meta_hash.clone(),
                dek_version: meta_new.dek_version,
                revision,
            };
            meta::set_anchor(&session.conn, &anchor).map_err(VaultError::Database)?;

            let _ = log_dual(
                Some(&session.conn),
                Some(&audit_writer),
                AuditEvent::MasterPasswordChange,
                None,
            );

            Ok((meta_new.clone(), meta_hash))
        })?;

        match Self::purge_backups(Path::new(&db_path)) {
            Ok(removed) => log::info!("Purged {} pre-rotation vault backup(s)", removed),
            Err(err) => log::warn!("Failed to purge pre-rotation vault backups: {}", err),
        }

        log::info!(
            "Master password changed (O(1): dek repackage only, anchor {})",
            meta_hash
        );
        Ok(committed_meta)
    }

    pub fn rebuild_local_db(
        &self,
        repo: &VaultFileManager,
        dek: &Dek,
        db_key: &DbKey,
        anchor: &Anchor,
    ) -> Result<(), VaultError> {
        let db_path = PathBuf::from(self.db_path());

        if let Some(session) = self
            .session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            drop(session);
        }

        if db_path.exists() {
            Self::backup_db_file(&db_path)?;
            std::fs::remove_file(&db_path)?;
        }

        let conn = open_connection(&db_path, db_key)?;
        conn.execute_batch(schema::INIT_SQL)
            .map_err(|e| VaultError::Database(e.to_string()))?;
        meta::ensure_schema_version(&conn).map_err(VaultError::Database)?;
        let vault_id = repo.read_vault_meta_v2()?.vault_id;
        meta::set_vault_id(&conn, &vault_id).map_err(VaultError::Database)?;

        let algo = repo.cipher_algorithm();
        let entry_count = match entries::rebuild_index(&conn, repo, dek, algo) {
            Ok(count) => count,
            Err(err) => {
                drop(conn);
                let _ = std::fs::remove_file(&db_path);
                return Err(err);
            }
        };

        let rebuilt_anchor = Anchor {
            meta_hash: anchor.meta_hash.clone(),
            dek_version: anchor.dek_version,
            revision: 0,
        };
        if let Err(err) = meta::set_anchor(&conn, &rebuilt_anchor).map_err(VaultError::Database) {
            drop(conn);
            let _ = std::fs::remove_file(&db_path);
            return Err(err);
        }

        let audit_writer = self.audit_writer();
        let _ = log_dual(
            Some(&conn),
            Some(&audit_writer),
            AuditEvent::LocalDbRebuilt,
            Some(&entry_count.to_string()),
        );
        drop(conn);

        log::info!(
            "Local security db rebuilt from entry files ({} entries, anchor revision reset to 0)",
            entry_count
        );
        Ok(())
    }

    fn backup_db_file(db_path: &Path) -> Result<PathBuf, VaultError> {
        let backup_dir = Self::backup_dir_for(db_path)?;
        std::fs::create_dir_all(&backup_dir)?;

        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f");
        let mut seq = 0u32;
        let mut backup_path = backup_dir.join(format!(
            "{}{}-{:03}{}",
            BACKUP_PREFIX, timestamp, seq, BACKUP_SUFFIX
        ));
        while backup_path.exists() {
            seq += 1;
            backup_path = backup_dir.join(format!(
                "{}{}-{:03}{}",
                BACKUP_PREFIX, timestamp, seq, BACKUP_SUFFIX
            ));
        }

        std::fs::copy(db_path, &backup_path)?;
        Self::prune_backups(&backup_dir, MAX_BACKUPS)?;
        Ok(backup_path)
    }
}

fn decode_salt(kdf: &KdfParams) -> Result<Vec<u8>, VaultError> {
    B64.decode(&kdf.salt)
        .map_err(|e| VaultError::VaultCorrupted(format!("kdf salt decode failed: {}", e)))
}

fn next_password_slot_id(slots: &[VaultSlot]) -> String {
    let max = slots
        .iter()
        .filter_map(|slot| slot.id.strip_prefix("pwd-"))
        .filter_map(|suffix| suffix.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("pwd-{}", max + 1)
}

fn sha256_hex_file(path: &Path) -> Result<String, VaultError> {
    let bytes = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

type EntryCipherColumns = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// `rotate_dek` 第 4 步：把本机库里所有条目的加密列从 `dek_old` 换成 `dek_new`。
///
/// 包含墓碑行（`deleted_at IS NOT NULL`），否则重建索引会让已删条目复活。
/// 明文列与 `revision` 不变，只有密文与派生密钥变化。
pub(crate) fn reencrypt_entry_columns(
    tx: &rusqlite::Transaction<'_>,
    dek_old: &Dek,
    dek_new: &Dek,
    algo: CipherAlgorithm,
) -> Result<usize, VaultError> {
    let rows: Vec<EntryCipherColumns> = {
        let mut stmt = tx
            .prepare(
                "SELECT id, encrypted_password, encrypted_notes, encrypted_totp_secret, \
                 encrypted_custom_fields FROM vault_entries",
            )
            .map_err(|e| VaultError::Database(e.to_string()))?;
        let mapped = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|e| VaultError::Database(e.to_string()))?;
        let mut out = Vec::new();
        for row in mapped {
            out.push(row.map_err(|e| VaultError::Database(e.to_string()))?);
        }
        out
    };

    let mut count = 0usize;
    for (id, enc_password, enc_notes, enc_totp, enc_custom) in rows {
        let key_old = derive_entry_key(dek_old, &id);
        let key_new = derive_entry_key(dek_new, &id);

        let password = decrypt_opt(enc_password.as_deref(), &key_old, FIELD_PASSWORD, &id)?;
        let notes = decrypt_opt(enc_notes.as_deref(), &key_old, FIELD_NOTES, &id)?;
        let totp = decrypt_opt(enc_totp.as_deref(), &key_old, FIELD_TOTP, &id)?;
        let custom = decrypt_custom_fields(enc_custom.as_deref(), &key_old, &id)?;

        let new_password = encrypt_opt(password.as_deref(), &key_new, FIELD_PASSWORD, &id, algo)?;
        let new_notes = encrypt_opt(notes.as_deref(), &key_new, FIELD_NOTES, &id, algo)?;
        let new_totp = encrypt_opt(totp.as_deref(), &key_new, FIELD_TOTP, &id, algo)?;
        let new_custom = match enc_custom {
            Some(_) => Some(encrypt_custom_fields(&custom, &key_new, &id, algo)?),
            None => None,
        };

        tx.execute(
            "UPDATE vault_entries SET encrypted_password = ?1, encrypted_notes = ?2, \
             encrypted_totp_secret = ?3, encrypted_custom_fields = ?4 WHERE id = ?5",
            rusqlite::params![new_password, new_notes, new_totp, new_custom, &id],
        )
        .map_err(|e| VaultError::Database(e.to_string()))?;
        count += 1;
    }

    Ok(count)
}

/// `rotate_dek` 第 8 步：对本机 SQLCipher 库执行 `PRAGMA rekey` 切到新 `dbKey`。
///
/// 必须在该连接上没有打开事务时调用；成功后同一连接的后续读写继续可用。
pub(crate) fn rotate_local_db_key(
    conn: &rusqlite::Connection,
    db_key_new: &DbKey,
) -> Result<(), VaultError> {
    let key_hex = hex::encode(db_key_new.expose());
    conn.execute_batch(&format!("PRAGMA rekey = \"x'{}'\";", key_hex))
        .map_err(|e| VaultError::Database(format!("PRAGMA rekey failed: {}", e)))
}
