use assistant_crypto::vault_crypto::totp::{
    decode_base32, from_otpauth_uri, generate_totp, time_remaining, TotpAlgorithm, TotpParams,
};

#[test]
fn totp_code_is_correct_length_and_numeric() {
    let params = TotpParams::default();
    let result = generate_totp("JBSWY3DPEHPK3PXP", &params).unwrap();
    assert_eq!(result.code.len(), 6);
    assert!(result.code.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn totp_8_digit_code() {
    let params = TotpParams {
        digits: 8,
        period: 30,
        algorithm: TotpAlgorithm::Sha1,
    };
    let result = generate_totp("JBSWY3DPEHPK3PXP", &params).unwrap();
    assert_eq!(result.code.len(), 8);
    assert!(result.code.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn totp_sha256_algorithm() {
    let params = TotpParams {
        digits: 6,
        period: 30,
        algorithm: TotpAlgorithm::Sha256,
    };
    let result = generate_totp("JBSWY3DPEHPK3PXP", &params).unwrap();
    assert_eq!(result.code.len(), 6);
}

#[test]
fn totp_sha512_algorithm() {
    let params = TotpParams {
        digits: 6,
        period: 30,
        algorithm: TotpAlgorithm::Sha512,
    };
    let result = generate_totp("JBSWY3DPEHPK3PXP", &params).unwrap();
    assert_eq!(result.code.len(), 6);
}

#[test]
fn totp_custom_period() {
    let params = TotpParams {
        digits: 6,
        period: 60,
        algorithm: TotpAlgorithm::Sha1,
    };
    let result = generate_totp("JBSWY3DPEHPK3PXP", &params).unwrap();
    assert!(result.time_remaining > 0 && result.time_remaining <= 60);
}

#[test]
fn totp_rejects_invalid_base32() {
    let params = TotpParams::default();
    assert!(generate_totp("!!!INVALID!!!", &params).is_err());
}

#[test]
fn totp_rejects_empty_secret() {
    let params = TotpParams::default();
    assert!(generate_totp("", &params).is_err());
}

#[test]
fn time_remaining_within_bounds() {
    let remaining = time_remaining(30);
    assert!(remaining > 0 && remaining <= 30);

    let remaining_60 = time_remaining(60);
    assert!(remaining_60 > 0 && remaining_60 <= 60);
}

#[test]
fn otpauth_uri_parse_full() {
    let uri = "otpauth://totp/GitHub:user@example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub&algorithm=SHA256&digits=8&period=60";
    let parsed = from_otpauth_uri(uri).unwrap();
    assert_eq!(parsed.secret, "JBSWY3DPEHPK3PXP");
    assert_eq!(parsed.issuer.as_deref(), Some("GitHub"));
    assert_eq!(parsed.account.as_deref(), Some("user@example.com"));
    assert_eq!(parsed.algorithm, TotpAlgorithm::Sha256);
    assert_eq!(parsed.digits, 8);
    assert_eq!(parsed.period, 60);
}

#[test]
fn otpauth_uri_parse_minimal() {
    let uri = "otpauth://totp/user@example.com?secret=JBSWY3DPEHPK3PXP";
    let parsed = from_otpauth_uri(uri).unwrap();
    assert_eq!(parsed.secret, "JBSWY3DPEHPK3PXP");
    assert_eq!(parsed.digits, 6);
    assert_eq!(parsed.period, 30);
    assert_eq!(parsed.algorithm, TotpAlgorithm::Sha1);
}

#[test]
fn otpauth_uri_rejects_missing_secret() {
    let uri = "otpauth://totp/user@example.com?issuer=GitHub";
    assert!(from_otpauth_uri(uri).is_err());
}

#[test]
fn otpauth_uri_rejects_wrong_scheme() {
    let uri = "https://example.com/totp?secret=JBSWY3DPEHPK3PXP";
    assert!(from_otpauth_uri(uri).is_err());
}

#[test]
fn otpauth_uri_rejects_hotp() {
    let uri = "otpauth://hotp/user@example.com?secret=JBSWY3DPEHPK3PXP";
    assert!(from_otpauth_uri(uri).is_err());
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
        let parsed = from_otpauth_uri(&uri).unwrap();
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
