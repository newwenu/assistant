use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use assistant_sync::git_sync::GitSync;
use assistant_sync::types::{PendingRemovals, SyncConfig};

use crate::vault_file::{CipherProvider, VaultFileManager};

mod export;
mod history;
mod lifecycle;
mod preview;
mod pull_push;
mod remote_admin;

pub use preview::mask_entry_preview;

/// 冲突揭示审计 `detail` 的字段封闭集合（§2.4）：自定义字段名一律收敛为 `customFields`。
pub const CONFLICT_FIELD_LABELS: [&str; 4] = ["password", "notes", "totp", "customFields"];

/// 拼装 `conflict:{entryId}:{side}:{field}:{pathDigest}`（§2.4 固定 5 段）。
///
/// `resolved` 表示调用点是否已完成字段解析：揭示成功后的非标准字段名必然是真实的自定义字段，
/// 可安全收敛为 `customFields`；限流分支在任何解析之前触发，无法区分「自定义字段」与畸形入参，
/// 一律写 `unknown`，避免把未校验的前端串写进 `detail`。
pub fn conflict_audit_detail(path: &str, side: &str, field: &str, resolved: bool) -> String {
    let entry_id = preview::parse_entry_id(path)
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        .unwrap_or_else(|| "unknown".to_string());
    let side = match side {
        "local" | "remote" => side,
        _ => "unknown",
    };
    let field = match field {
        "password" => "password",
        "notes" => "notes",
        "totp" | "totpSecret" => "totp",
        _ if resolved => "customFields",
        _ => "unknown",
    };
    format!(
        "conflict:{}:{}:{}:{}",
        entry_id,
        side,
        field,
        crate::audit::path_digest(path)
    )
}

pub struct SyncEngine {
    git_sync: Mutex<GitSync>,
    vault_file: Arc<VaultFileManager>,
    config: Mutex<SyncConfig>,
    pending_removals: Mutex<Option<PendingRemovals>>,
}

impl SyncEngine {
    /// 过渡构造入口：自建仓库实例，仅供既有测试使用。应用层应改用 `new_with_repo`。
    pub fn new(vault_dir: PathBuf, config: SyncConfig, cipher: CipherProvider) -> Self {
        Self::new_with_repo(Arc::new(VaultFileManager::new(vault_dir, cipher)), config)
    }

    /// 复用应用层 `VaultRepo` 持有的同一实例：git 目录直接取自仓库句柄，保证两者指向同一路径。
    pub fn new_with_repo(repo: Arc<VaultFileManager>, config: SyncConfig) -> Self {
        let git_sync = GitSync::new(repo.vault_dir().to_path_buf(), config.clone());
        Self {
            git_sync: Mutex::new(git_sync),
            vault_file: repo,
            config: Mutex::new(config),
            pending_removals: Mutex::new(None),
        }
    }

    pub fn vault_file_manager(&self) -> &VaultFileManager {
        &self.vault_file
    }
}
