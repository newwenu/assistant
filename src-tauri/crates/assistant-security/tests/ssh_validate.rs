use assistant_crypto::error::VaultError;
use assistant_security::ssh_agent::validate::validate_private_key;
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::pkcs8::EncodePrivateKey;

fn expect_invalid(
    result: Result<assistant_security::ssh_agent::SshKeyMeta, VaultError>,
    needle: &str,
) {
    match result {
        Err(VaultError::InvalidKeyFormat(msg)) => {
            assert!(msg.contains(needle), "unexpected message: {msg}")
        }
        other => panic!("expected InvalidKeyFormat containing {needle:?}, got {other:?}"),
    }
}

#[test]
fn rejects_empty_non_utf8_and_unrecognized_input() {
    expect_invalid(validate_private_key(b""), "empty");
    expect_invalid(validate_private_key(&[0xff, 0xfe, 0xfd]), "not valid UTF-8");
    expect_invalid(
        validate_private_key(b"ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQ== user@host"),
        "Unrecognized key format",
    );
}

#[test]
fn rejects_legacy_pem_formats_with_conversion_hint() {
    expect_invalid(
        validate_private_key(
            b"-----BEGIN EC PRIVATE KEY-----\nAAAA\n-----END EC PRIVATE KEY-----\n",
        ),
        "Legacy PEM format not supported",
    );
    expect_invalid(
        validate_private_key(
            b"-----BEGIN DSA PRIVATE KEY-----\nAAAA\n-----END DSA PRIVATE KEY-----\n",
        ),
        "Legacy PEM format not supported",
    );
}

#[test]
fn rejects_incomplete_key_blocks() {
    expect_invalid(
        validate_private_key(b"-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n"),
        "missing END marker",
    );
    expect_invalid(
        validate_private_key(b"-----BEGIN RSA PRIVATE KEY-----\nAAAA\n"),
        "missing END marker",
    );
    expect_invalid(
        validate_private_key(b"-----BEGIN PRIVATE KEY-----\nAAAA\n"),
        "missing END marker",
    );
    expect_invalid(
        validate_private_key(b"PuTTY-User-Key-File-2: ssh-rsa\n"),
        "missing Private-Lines",
    );
}

#[test]
fn accepts_openssh_pkcs1_pkcs8_and_ppk_formats() {
    let rsa_key = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 2048).unwrap();
    let keypair = ssh_key::private::RsaKeypair::try_from(&rsa_key).unwrap();
    let openssh = ssh_key::PrivateKey::new(
        ssh_key::private::KeypairData::Rsa(keypair),
        "vault-test@example",
    )
    .unwrap()
    .to_openssh(ssh_key::LineEnding::LF)
    .unwrap();

    let from_openssh = validate_private_key(openssh.as_bytes()).unwrap();
    assert_eq!(from_openssh.key_type, "rsa");
    assert_eq!(from_openssh.bits, 2048);
    assert_eq!(from_openssh.comment, "vault-test@example");
    assert!(from_openssh.fingerprint.starts_with("SHA256:"));
    assert!(from_openssh.public_key.starts_with("ssh-rsa "));

    let pkcs1 = rsa_key.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF).unwrap();
    let from_pkcs1 = validate_private_key(pkcs1.as_bytes()).unwrap();
    assert_eq!(from_pkcs1.key_type, "rsa");
    assert_eq!(from_pkcs1.bits, 2048);
    assert_eq!(from_pkcs1.fingerprint, from_openssh.fingerprint);

    let pkcs8 = rsa_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();
    let from_pkcs8 = validate_private_key(pkcs8.as_bytes()).unwrap();
    assert_eq!(from_pkcs8.key_type, "rsa");
    assert_eq!(from_pkcs8.bits, 2048);
    assert_eq!(from_pkcs8.fingerprint, from_openssh.fingerprint);

    let ppk = b"PuTTY-User-Key-File-2: ssh-rsa\nEncryption: none\nComment: imported\nPrivate-Lines: 1\nAAAA\n";
    let from_ppk = validate_private_key(ppk).unwrap();
    assert_eq!(from_ppk.key_type, "ppk-v2");
    assert_eq!(from_ppk.bits, 0);
    assert!(from_ppk.fingerprint.is_empty());
}
