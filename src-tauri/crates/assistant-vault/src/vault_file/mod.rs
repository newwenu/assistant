pub mod types;

mod entry;
mod meta;

use std::fs;
use std::path::{Path, PathBuf};

use assistant_crypto::vault_crypto::types::CipherAlgorithm;
use assistant_sync::types::SyncError;

pub use crate::dto::integrity::IntegrityResult;
pub use entry::{decode_entry_content, peek_entry_dek_version};
pub use meta::canonical_meta_bytes;

const ENTRIES_DIR: &str = "entries";
const VAULT_META_FILE: &str = ".vault-meta";
pub const VAULT_META_FILE_NAME: &str = VAULT_META_FILE;
const GITIGNORE_FILE: &str = ".gitignore";
const GIT_DIR: &str = ".git";
const TEMP_SUFFIX: &str = ".tmp";

const MINIMAL_GITIGNORE: &str = "\
security.db
security.db-wal
security.db-shm
backups/
.audit/
vault-config.json
recovery/
*.tmp
";

pub type CipherProvider = Box<dyn Fn() -> CipherAlgorithm + Send + Sync>;

pub struct VaultFileManager {
    vault_dir: PathBuf,
    cipher: CipherProvider,
}

impl VaultFileManager {
    pub fn new(vault_dir: PathBuf, cipher: CipherProvider) -> Self {
        Self { vault_dir, cipher }
    }

    pub(crate) fn cipher_algorithm(&self) -> CipherAlgorithm {
        (self.cipher)()
    }

    pub fn vault_dir(&self) -> &Path {
        &self.vault_dir
    }

    pub fn entries_dir(&self) -> PathBuf {
        self.vault_dir.join(ENTRIES_DIR)
    }

    pub fn ensure_structure(&self) -> Result<(), SyncError> {
        fs::create_dir_all(self.entries_dir())?;
        Ok(())
    }

    pub fn ensure_gitignore(&self) -> Result<(), SyncError> {
        fs::create_dir_all(&self.vault_dir)?;
        let path = self.vault_dir.join(GITIGNORE_FILE);
        if !path.exists() {
            fs::write(&path, MINIMAL_GITIGNORE)?;
        }
        Ok(())
    }

    pub fn clean_temp_files(&self) -> Result<u32, SyncError> {
        let mut removed = 0u32;
        for dir in [self.vault_dir.clone(), self.entries_dir()] {
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(TEMP_SUFFIX) {
                    fs::remove_file(entry.path())?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    pub fn check_repository_layout(&self) -> Result<(), SyncError> {
        if !self.vault_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.vault_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            if name == VAULT_META_FILE || name == GITIGNORE_FILE || name == GIT_DIR {
                continue;
            }

            if name == ENTRIES_DIR {
                if !entry.file_type()?.is_dir() {
                    return Err(SyncError::RepositoryContainsForbiddenFile(name));
                }
                for inner in fs::read_dir(entry.path())? {
                    let inner = inner?;
                    let inner_name = inner.file_name().to_string_lossy().to_string();
                    if !inner.file_type()?.is_file() || !inner_name.ends_with(".enc") {
                        return Err(SyncError::RepositoryContainsForbiddenFile(format!(
                            "{}/{}",
                            ENTRIES_DIR, inner_name
                        )));
                    }
                }
                continue;
            }

            return Err(SyncError::RepositoryContainsForbiddenFile(name));
        }

        Ok(())
    }

    pub fn entry_file_path(&self, entry_id: &str) -> PathBuf {
        self.entries_dir().join(format!("{}.enc", entry_id))
    }
}

fn write_atomically(path: &Path, contents: &[u8]) -> Result<(), SyncError> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
