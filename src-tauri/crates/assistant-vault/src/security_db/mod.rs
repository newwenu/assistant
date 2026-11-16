mod anchor;
mod session;
mod state;

pub mod biometric_ops;
pub mod crypto_ops;
pub mod entries;
pub mod import_export;
pub mod meta;
pub mod rekey;
pub mod schema;

pub use anchor::AnchorExpectation;
pub use meta::Anchor;
pub use session::{LockReason, SessionConnGuard, SessionDekGuard, VaultSession};
pub use state::SecurityDbState;

pub(crate) use schema::{open_connection, probe_read};

pub const SQLCIPHER_PRAGMAS: &str =
    "PRAGMA cipher_page_size = 4096; PRAGMA kdf_iter=1; PRAGMA cipher_hmac_algorithm = HMAC_SHA512; PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;";

/// 内存加固：密钥材料用后即清、临时表不落盘。必须在 `PRAGMA key` 之前生效。
pub const SQLCIPHER_HARDENING_PRAGMAS: &str =
    "PRAGMA cipher_memory_security = ON; PRAGMA temp_store = MEMORY;";

const MAX_BACKUPS: usize = 5;

/// JSONL 补录游标：`vault_meta` 的 KV 行，不改 DDL。
const JSONL_SYNC_KEY: &str = "audit_jsonl_synced_at";

/// 只补录「库不可用期间发生」的事件；其余事件解锁态本来就能直接写库。
///
/// 必须覆盖全部 J 通道事件（含锁库批量移除的 `ssh_key_removed`）；双写事件留在白名单内
/// 无害（去重会挡掉），但白名单缺少任一 J 事件即导致该事件永久不入库。
pub const BACKFILL_EVENTS: &[&str] = &[
    "recovery_export_used",
    "dek_rotated",
    "local_db_rebuilt",
    "vault_repository_reset",
    "vault_unlock_failed",
    "vault_unsupported_version",
    "biometric_failed",
    "ssh_key_removed",
];

/// J 通道事件全集（规格 §2.5 第 2 条的唯一事实来源）。
///
/// 断言 `J_CHANNEL_EVENTS ⊆ BACKFILL_EVENTS`：新增 J 事件却漏改白名单必须让测试失败。
pub const J_CHANNEL_EVENTS: &[&str] = &[
    "vault_unlock_failed",
    "recovery_export_used",
    "vault_repository_reset",
    "vault_unsupported_version",
    "biometric_failed",
    "ssh_key_removed",
];

const BACKUP_PREFIX: &str = "security-";

const BACKUP_SUFFIX: &str = ".db.bak";
