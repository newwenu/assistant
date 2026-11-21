use assistant_agent::kernel::Clock as TimeSource;

#[derive(Default)]
pub struct SystemTimeSource;

impl SystemTimeSource {
    pub fn new() -> Self {
        Self
    }
}

impl TimeSource for SystemTimeSource {
    fn now(&self) -> std::time::Instant {
        std::time::Instant::now()
    }

    fn now_millis(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}
