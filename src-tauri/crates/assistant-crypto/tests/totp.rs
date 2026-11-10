use assistant_crypto::vault_crypto::totp::{
    decode_base32, generate_at, parse_otpauth, time_remaining_at, TotpAlgorithm, TotpSpec,
};

const SECRET: &str = "JBSWY3DPEHPK3PXP";
const NOW: u64 = 1_700_000_000;

// 原 5 个用例只断言「码长 + 纯数字 + time_remaining 范围」，SHA1/SHA256/SHA512 互换、
// 动态截断（offset = 末字节 & 0x0f）写错时仍会全绿，已合并替换为下方 RFC 6238 向量。
/// RFC 6238 Appendix B known-answer vector（各算法使用各自长度的 seed）。
/// T=59 与 T=1111111111 覆盖单步与多步 counter，8 位 / 6 位覆盖截断后取模。
#[test]
fn totp_matches_rfc6238_appendix_b_vectors() {
    const SHA1_SEED: &[u8] = b"12345678901234567890";
    const SHA256_SEED: &[u8] = b"12345678901234567890123456789012";
    const SHA512_SEED: &[u8] = b"1234567890123456789012345678901234567890123456789012345678901234";

    let cases: [(&[u8], TotpAlgorithm, u32, u64, &str); 7] = [
        (SHA1_SEED, TotpAlgorithm::Sha1, 8, 59, "94287082"),
        (SHA256_SEED, TotpAlgorithm::Sha256, 8, 59, "46119246"),
        (SHA512_SEED, TotpAlgorithm::Sha512, 8, 59, "90693936"),
        (SHA1_SEED, TotpAlgorithm::Sha1, 6, 59, "287082"),
        (SHA1_SEED, TotpAlgorithm::Sha1, 8, 1_111_111_111, "14050471"),
        (
            SHA256_SEED,
            TotpAlgorithm::Sha256,
            8,
            1_111_111_111,
            "67062674",
        ),
        (
            SHA512_SEED,
            TotpAlgorithm::Sha512,
            8,
            1_111_111_111,
            "99943326",
        ),
    ];

    for (seed, algorithm, digits, unix_time, expected) in cases {
        let code = generate_at(seed, &TotpSpec::new(digits, 30, algorithm), unix_time).unwrap();
        assert_eq!(
            code.code, expected,
            "{algorithm:?} / {digits} digits / T={unix_time}"
        );
    }

    assert_eq!(
        decode_base32("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap(),
        SHA1_SEED
    );
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
