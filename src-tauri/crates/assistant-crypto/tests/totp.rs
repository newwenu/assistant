use assistant_crypto::vault_crypto::totp::{
    decode_base32, generate_at, parse_otpauth, time_remaining_at, TotpAlgorithm, TotpSpec,
};

const SECRET: &str = "JBSWY3DPEHPK3PXP";
const NOW: u64 = 1_700_000_000;

fn generate(secret: &str, spec: TotpSpec) -> assistant_crypto::vault_crypto::totp::TotpCode {
    let bytes = decode_base32(secret).unwrap();
    generate_at(&bytes, &spec, NOW).unwrap()
}

#[test]
fn totp_code_is_correct_length_and_numeric() {
    let result = generate(SECRET, TotpSpec::default());
    assert_eq!(result.code.len(), 6);
    assert!(result.code.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn totp_8_digit_code() {
    let result = generate(SECRET, TotpSpec::new(8, 30, TotpAlgorithm::Sha1));
    assert_eq!(result.code.len(), 8);
    assert!(result.code.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn totp_sha256_algorithm() {
    let result = generate(SECRET, TotpSpec::new(6, 30, TotpAlgorithm::Sha256));
    assert_eq!(result.code.len(), 6);
    assert!(result.code.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn totp_sha512_algorithm() {
    let result = generate(SECRET, TotpSpec::new(6, 30, TotpAlgorithm::Sha512));
    assert_eq!(result.code.len(), 6);
    assert!(result.code.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn totp_custom_period() {
    let result = generate(SECRET, TotpSpec::new(6, 60, TotpAlgorithm::Sha1));
    assert!(result.time_remaining > 0 && result.time_remaining <= 60);
}

#[test]
fn totp_rejects_invalid_base32() {
    assert!(decode_base32("!!!INVALID!!!").is_err());
}

#[test]
fn totp_rejects_empty_secret() {
    assert!(decode_base32("").is_err());
}

#[test]
fn time_remaining_within_bounds() {
    let remaining = time_remaining_at(NOW, 30);
    assert!(remaining > 0 && remaining <= 30);

    let remaining_60 = time_remaining_at(NOW, 60);
    assert!(remaining_60 > 0 && remaining_60 <= 60);
}

#[test]
fn totp_spec_clamps_invalid_values() {
    let spec = TotpSpec::new(0, 0, TotpAlgorithm::Sha1);
    assert_eq!(spec.digits, 1);
    assert_eq!(spec.period, 1);

    let spec = TotpSpec::new(20, 30, TotpAlgorithm::Sha1);
    assert_eq!(spec.digits, 10);
}

#[test]
fn algorithm_as_str_and_parse_round_trip() {
    for algorithm in [
        TotpAlgorithm::Sha1,
        TotpAlgorithm::Sha256,
        TotpAlgorithm::Sha512,
    ] {
        let parsed: TotpAlgorithm = algorithm.as_str().parse().unwrap();
        assert_eq!(parsed, algorithm);
    }
    assert!("sha-256".parse::<TotpAlgorithm>().is_ok());
    assert!("md5".parse::<TotpAlgorithm>().is_err());
}

#[test]
fn otpauth_uri_parse_full() {
    let uri = "otpauth://totp/GitHub:user@example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub&algorithm=SHA256&digits=8&period=60";
    let parsed = parse_otpauth(uri).unwrap();
    assert_eq!(parsed.secret, SECRET);
    assert_eq!(parsed.issuer.as_deref(), Some("GitHub"));
    assert_eq!(parsed.account.as_deref(), Some("user@example.com"));
    assert_eq!(parsed.algorithm, TotpAlgorithm::Sha256);
    assert_eq!(parsed.digits, 8);
    assert_eq!(parsed.period, 60);
}

#[test]
fn otpauth_uri_parse_minimal() {
    let uri = "otpauth://totp/user@example.com?secret=JBSWY3DPEHPK3PXP";
    let parsed = parse_otpauth(uri).unwrap();
    assert_eq!(parsed.secret, SECRET);
    assert_eq!(parsed.digits, 6);
    assert_eq!(parsed.period, 30);
    assert_eq!(parsed.algorithm, TotpAlgorithm::Sha1);
}

#[test]
fn otpauth_uri_rejects_missing_secret() {
    let uri = "otpauth://totp/user@example.com?issuer=GitHub";
    assert!(parse_otpauth(uri).is_err());
}

#[test]
fn otpauth_uri_rejects_wrong_scheme() {
    let uri = "https://example.com/totp?secret=JBSWY3DPEHPK3PXP";
    assert!(parse_otpauth(uri).is_err());
}

#[test]
fn otpauth_uri_rejects_hotp() {
    let uri = "otpauth://hotp/user@example.com?secret=JBSWY3DPEHPK3PXP";
    assert!(parse_otpauth(uri).is_err());
}

#[test]
fn otpauth_uri_algorithm_variants() {
    for (algo_str, expected) in [
        ("SHA1", TotpAlgorithm::Sha1),
        ("SHA256", TotpAlgorithm::Sha256),
        ("SHA-256", TotpAlgorithm::Sha256),
        ("SHA512", TotpAlgorithm::Sha512),
        ("SHA-512", TotpAlgorithm::Sha512),
    ] {
        let uri = format!(
            "otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&algorithm={}",
            algo_str
        );
        let parsed = parse_otpauth(&uri).unwrap();
        assert_eq!(parsed.algorithm, expected);
    }
}

#[test]
fn decode_base32_accepts_padded_and_unpadded_secrets() {
    assert_eq!(decode_base32("JBSWY3DP").unwrap(), b"Hello");
    assert_eq!(
        decode_base32("JBSWY3DPEHPK3PXP").unwrap(),
        b"Hello!\xde\xad\xbe\xef"
    );
    assert_eq!(decode_base32("ME======").unwrap(), b"a");
    assert_eq!(decode_base32("ME").unwrap(), b"a");
    assert!(decode_base32("   ").is_err());
}
