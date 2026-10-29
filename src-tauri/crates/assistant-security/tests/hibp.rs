use assistant_security::vault_crypto::hibp::{parse_hibp_response, sha1_hex, SafePassword};

#[test]
fn parse_hibp_response_finds_match() {
    let body = "003D68EB55012C21:5\n1E4C9B9B9B9B9B9B:2\n";
    let count = parse_hibp_response(body, "003D68EB55012C21");
    assert_eq!(count, 5);
}

#[test]
fn parse_hibp_response_no_match() {
    let body = "003D68EB55012C21:5\n1E4C9B9B9B9B9B9B:2\n";
    let count = parse_hibp_response(body, "AAAAAAAAAAAAAAAA");
    assert_eq!(count, 0);
}

#[test]
fn parse_hibp_response_empty_body() {
    let count = parse_hibp_response("", "003D68EB55012C21");
    assert_eq!(count, 0);
}

#[test]
fn parse_hibp_response_case_insensitive() {
    let body = "003d68eb55012c21:10\n";
    let count = parse_hibp_response(body, "003D68EB55012C21");
    assert_eq!(count, 10);
}

#[test]
fn parse_hibp_response_trims_whitespace() {
    let body = "  003D68EB55012C21:3  \n  \n";
    let count = parse_hibp_response(body, "003D68EB55012C21");
    assert_eq!(count, 3);
}

#[test]
fn sha1_hex_is_deterministic_uppercase_and_matches_known_value() {
    assert_eq!(sha1_hex("password"), sha1_hex("password"));
    assert_ne!(sha1_hex("password"), sha1_hex("Password"));
    assert_eq!(&sha1_hex("password")[..5], "5BAA6");
    assert_eq!(sha1_hex("test").len(), 40);
}

#[test]
fn safe_password_exposes_plain_value_and_drops_without_panic() {
    let safe = SafePassword::new("sensitive_password".to_string());
    assert_eq!(safe.as_str(), "sensitive_password");
    drop(safe);
}
