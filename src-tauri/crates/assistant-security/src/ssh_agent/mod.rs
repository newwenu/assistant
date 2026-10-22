pub mod validate;

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix;

#[cfg(target_os = "windows")]
mod windows;

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::types::VaultError;

pub trait SshAgentBridge: Send + Sync {
    fn add_key(
        &self,
        private_key: &[u8],
        fingerprint: &str,
        lifetime: Option<u32>,
    ) -> Result<(), VaultError>;
    fn remove_key(&self, fingerprint: &str) -> Result<(), VaultError>;
    fn list_keys(&self) -> Result<Vec<SshKeyInfo>, VaultError>;
    fn is_agent_running(&self) -> Result<bool, VaultError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyInfo {
    pub fingerprint: String,
    pub key_type: String,
    pub comment: String,
    pub bits: u32,
    #[serde(default)]
    pub added_by_vault: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyMeta {
    pub key_type: String,
    pub bits: u32,
    pub fingerprint: String,
    pub comment: String,
    pub public_key: String,
}

pub fn create_agent_bridge(
    use_pageant: bool,
    auth_sock_override: Option<String>,
) -> Box<dyn SshAgentBridge> {
    #[cfg(target_os = "windows")]
    {
        let _ = auth_sock_override;
        if use_pageant {
            return Box::new(windows::PageantBridge);
        }
        Box::new(windows::OpenSshAgentBridge)
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let _ = use_pageant;
        Box::new(unix::UnixSshAgentBridge { auth_sock_override })
    }
}

fn run_ssh_add(
    args: &[&str],
    auth_sock_override: Option<&str>,
) -> Result<std::process::Output, VaultError> {
    let ssh_add = find_ssh_add_binary()?;
    let mut cmd = std::process::Command::new(&ssh_add);
    cmd.args(args);
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    inject_ssh_auth_sock(&mut cmd, auth_sock_override);
    #[cfg(target_os = "windows")]
    let _ = auth_sock_override;
    cmd.output()
        .map_err(|e| VaultError::Other(format!("ssh-add execution failed: {}", e)))
}

pub fn find_ssh_add_binary() -> Result<String, VaultError> {
    if cfg!(target_os = "windows") {
        let candidates = [
            std::env::var("GIT_INSTALL_ROOT")
                .map(|root| format!("{}\\usr\\bin\\ssh-add.exe", root)),
            Ok("C:\\Program Files\\Git\\usr\\bin\\ssh-add.exe".to_string()),
            Ok("C:\\Program Files (x86)\\Git\\usr\\bin\\ssh-add.exe".to_string()),
        ];
        for path in candidates.into_iter().flatten() {
            if std::path::Path::new(&path).exists() {
                return Ok(path);
            }
        }
        Err(VaultError::Other(
            "ssh-add not found: install Git for Windows".to_string(),
        ))
    } else {
        Ok("ssh-add".to_string())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn inject_ssh_auth_sock(cmd: &mut std::process::Command, override_path: Option<&str>) {
    if let Some(path) = override_path {
        cmd.env("SSH_AUTH_SOCK", path);
    }
}

impl fmt::Display for SshKeyInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} ({} bits)",
            self.key_type, self.fingerprint, self.comment, self.bits
        )
    }
}
