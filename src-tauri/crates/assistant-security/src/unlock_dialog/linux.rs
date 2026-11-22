use crate::unlock_dialog::{UnlockDialog, UnlockResult};
use assistant_crypto::vault_crypto::types::SecureString;

use log::{info, warn};
use zeroize::Zeroize;

pub struct LinuxUnlockDialog {
    backend: LinuxBackend,
}

#[derive(Clone, Copy)]
enum LinuxBackend {
    Zenity,
    Kdialog,
}

unsafe impl Send for LinuxUnlockDialog {}
unsafe impl Sync for LinuxUnlockDialog {}

impl LinuxUnlockDialog {
    pub fn new() -> Result<Self, assistant_crypto::error::VaultError> {
        let backend = if is_zenity_available() {
            info!("Linux unlock dialog: using zenity backend");
            LinuxBackend::Zenity
        } else if is_kdialog_available() {
            info!("Linux unlock dialog: using kdialog backend");
            LinuxBackend::Kdialog
        } else {
            return Err(assistant_crypto::error::VaultError::Other(
                "No native dialog backend available (need zenity or kdialog)".into(),
            ));
        };
        Ok(Self { backend })
    }
}

impl UnlockDialog for LinuxUnlockDialog {
    fn show(
        &self,
        on_result: Box<dyn FnOnce(UnlockResult) + Send>,
    ) -> Result<(), assistant_crypto::error::VaultError> {
        let backend = self.backend;
        std::thread::spawn(move || match backend {
            LinuxBackend::Zenity => match run_zenity_dialog() {
                Ok(password) => {
                    info!("Linux unlock dialog (zenity): user confirmed");
                    on_result(UnlockResult::Confirmed(password));
                }
                Err(DialogError::Cancelled) => {
                    info!("Linux unlock dialog (zenity): user cancelled");
                    on_result(UnlockResult::Cancelled);
                }
                Err(DialogError::Failed(e)) => {
                    warn!("Linux unlock dialog (zenity) failed: {}", e);
                    on_result(UnlockResult::Cancelled);
                }
            },
            LinuxBackend::Kdialog => match run_kdialog_dialog() {
                Ok(password) => {
                    info!("Linux unlock dialog (kdialog): user confirmed");
                    on_result(UnlockResult::Confirmed(password));
                }
                Err(DialogError::Cancelled) => {
                    info!("Linux unlock dialog (kdialog): user cancelled");
                    on_result(UnlockResult::Cancelled);
                }
                Err(DialogError::Failed(e)) => {
                    warn!("Linux unlock dialog (kdialog) failed: {}", e);
                    on_result(UnlockResult::Cancelled);
                }
            },
        });

        Ok(())
    }
}

enum DialogError {
    Cancelled,
    Failed(String),
}

fn is_zenity_available() -> bool {
    std::process::Command::new("zenity")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn is_kdialog_available() -> bool {
    std::process::Command::new("kdialog")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_zenity_dialog() -> Result<SecureString, DialogError> {
    let output = std::process::Command::new("zenity")
        .args([
            "--entry",
            "--title=Vault Unlock",
            "--text=Enter master password:",
            "--hide-text",
            "--width=380",
        ])
        .output()
        .map_err(|e| DialogError::Failed(format!("zenity spawn: {e}")))?;

    if !output.status.success() {
        return Err(DialogError::Cancelled);
    }

    let mut stdout = output.stdout;
    while stdout.last() == Some(&b'\n') || stdout.last() == Some(&b'\r') {
        stdout.pop();
    }

    let password = match String::from_utf8(stdout) {
        Ok(s) => s,
        Err(e) => {
            let mut bytes = e.into_bytes();
            bytes.zeroize();
            return Err(DialogError::Failed("stdout not valid UTF-8".into()));
        }
    };

    Ok(SecureString::new(password))
}

fn run_kdialog_dialog() -> Result<SecureString, DialogError> {
    let output = std::process::Command::new("kdialog")
        .args([
            "--password",
            "Enter master password:",
            "--title",
            "Vault Unlock",
        ])
        .output()
        .map_err(|e| DialogError::Failed(format!("kdialog spawn: {e}")))?;

    if !output.status.success() {
        return Err(DialogError::Cancelled);
    }

    let mut stdout = output.stdout;
    while stdout.last() == Some(&b'\n') || stdout.last() == Some(&b'\r') {
        stdout.pop();
    }

    let password = match String::from_utf8(stdout) {
        Ok(s) => s,
        Err(e) => {
            let mut bytes = e.into_bytes();
            bytes.zeroize();
            return Err(DialogError::Failed("stdout not valid UTF-8".into()));
        }
    };

    Ok(SecureString::new(password))
}
