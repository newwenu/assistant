use assistant_sync::types::{Readyness, SyncError};

use super::SyncEngine;

impl SyncEngine {
    pub fn readyness(&self) -> Result<Readyness, SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .readyness()
    }

    pub fn prepare(&self) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prepare()
    }

    pub fn finalize(&self, msg: &str) -> Result<(), SyncError> {
        self.git_sync
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .finalize(msg)
    }
}
