use std::io::Write;
use std::process::Stdio;

use log::{debug, info, warn};

use super::{run_ssh_add, SshAgentBridge, SshKeyInfo};
use assistant_crypto::error::VaultError;

pub struct PageantBridge;

impl SshAgentBridge for PageantBridge {
    fn add_key(
        &self,
        private_key: &[u8],
        _fingerprint: &str,
        lifetime: Option<u32>,
    ) -> Result<(), VaultError> {
        add_key_via_ssh_add(private_key, lifetime)
    }

    fn remove_key(&self, fingerprint: &str) -> Result<(), VaultError> {
        let output = run_ssh_add(&["-d", fingerprint], None)?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(VaultError::Other(format!(
                "ssh-add -d failed (Pageant): {}",
                stderr.trim()
            )))
        }
    }

    fn list_keys(&self) -> Result<Vec<SshKeyInfo>, VaultError> {
        list_keys_via_ssh_add()
    }

    fn is_agent_running(&self) -> Result<bool, VaultError> {
        is_pageant_running()
    }
}

pub struct OpenSshAgentBridge;

impl SshAgentBridge for OpenSshAgentBridge {
    fn add_key(
        &self,
        private_key: &[u8],
        _fingerprint: &str,
        lifetime: Option<u32>,
    ) -> Result<(), VaultError> {
        add_key_via_ssh_add(private_key, lifetime)
    }

    fn remove_key(&self, fingerprint: &str) -> Result<(), VaultError> {
        let output = run_ssh_add(&["-d", fingerprint], None)?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(VaultError::Other(format!(
                "ssh-add -d failed (OpenSSH Agent): {}",
                stderr.trim()
            )))
        }
    }

    fn list_keys(&self) -> Result<Vec<SshKeyInfo>, VaultError> {
        list_keys_via_ssh_add()
    }

    fn is_agent_running(&self) -> Result<bool, VaultError> {
        let pipe_name = r"\\.\pipe\openssh-ssh-agent";
        std::fs::metadata(pipe_name).map(|_| true).or(Ok(false))
    }
}

fn add_key_via_ssh_add(private_key: &[u8], lifetime: Option<u32>) -> Result<(), VaultError> {
    let mut args: Vec<String> = Vec::new();
    if let Some(secs) = lifetime {
        args.push("-t".to_string());
        args.push(secs.to_string());
    }
    args.push("-".to_string());

    let ssh_add = super::find_ssh_add_binary()?;
    let mut cmd = std::process::Command::new(&ssh_add);
    cmd.args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        warn!("ssh-add spawn failed: {}", e);
        VaultError::Other(format!("ssh-add spawn failed: {}", e))
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(private_key).map_err(|e| {
            warn!("ssh-add stdin write failed: {}", e);
            VaultError::Other(format!("ssh-add stdin write failed: {}", e))
        })?;
        drop(stdin);
    }

    let output = child.wait_with_output().map_err(|e| {
        warn!("ssh-add wait failed: {}", e);
        VaultError::Other(format!("ssh-add wait failed: {}", e))
    })?;
    if output.status.success() {
        info!("SSH key added to agent via ssh-add");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("ssh-add failed: {}", stderr.trim());
        Err(VaultError::Other(format!(
            "ssh-add failed: {}",
            stderr.trim()
        )))
    }
}

fn list_keys_via_ssh_add() -> Result<Vec<SshKeyInfo>, VaultError> {
    let output = run_ssh_add(&["-l"], None)?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_ssh_add_list(&stdout))
}

fn is_pageant_running() -> Result<bool, VaultError> {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Pageant.exe", "/NH"])
        .output()
        .map_err(|e| {
            warn!("tasklist failed: {}", e);
            VaultError::Other(format!("tasklist failed: {}", e))
        })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let running = stdout.contains("Pageant.exe");
    debug!("Pageant running: {}", running);
    Ok(running)
}

fn parse_ssh_add_list(output: &str) -> Vec<SshKeyInfo> {
    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            if parts.len() < 3 {
                return None;
            }
            let bits: u32 = parts[0].parse().ok()?;
            let fingerprint = parts[1].to_string();
            let key_type = parts[2].to_string();
            let comment = if parts.len() > 3 {
                parts[3].to_string()
            } else {
                String::new()
            };
            Some(SshKeyInfo {
                fingerprint,
                key_type,
                comment,
                bits,
                added_by_vault: false,
            })
        })
        .collect()
}
