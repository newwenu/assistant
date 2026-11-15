use std::path::Path;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::Rng;

use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::{CipherAlgorithm, DbKey, Dek, Kek};
use assistant_crypto::vault_crypto::{
    build_slot_aad, derive_db_key, derive_kek, derive_meta_key, seal_dek,
};

use crate::biometric::BiometricKeyStore;
use crate::dto::{RecoveryBackupDto, RecoveryKeyDto};
use crate::recovery_key::{encode_paper, format_groups};
use crate::security_db::{
    open_connection, Anchor, AnchorExpectation, SecurityDbState, VaultSession,
};
use crate::vault_file::types::{
    KdfParams, RecoveryBackup, VaultManifest, VaultMetaFileV2, VaultSlot,
};
use crate::vault_file::VaultFileManager;

pub const DEFAULT_KDF_ALGO: &str = "argon2id";
pub const DEFAULT_KDF_M: u32 = 65536;
pub const DEFAULT_KDF_T: u32 = 3;
pub const DEFAULT_KDF_P: u32 = 4;
pub const DEFAULT_KDF_V: u32 = 19;
pub const DEFAULT_DEK_VERSION: u32 = 1;

const PASSWORD_SLOT_TYPE: &str = "password";
const DEK_PAYLOAD_KIND: &str = "dek";
const KEK_SALT_LEN: usize = 16;

pub fn initialize(
    state: &SecurityDbState,
    repo: &VaultFileManager,
    master_password: &str,
) -> Result<(), VaultError> {
    if state.is_initialized()? {
        return Err(VaultError::AlreadyExists);
    }

    repo.check_repository_layout()
        .map_err(|e| VaultError::Other(e.to_string()))?;

    // 已有 `.vault-meta` 时禁止重新初始化：v1 库在此报 UnsupportedVaultVersion，
    // v2 库报 AlreadyExists。否则初始化会静默覆盖旧仓库（v1 条目随之变成孤儿）。
    if repo.meta_path().exists() {
        repo.read_vault_meta_v2()?;
        return Err(VaultError::AlreadyExists);
    }

    let strength = crate::password_strength::estimate_strength(master_password);
    if strength.score < 2 {
        return Err(VaultError::Validation(format!(
            "Master password too weak (score {}/4, need ≥2). {}",
            strength.score, strength.crack_time_display
        )));
    }

    let vault_id = hex::encode(rand::thread_rng().gen::<[u8; 16]>());
    let salt: [u8; KEK_SALT_LEN] = rand::thread_rng().gen();
    let kek = derive_kek(
        master_password,
        &salt,
        DEFAULT_KDF_M,
        DEFAULT_KDF_T,
        DEFAULT_KDF_P,
    )?;
    let dek = Dek::new(rand::thread_rng().gen::<[u8; 20]>());
    let db_key = derive_db_key(&dek, &vault_id);

    let slot_id = "pwd-1".to_string();
    let aad = build_slot_aad(
        DEK_PAYLOAD_KIND,
        PASSWORD_SLOT_TYPE,
        &slot_id,
        DEFAULT_DEK_VERSION,
    );
    let wrapped_dek = seal_dek(&kek, &dek, &aad)?;

    let mut meta = VaultMetaFileV2 {
        version: 2,
        vault_id: vault_id.clone(),
        dek_version: DEFAULT_DEK_VERSION,
        cipher: cipher_wire_name(repo.cipher_algorithm()).to_string(),
        slots: vec![VaultSlot {
            id: slot_id,
            slot_type: PASSWORD_SLOT_TYPE.to_string(),
            payload_kind: DEK_PAYLOAD_KIND.to_string(),
            kdf: KdfParams {
                algo: DEFAULT_KDF_ALGO.to_string(),
                salt: B64.encode(salt),
                m: DEFAULT_KDF_M,
                t: DEFAULT_KDF_T,
                p: DEFAULT_KDF_P,
                v: DEFAULT_KDF_V,
            },
            wrapped_dek: B64.encode(wrapped_dek),
            created_at: crate::types::now_millis(),
        }],
        manifest: VaultManifest {
            count: 0,
            entries: Vec::new(),
        },
        recovery_backup: None,
        integrity: String::new(),
    };

    let meta_key = assistant_crypto::vault_crypto::derive_meta_key(&dek);
    repo.write_vault_meta_v2(&mut meta, &meta_key)?;
    let meta_hash = repo.meta_file_hash()?;

    let anchor = Anchor {
        meta_hash,
        dek_version: DEFAULT_DEK_VERSION,
        revision: 1,
    };
    state.init(dek, db_key, meta.dek_version, &vault_id, &anchor, Some(kek))
}

pub fn unlock(
    state: &SecurityDbState,
    repo: &VaultFileManager,
    master_password: &str,
) -> Result<(), VaultError> {
    let meta = repo.read_vault_meta_v2()?;
    let (kek, dek, db_key) = derive_session_keys(master_password, &meta)?;

    let db_path = Path::new(state.db_path());
    if !db_path.exists() {
        return Err(VaultError::NotInitialized);
    }
    let conn = open_connection(db_path, &db_key)?;

    let expected = AnchorExpectation::new(repo.meta_file_hash()?, meta.dek_version);
    state.unlock(
        VaultSession::new(conn, dek, db_key, meta.dek_version).with_kek(kek),
        &expected,
    )?;

    if state.vault_id()?.is_none() {
        state.set_vault_id(&meta.vault_id)?;
    }
    Ok(())
}

pub fn change_password(
    state: &SecurityDbState,
    repo: &VaultFileManager,
    current: &str,
    new: &str,
) -> Result<VaultMetaFileV2, VaultError> {
    state.change_master_password(repo, current, new)
}

pub fn sync_write_meta(repo: &VaultFileManager, dek: &Dek) -> Result<VaultMetaFileV2, VaultError> {
    repo.check_repository_layout()
        .map_err(|e| VaultError::Other(e.to_string()))?;
    let mut meta = repo.read_vault_meta_v2()?;
    meta.manifest = repo.compute_manifest(meta.dek_version)?;
    let meta_key = assistant_crypto::vault_crypto::derive_meta_key(dek);
    repo.write_vault_meta_v2(&mut meta, &meta_key)?;
    Ok(meta)
}

/// 本地写完 `.vault-meta` 后必须前移本机 anchor（key §4.6 `revision += 1`）。
///
/// 调用点必须在**事务回调之外**：事务回调持有 session 锁，`refresh_anchor`
/// 内部要重新取锁，`std::sync::Mutex` 不可重入会自锁死。
pub fn refresh_anchor(state: &SecurityDbState, repo: &VaultFileManager) -> Result<(), VaultError> {
    let meta = repo.read_vault_meta_v2()?;
    state.refresh_anchor(&repo.meta_file_hash()?, meta.dek_version)
}

pub fn rebuild_local_db(
    state: &SecurityDbState,
    repo: &VaultFileManager,
    master_password: &str,
) -> Result<(), VaultError> {
    let meta = repo.read_vault_meta_v2()?;
    let (kek, dek, db_key) = derive_session_keys(master_password, &meta)?;

    let anchor = Anchor {
        meta_hash: repo.meta_file_hash()?,
        dek_version: meta.dek_version,
        revision: 0,
    };
    state.rebuild_local_db(repo, &dek, &db_key, &anchor)?;

    let conn = open_connection(Path::new(state.db_path()), &db_key)?;
    let expected = AnchorExpectation::new(repo.meta_file_hash()?, meta.dek_version);
    state.unlock(
        VaultSession::new(conn, dek, db_key, meta.dek_version).with_kek(kek),
        &expected,
    )
}

pub fn rotate_dek(
    state: &SecurityDbState,
    repo: &VaultFileManager,
    key_store: &dyn BiometricKeyStore,
) -> Result<RecoveryKeyDto, VaultError> {
    // 前置守卫：轮换必须发生在口令会话中（生物识别/恢复码会话没有 KEK，
    // 无法重包 dek）。此检查不持有会话锁，避免与后续步骤的自锁。
    state.with_session(|session| {
        if session.kek().is_none() {
            return Err(VaultError::Validation(
                "轮换数据密钥需要口令会话（当前会话由生物识别或恢复码建立）".to_string(),
            ));
        }
        Ok(())
    })?;

    // 第 1 步：先吊销生物识别凭据。`vault-payload` 删除失败必须中止，
    // 否则旧 dek 仍可被生物识别解出，轮换等于没做（R8）。
    state.revoke_biometric_for_rotation(key_store)?;

    let meta = repo.read_vault_meta_v2()?;
    let dek_version_new = meta
        .dek_version
        .checked_add(1)
        .ok_or_else(|| VaultError::VaultCorrupted("dekVersion overflow".to_string()))?;
    let dek_new = Dek::generate();
    let db_key_new = derive_db_key(&dek_new, &meta.vault_id);
    let meta_key_new = derive_meta_key(&dek_new);
    let algo = repo.cipher_algorithm();
    let created_at = crate::types::now_millis();
    let paper_key = encode_paper(&dek_new);
    let groups = format_groups(&paper_key);

    state.with_session_mut(|session| {
        if session.dek_version != meta.dek_version {
            return Err(VaultError::VaultRollbackDetected);
        }
        let kek = session.kek().ok_or_else(|| {
            VaultError::Validation(
                "轮换数据密钥需要口令会话（当前会话由生物识别或恢复码建立）".to_string(),
            )
        })?;

        // 第 3 步：条目文件全量重加密（先写 `<id>.tmp`，全部成功才批量 rename）
        repo.reencrypt_entries(&session.dek, &dek_new, dek_version_new)
            .map_err(sync_error_to_vault_error)?;

        // 第 4 步：本机库加密列换钥（含墓碑行，事务内完成，失败整体回滚）
        let tx = session
            .conn
            .unchecked_transaction()
            .map_err(|e| VaultError::Database(e.to_string()))?;
        let reencrypted = crate::security_db::rekey::reencrypt_entry_columns(
            &tx,
            &session.dek,
            &dek_new,
            algo,
        )?;
        tx.commit()
            .map_err(|e| VaultError::Database(e.to_string()))?;

        // 第 5 步：用当前 KEK 重新包住 dek_new（AAD 带新 dekVersion）
        let slot = meta
            .slots
            .iter()
            .find(|slot| slot.slot_type == PASSWORD_SLOT_TYPE)
            .ok_or_else(|| VaultError::VaultCorrupted("password slot not found".to_string()))?;
        let aad_new = build_slot_aad(
            &slot.payload_kind,
            &slot.slot_type,
            &slot.id,
            dek_version_new,
        );
        let wrapped_new = seal_dek(kek, &dek_new, &aad_new)?;

        // 第 6 步：新 meta —— 新 slot、dekVersion+1、清空 recovery_backup、全量重算 manifest
        let mut meta_new = meta.clone();
        meta_new.dek_version = dek_version_new;
        meta_new.slots = vec![VaultSlot {
            id: slot.id.clone(),
            slot_type: slot.slot_type.clone(),
            payload_kind: slot.payload_kind.clone(),
            kdf: slot.kdf.clone(),
            wrapped_dek: B64.encode(wrapped_new),
            created_at,
        }];
        meta_new.manifest = repo.compute_manifest(dek_version_new)?;
        meta_new.recovery_backup = None;

        // 第 7 步：写 meta 即提交点，随后推进 anchor
        repo.write_vault_meta_v2(&mut meta_new, &meta_key_new)?;
        let meta_hash = repo.meta_file_hash()?;
        let revision = crate::security_db::meta::get_anchor(&session.conn)
            .map_err(VaultError::Database)?
            .map(|anchor| anchor.revision)
            .unwrap_or(0)
            .saturating_add(1);
        let anchor = Anchor {
            meta_hash,
            dek_version: dek_version_new,
            revision,
        };
        crate::security_db::meta::set_anchor(&session.conn, &anchor)
            .map_err(VaultError::Database)?;

        let writer = state.audit_writer();
        crate::audit::log_dual(
            Some(&session.conn),
            Some(&writer),
            crate::audit::AuditEvent::DekRotated,
            Some(&format!(
                "{}:{}:{}",
                meta.dek_version, dek_version_new, reencrypted
            )),
        );

        // 第 8 步：本机库 rekey。失败不致命 —— 本机库是可重建缓存，
        // 下次解锁会因 dbKey 不匹配走 `rebuild_local_db`。
        if let Err(err) =
            crate::security_db::rekey::rotate_local_db_key(&session.conn, &db_key_new)
        {
            log::warn!(
                "PRAGMA rekey failed after dek rotation, local db will be rebuilt on next unlock: {}",
                err
            );
        }

        session.dek = dek_new;
        session.db_key = db_key_new;
        session.meta_key = meta_key_new;
        session.dek_version = dek_version_new;

        Ok(())
    })?;

    // 第 9 步：返回新纸面恢复码；生物识别已吊销，由 UI 引导重新注册
    log::info!("DEK rotated to version {}", dek_version_new);
    Ok(RecoveryKeyDto {
        paper_key,
        groups,
        dek_version: dek_version_new,
        created_at,
    })
}

fn sync_error_to_vault_error(err: assistant_sync::types::SyncError) -> VaultError {
    use assistant_sync::types::SyncError;
    match err {
        SyncError::Io(msg) => VaultError::Io(std::io::Error::other(msg)),
        SyncError::EntryCorrupted(msg) => VaultError::EntryCorrupted(msg),
        SyncError::UnsupportedEntryFileVersion(msg) => VaultError::VaultCorrupted(msg),
        SyncError::VaultKeyMismatch(msg) => VaultError::Crypto(msg),
        other => VaultError::Other(other.to_string()),
    }
}

/// 重置仓库：先清空 keystore（任一失败即中止），再删除磁盘文件。
pub fn reset_repository(
    state: &SecurityDbState,
    key_store: &dyn BiometricKeyStore,
    targets: &[std::path::PathBuf],
) -> Result<(), VaultError> {
    state.disenroll_biometric(key_store)?;

    for path in targets {
        if path.is_dir() {
            std::fs::remove_dir_all(path)?;
        } else if path.exists() {
            std::fs::remove_file(path)?;
        }
    }

    state.audit_jsonl(crate::audit::AuditEvent::VaultRepositoryReset, None);

    log::info!("Vault repository reset completed");
    Ok(())
}

pub fn export_recovery_key(
    state: &SecurityDbState,
    repo: &VaultFileManager,
    master_password: &str,
) -> Result<RecoveryKeyDto, VaultError> {
    let mut meta = repo.read_vault_meta_v2()?;
    let (_kek, dek, _db_key) = derive_session_keys(master_password, &meta)?;
    let confirmed_at = crate::types::now_millis();

    state.with_session_mut(|session| {
        if !dek.ct_eq(&session.dek) {
            return Err(VaultError::InvalidPassword);
        }

        let paper_key = encode_paper(&session.dek);
        let groups = format_groups(&paper_key);
        let dek_version = meta.dek_version;

        meta.recovery_backup = Some(RecoveryBackup {
            dek_version,
            confirmed_at,
        });
        repo.write_vault_meta_v2(&mut meta, session.meta_key())?;

        let meta_hash = repo.meta_file_hash()?;
        let revision = crate::security_db::meta::get_anchor(&session.conn)
            .map_err(VaultError::Database)?
            .map(|anchor| anchor.revision)
            .unwrap_or(0)
            .saturating_add(1);
        let anchor = Anchor {
            meta_hash,
            dek_version,
            revision,
        };
        crate::security_db::meta::set_anchor(&session.conn, &anchor)
            .map_err(VaultError::Database)?;

        let writer = state.audit_writer();
        crate::audit::log_dual(
            Some(&session.conn),
            Some(&writer),
            crate::audit::AuditEvent::RecoveryKeyExported,
            Some(&dek_version.to_string()),
        );

        Ok(RecoveryKeyDto {
            paper_key,
            groups,
            dek_version,
            created_at: confirmed_at,
        })
    })
}

pub fn recovery_backup(repo: &VaultFileManager) -> Result<Option<RecoveryBackupDto>, VaultError> {
    let meta = match repo.read_vault_meta_v2() {
        Ok(meta) => meta,
        Err(VaultError::NotInitialized) => return Ok(None),
        Err(err) => return Err(err),
    };
    Ok(meta
        .recovery_backup
        .as_ref()
        .filter(|backup| backup.dek_version == meta.dek_version)
        .map(|backup| RecoveryBackupDto {
            dek_version: backup.dek_version,
            confirmed_at: backup.confirmed_at,
        }))
}

pub fn cipher_wire_name(algo: CipherAlgorithm) -> &'static str {
    match algo {
        CipherAlgorithm::Aes256Gcm => "aes256Gcm",
        CipherAlgorithm::ChaCha20Poly1305 => "chacha20Poly1305",
    }
}

pub fn cipher_from_wire_name(name: &str) -> Result<CipherAlgorithm, VaultError> {
    match name {
        "aes256Gcm" => Ok(CipherAlgorithm::Aes256Gcm),
        "chacha20Poly1305" => Ok(CipherAlgorithm::ChaCha20Poly1305),
        other => Err(VaultError::VaultCorrupted(format!(
            "unsupported cipher: {}",
            other
        ))),
    }
}

pub fn derive_session_keys(
    master_password: &str,
    meta: &VaultMetaFileV2,
) -> Result<(Kek, Dek, DbKey), VaultError> {
    let slot = meta
        .slots
        .iter()
        .find(|slot| slot.slot_type == PASSWORD_SLOT_TYPE)
        .ok_or_else(|| VaultError::VaultCorrupted("password slot not found".to_string()))?;
    let salt = B64
        .decode(&slot.kdf.salt)
        .map_err(|e| VaultError::VaultCorrupted(format!("kdf salt decode failed: {}", e)))?;
    let kek = derive_kek(master_password, &salt, slot.kdf.m, slot.kdf.t, slot.kdf.p)?;
    let wrapped = B64
        .decode(&slot.wrapped_dek)
        .map_err(|e| VaultError::VaultCorrupted(format!("wrappedDek decode failed: {}", e)))?;
    let aad = build_slot_aad(
        &slot.payload_kind,
        &slot.slot_type,
        &slot.id,
        meta.dek_version,
    );
    let dek = assistant_crypto::vault_crypto::open_dek(&kek, &wrapped, &aad)?;
    let db_key = derive_db_key(&dek, &meta.vault_id);
    Ok((kek, dek, db_key))
}
