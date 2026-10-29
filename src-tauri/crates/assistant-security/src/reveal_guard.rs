use std::sync::Mutex;
use std::time::Instant;

use log::warn;

const RATE_LIMIT_PER_MINUTE: usize = 3;
const RATE_WINDOW_SECS: u64 = 60;

pub struct RevealGuard {
    timestamps: Mutex<Vec<Instant>>,
}

impl Default for RevealGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl RevealGuard {
    pub fn new() -> Self {
        Self {
            timestamps: Mutex::new(Vec::with_capacity(RATE_LIMIT_PER_MINUTE)),
        }
    }

    pub fn check_and_record(&self) -> Result<(), assistant_crypto::error::VaultError> {
        let mut timestamps = self.timestamps.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();

        timestamps.retain(|t| now.duration_since(*t).as_secs() < RATE_WINDOW_SECS);

        if timestamps.len() >= RATE_LIMIT_PER_MINUTE {
            warn!(
                "Vault reveal rate limited ({}/{})",
                timestamps.len(),
                RATE_LIMIT_PER_MINUTE
            );
            return Err(assistant_crypto::error::VaultError::RateLimited);
        }

        timestamps.push(now);
        Ok(())
    }
}
