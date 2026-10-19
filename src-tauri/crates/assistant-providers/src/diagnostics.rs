use std::sync::atomic::{AtomicU8, Ordering};

static DIAGNOSTIC_LEVEL: AtomicU8 = AtomicU8::new(DiagnosticLevel::Normal as u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DiagnosticLevel {
    Normal = 0,
    Verbose = 1,
    Full = 2,
}

impl DiagnosticLevel {
    pub fn current() -> Self {
        match DIAGNOSTIC_LEVEL.load(Ordering::Relaxed) {
            0 => DiagnosticLevel::Normal,
            1 => DiagnosticLevel::Verbose,
            _ => DiagnosticLevel::Full,
        }
    }

    pub fn set(self) {
        DIAGNOSTIC_LEVEL.store(self as u8, Ordering::Relaxed);
    }

    pub fn should_log_request_body(self) -> bool {
        matches!(self, DiagnosticLevel::Verbose | DiagnosticLevel::Full)
    }

    pub fn should_log_response_body(self) -> bool {
        matches!(self, DiagnosticLevel::Full)
    }

    pub fn should_log_sse_events(self) -> bool {
        matches!(self, DiagnosticLevel::Verbose | DiagnosticLevel::Full)
    }
}
