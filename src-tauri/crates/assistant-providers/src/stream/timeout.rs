use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamTimeoutStage {
    FirstToken,
    Idle,
}

#[derive(Debug)]
pub struct StreamTimeoutController {
    ttft_timeout: Duration,
    idle_timeout: Duration,
    first_token_received: bool,
    stream_start: Instant,
    last_chunk_time: Instant,
}

impl StreamTimeoutController {
    pub fn new(ttft_timeout: Duration, idle_timeout: Duration) -> Self {
        let now = Instant::now();
        Self {
            ttft_timeout,
            idle_timeout,
            first_token_received: false,
            stream_start: now,
            last_chunk_time: now,
        }
    }

    pub fn observe_content(&mut self, has_content: bool) {
        if has_content && !self.first_token_received {
            self.first_token_received = true;
        }
        self.last_chunk_time = Instant::now();
    }

    pub fn touch(&mut self) {
        self.last_chunk_time = Instant::now();
    }

    pub fn mark_first_token(&mut self) {
        if !self.first_token_received {
            self.first_token_received = true;
        }
    }

    pub fn check(&self) -> Option<StreamTimeoutStage> {
        let now = Instant::now();
        if !self.first_token_received {
            if now.duration_since(self.stream_start) > self.ttft_timeout {
                return Some(StreamTimeoutStage::FirstToken);
            }
        } else if now.duration_since(self.last_chunk_time) > self.idle_timeout {
            return Some(StreamTimeoutStage::Idle);
        }
        None
    }

    pub fn first_token_received(&self) -> bool {
        self.first_token_received
    }

    pub fn ttft_timeout(&self) -> Duration {
        self.ttft_timeout
    }

    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }
}

#[cfg(all(test, feature = "internal-tests"))]
mod tests {
    use super::*;

    #[test]
    fn no_timeout_before_limits() {
        let ctrl = StreamTimeoutController::new(Duration::from_secs(30), Duration::from_secs(30));
        assert!(ctrl.check().is_none());
    }

    #[test]
    fn first_token_timeout_stage() {
        let ctrl = StreamTimeoutController::new(Duration::from_millis(1), Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(ctrl.check(), Some(StreamTimeoutStage::FirstToken));
    }

    #[test]
    fn idle_timeout_after_first_token() {
        let mut ctrl =
            StreamTimeoutController::new(Duration::from_secs(30), Duration::from_millis(1));
        ctrl.observe_content(true);
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(ctrl.check(), Some(StreamTimeoutStage::Idle));
    }

    #[test]
    fn no_idle_timeout_before_first_token() {
        let ctrl = StreamTimeoutController::new(Duration::from_millis(1), Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(ctrl.check(), Some(StreamTimeoutStage::FirstToken));
    }

    #[test]
    fn observe_content_first_token_flag() {
        let mut ctrl =
            StreamTimeoutController::new(Duration::from_secs(30), Duration::from_secs(30));
        assert!(!ctrl.first_token_received());
        ctrl.observe_content(false);
        assert!(!ctrl.first_token_received());
        ctrl.observe_content(true);
        assert!(ctrl.first_token_received());
    }

    #[test]
    fn touch_updates_timestamp_without_first_token() {
        let mut ctrl =
            StreamTimeoutController::new(Duration::from_secs(30), Duration::from_millis(1));
        ctrl.observe_content(true);
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(ctrl.check(), Some(StreamTimeoutStage::Idle));
        ctrl.touch();
        assert!(ctrl.check().is_none());
        assert!(ctrl.first_token_received());
    }

    #[test]
    fn mark_first_token_sets_flag_without_timestamp() {
        let mut ctrl =
            StreamTimeoutController::new(Duration::from_millis(1), Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(ctrl.check(), Some(StreamTimeoutStage::FirstToken));
        ctrl.mark_first_token();
        assert!(ctrl.first_token_received());
        assert!(ctrl.check().is_none());
    }
}
