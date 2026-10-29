use assistant_crypto::error::VaultError;
use assistant_security::reveal_guard::RevealGuard;

#[test]
fn reveal_guard_allows_up_to_rate_limit() {
    let guard = RevealGuard::new();

    assert!(guard.check_and_record().is_ok());
    assert!(guard.check_and_record().is_ok());
    assert!(guard.check_and_record().is_ok());
}

#[test]
fn reveal_guard_blocks_after_rate_limit() {
    let guard = RevealGuard::new();

    guard.check_and_record().unwrap();
    guard.check_and_record().unwrap();
    guard.check_and_record().unwrap();

    let result = guard.check_and_record();
    assert!(
        matches!(result, Err(VaultError::RateLimited)),
        "4th reveal within window should be rate limited"
    );
}

#[test]
fn reveal_guard_rate_limit_is_per_instance() {
    let guard1 = RevealGuard::new();
    let guard2 = RevealGuard::new();

    guard1.check_and_record().unwrap();
    guard1.check_and_record().unwrap();
    guard1.check_and_record().unwrap();

    assert!(guard2.check_and_record().is_ok());
}
