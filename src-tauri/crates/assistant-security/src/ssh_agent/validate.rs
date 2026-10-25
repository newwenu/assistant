use super::SshKeyMeta;
use crate::types::VaultError;
use log::debug;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use ssh_key::PrivateKey;

pub fn validate_private_key(key_data: &[u8]) -> Result<SshKeyMeta, VaultError> {
    if key_data.is_empty() {
        return Err(VaultError::InvalidKeyFormat(
            "Key data is empty".to_string(),
        ));
    }

    let key_str = std::str::from_utf8(key_data)
        .map_err(|_| VaultError::InvalidKeyFormat("Key is not valid UTF-8".to_string()))?;
    let trimmed = key_str.trim();

    if trimmed.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----") {
        validate_openssh_key(trimmed)
    } else if trimmed.starts_with("-----BEGIN RSA PRIVATE KEY-----") {
        validate_pkcs1_rsa_key(trimmed)
    } else if trimmed.starts_with("-----BEGIN PRIVATE KEY-----") {
        validate_pkcs8_key(trimmed)
    } else if trimmed.starts_with("-----BEGIN EC PRIVATE KEY-----")
        || trimmed.starts_with("-----BEGIN DSA PRIVATE KEY-----")
    {
        Err(VaultError::InvalidKeyFormat(
            "Legacy PEM format not supported. Please convert to OpenSSH format: ssh-keygen -p -f <key> -m RFC4716".to_string(),
        ))
    } else if trimmed.starts_with("PuTTY-User-Key-File-") {
        validate_ppk_key(trimmed)
    } else {
        Err(VaultError::InvalidKeyFormat(
            "Unrecognized key format. Expected OpenSSH, PEM (RSA/PKCS8), or PuTTY .ppk".to_string(),
        ))
    }
}

fn validate_openssh_key(key_str: &str) -> Result<SshKeyMeta, VaultError> {
    if !key_str.contains("-----END OPENSSH PRIVATE KEY-----") {
        return Err(VaultError::InvalidKeyFormat(
            "Incomplete OpenSSH private key: missing END marker".to_string(),
        ));
    }

    let private_key = PrivateKey::from_openssh(key_str)
        .map_err(|e| VaultError::InvalidKeyFormat(format!("Invalid OpenSSH key: {}", e)))?;
    extract_meta(&private_key)
}

fn validate_pkcs1_rsa_key(key_str: &str) -> Result<SshKeyMeta, VaultError> {
    if !key_str.contains("-----END RSA PRIVATE KEY-----") {
        return Err(VaultError::InvalidKeyFormat(
            "Incomplete PKCS#1 RSA key: missing END marker".to_string(),
        ));
    }

    let rsa_key = rsa::RsaPrivateKey::from_pkcs1_pem(key_str)
        .map_err(|e| VaultError::InvalidKeyFormat(format!("Invalid PKCS#1 RSA key: {}", e)))?;
    rsa_to_meta(&rsa_key)
}

fn validate_pkcs8_key(key_str: &str) -> Result<SshKeyMeta, VaultError> {
    if !key_str.contains("-----END PRIVATE KEY-----") {
        return Err(VaultError::InvalidKeyFormat(
            "Incomplete PKCS#8 key: missing END marker".to_string(),
        ));
    }

    match rsa::RsaPrivateKey::from_pkcs8_pem(key_str) {
        Ok(rsa_key) => rsa_to_meta(&rsa_key),
        Err(_) => Err(VaultError::InvalidKeyFormat(
            "PKCS#8 EC/DSA key not supported. Please convert to OpenSSH format: ssh-keygen -p -f <key> -m RFC4716".to_string(),
        )),
    }
}

fn rsa_to_meta(rsa_key: &rsa::RsaPrivateKey) -> Result<SshKeyMeta, VaultError> {
    let keypair = ssh_key::private::RsaKeypair::try_from(rsa_key)
        .map_err(|e| VaultError::InvalidKeyFormat(format!("RSA key conversion failed: {}", e)))?;
    let private_key =
        PrivateKey::new(ssh_key::private::KeypairData::Rsa(keypair), "").map_err(|e| {
            VaultError::InvalidKeyFormat(format!("PrivateKey construction failed: {}", e))
        })?;
    extract_meta(&private_key)
}

fn validate_ppk_key(key_str: &str) -> Result<SshKeyMeta, VaultError> {
    if !key_str.contains("Private-Lines:") {
        return Err(VaultError::InvalidKeyFormat(
            "Invalid PuTTY .ppk key: missing Private-Lines section".to_string(),
        ));
    }

    let key_type = key_str
        .lines()
        .find(|l| l.starts_with("PuTTY-User-Key-File-"))
        .map(|l| {
            let suffix = l.trim_start_matches("PuTTY-User-Key-File-");
            let version: String = suffix.chars().take_while(|c| c.is_ascii_digit()).collect();
            if version.is_empty() {
                suffix.to_string()
            } else {
                format!("ppk-v{}", version)
            }
        })
        .unwrap_or_else(|| "ppk".to_string());

    Ok(SshKeyMeta {
        key_type,
        bits: 0,
        fingerprint: String::new(),
        comment: String::new(),
        public_key: String::new(),
    })
}

fn extract_meta(private_key: &PrivateKey) -> Result<SshKeyMeta, VaultError> {
    let public_key = private_key.public_key();
    let fingerprint = public_key.fingerprint(ssh_key::HashAlg::Sha256).to_string();
    let algorithm = public_key.algorithm();
    let key_type_str = algorithm.as_str();
    let key_type = simplify_key_type(key_type_str);
    let bits = compute_bit_size(public_key);
    let comment = private_key.comment().to_string();

    debug!(
        "SSH key validated: {} {} ({} bits)",
        key_type, fingerprint, bits
    );

    Ok(SshKeyMeta {
        key_type,
        bits,
        fingerprint,
        comment,
        public_key: public_key.to_openssh().unwrap_or_default(),
    })
}

fn simplify_key_type(ssh_type: &str) -> String {
    match ssh_type {
        "ssh-ed25519" => "ed25519".to_string(),
        "ssh-rsa" => "rsa".to_string(),
        s => s.to_string(),
    }
}

fn compute_bit_size(public_key: &ssh_key::PublicKey) -> u32 {
    use ssh_key::public::KeyData;

    match public_key.key_data() {
        KeyData::Rsa(rsa) => {
            let n_bytes = rsa.n.as_bytes();
            let effective_len = if n_bytes.first() == Some(&0) && n_bytes.len() > 1 {
                n_bytes.len() - 1
            } else {
                n_bytes.len()
            };
            effective_len as u32 * 8
        }
        KeyData::Ed25519(_) => 256,
        KeyData::Ecdsa(ecdsa) => match ecdsa.curve() {
            ssh_key::EcdsaCurve::NistP256 => 256,
            ssh_key::EcdsaCurve::NistP384 => 384,
            ssh_key::EcdsaCurve::NistP521 => 521,
        },
        KeyData::Dsa(_) => 1024,
        KeyData::SkEcdsaSha2NistP256(_) => 256,
        KeyData::SkEd25519(_) => 256,
        KeyData::Other(_) | _ => 0,
    }
}
