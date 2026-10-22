use super::SshKeyMeta;
use crate::types::VaultError;
use base64::Engine;

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
    } else if trimmed.starts_with("-----BEGIN RSA PRIVATE KEY-----")
        || trimmed.starts_with("-----BEGIN EC PRIVATE KEY-----")
        || trimmed.starts_with("-----BEGIN DSA PRIVATE KEY-----")
        || trimmed.starts_with("-----BEGIN PRIVATE KEY-----")
    {
        validate_pem_key(trimmed)
    } else if trimmed.starts_with("PuTTY-User-Key-File-") {
        validate_ppk_key(trimmed)
    } else {
        Err(VaultError::InvalidKeyFormat(
            "Unrecognized key format. Expected OpenSSH, PEM, or PuTTY .ppk".to_string(),
        ))
    }
}

fn validate_openssh_key(key_str: &str) -> Result<SshKeyMeta, VaultError> {
    if !key_str.contains("-----END OPENSSH PRIVATE KEY-----") {
        return Err(VaultError::InvalidKeyFormat(
            "Incomplete OpenSSH private key: missing END marker".to_string(),
        ));
    }

    let body: String = key_str
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();

    base64::engine::general_purpose::STANDARD
        .decode(&body)
        .map_err(|_| {
            VaultError::InvalidKeyFormat("OpenSSH key: invalid base64 encoding".to_string())
        })?;

    let meta = extract_meta_via_ssh_keygen(key_str)?;
    Ok(meta)
}

fn validate_pem_key(key_str: &str) -> Result<SshKeyMeta, VaultError> {
    if !key_str.contains("-----END") {
        return Err(VaultError::InvalidKeyFormat(
            "Incomplete PEM key: missing END marker".to_string(),
        ));
    }

    let meta = extract_meta_via_ssh_keygen(key_str)?;
    Ok(meta)
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

    let meta = extract_meta_via_ssh_keygen(key_str).unwrap_or(SshKeyMeta {
        key_type,
        bits: 0,
        fingerprint: String::new(),
        comment: String::new(),
        public_key: String::new(),
    });
    Ok(meta)
}

fn extract_meta_via_ssh_keygen(key_str: &str) -> Result<SshKeyMeta, VaultError> {
    let ssh_keygen = find_ssh_keygen_binary()?;

    let mut child = std::process::Command::new(&ssh_keygen)
        .args(["-l", "-f", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| VaultError::Other(format!("ssh-keygen spawn failed: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(key_str.as_bytes());
        drop(stdin);
    }

    let output = child
        .wait_with_output()
        .map_err(|e| VaultError::Other(format!("ssh-keygen wait failed: {}", e)))?;

    if !output.status.success() {
        return Err(VaultError::InvalidKeyFormat(
            "ssh-keygen could not parse the key".to_string(),
        ));
    }

    let fingerprint_line = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_fingerprint_line(&fingerprint_line)
}

fn parse_fingerprint_line(line: &str) -> Result<SshKeyMeta, VaultError> {
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err(VaultError::InvalidKeyFormat(format!(
            "Unexpected ssh-keygen output: {}",
            line
        )));
    }

    let bits: u32 = parts[0].parse().unwrap_or(0);
    let fingerprint = parts[1].to_string();
    let comment = if parts.len() > 2 {
        parts[2]
            .trim_start_matches('(')
            .trim_end_matches(')')
            .to_string()
    } else {
        String::new()
    };

    let key_type = if fingerprint.starts_with("SHA256:") {
        if bits == 256 {
            "ed25519".to_string()
        } else if bits == 384 {
            "ecdsa-sha2-nistp384".to_string()
        } else if bits == 521 {
            "ecdsa-sha2-nistp521".to_string()
        } else if bits == 2048 || bits == 3072 || bits == 4096 || bits == 8192 {
            "rsa".to_string()
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    };

    Ok(SshKeyMeta {
        key_type,
        bits,
        fingerprint,
        comment,
        public_key: String::new(),
    })
}

fn find_ssh_keygen_binary() -> Result<String, VaultError> {
    if cfg!(target_os = "windows") {
        let candidates = [
            std::env::var("GIT_INSTALL_ROOT")
                .map(|root| format!("{}\\usr\\bin\\ssh-keygen.exe", root)),
            Ok("C:\\Program Files\\Git\\usr\\bin\\ssh-keygen.exe".to_string()),
            Ok("C:\\Program Files (x86)\\Git\\usr\\bin\\ssh-keygen.exe".to_string()),
        ];
        for path in candidates.into_iter().flatten() {
            if std::path::Path::new(&path).exists() {
                return Ok(path);
            }
        }
        Err(VaultError::Other(
            "ssh-keygen not found: install Git for Windows".to_string(),
        ))
    } else {
        Ok("ssh-keygen".to_string())
    }
}
