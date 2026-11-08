use assistant_agent::kernel::Clock;

#[derive(Default)]
pub struct SystemClock;

impl SystemClock {
    pub fn new() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> std::time::Instant {
        std::time::Instant::now()
    }

    fn now_millis(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}
