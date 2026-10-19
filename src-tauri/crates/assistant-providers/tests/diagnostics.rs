use assistant_providers::diagnostics::DiagnosticLevel;

#[test]
fn level_ordering() {
    assert!((DiagnosticLevel::Normal as u8) < DiagnosticLevel::Verbose as u8);
    assert!((DiagnosticLevel::Verbose as u8) < DiagnosticLevel::Full as u8);
}

#[test]
fn should_log_request_body() {
    assert!(!DiagnosticLevel::Normal.should_log_request_body());
    assert!(DiagnosticLevel::Verbose.should_log_request_body());
    assert!(DiagnosticLevel::Full.should_log_request_body());
}

#[test]
fn should_log_response_body() {
    assert!(!DiagnosticLevel::Normal.should_log_response_body());
    assert!(!DiagnosticLevel::Verbose.should_log_response_body());
    assert!(DiagnosticLevel::Full.should_log_response_body());
}

#[test]
fn should_log_sse_events() {
    assert!(!DiagnosticLevel::Normal.should_log_sse_events());
    assert!(DiagnosticLevel::Verbose.should_log_sse_events());
    assert!(DiagnosticLevel::Full.should_log_sse_events());
}

#[test]
fn set_and_get_current() {
    let original = DiagnosticLevel::current();
    DiagnosticLevel::Verbose.set();
    assert_eq!(DiagnosticLevel::current(), DiagnosticLevel::Verbose);
    original.set();
}
