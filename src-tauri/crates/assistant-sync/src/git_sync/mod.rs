mod commit;
mod conflict;
mod helpers;
mod lifecycle;
mod remote;
mod state;

pub mod git;
pub mod ssh;

use std::path::PathBuf;
use std::time::Duration;

use crate::types::SyncConfig;

pub const STORE_GIT_DIR: &str = ".git/";
pub const GIT_PULL_OUTDATED: Duration = Duration::from_secs(30);

pub struct GitSync {
    repo_path: PathBuf,
    #[allow(dead_code)]
    config: SyncConfig,
}
