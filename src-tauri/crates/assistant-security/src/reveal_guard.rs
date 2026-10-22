use std::sync::Mutex;
use std::time::Instant;

const RATE_LIMIT_PER_MINUTE: usize = 3;
const RATE_WINDOW_SECS: u64 = 60;

pub struct RevealGuard {
    timestamps: Mutex<Vec<Instant>>,
}

impl RevealGuard {
    pub fn new() -> Self {
        Self {
            timestamps: Mutex::new(Vec::with_capacity(RATE_LIMIT_PER_MINUTE)),
        }
    }

    pub fn check_and_record(&self) -> Result<(), crate::types::VaultError> {
        let mut timestamps = self.timestamps.lock().unwrap();
        let now = Instant::now();

        timestamps.retain(|t| now.duration_since(*t).as_secs() < RATE_WINDOW_SECS);

        if timestamps.len() >= RATE_LIMIT_PER_MINUTE {
            return Err(crate::types::VaultError::RateLimited);
        }

        timestamps.push(now);
        Ok(())
    }
}
