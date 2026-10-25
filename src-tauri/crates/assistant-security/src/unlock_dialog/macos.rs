use crate::unlock_dialog::{UnlockDialog, UnlockResult};
use crate::vault_crypto::types::SecureString;

use log::{info, warn};
use zeroize::Zeroize;

pub struct MacosUnlockDialog;

unsafe impl Send for MacosUnlockDialog {}
unsafe impl Sync for MacosUnlockDialog {}

impl MacosUnlockDialog {
    pub fn new() -> Result<Self, crate::types::VaultError> {
        Ok(Self)
    }
}

impl UnlockDialog for MacosUnlockDialog {
    fn show(
        &self,
        on_result: Box<dyn FnOnce(UnlockResult) + Send>,
    ) -> Result<(), crate::types::VaultError> {
        std::thread::spawn(move || match run_osascript_dialog() {
            Ok(password) => {
                info!("macOS unlock dialog: user confirmed");
                on_result(UnlockResult::Confirmed(password));
            }
            Err(DialogError::Cancelled) => {
                info!("macOS unlock dialog: user cancelled");
                on_result(UnlockResult::Cancelled);
            }
            Err(DialogError::Failed(e)) => {
                warn!("macOS unlock dialog failed: {}", e);
                on_result(UnlockResult::Cancelled);
            }
        });

        Ok(())
    }
}

enum DialogError {
    Cancelled,
    Failed(String),
}

fn run_osascript_dialog() -> Result<SecureString, DialogError> {
    let script = r#"
        try
            set dialogResult to display dialog "Enter master password:" default answer "" with hidden answer buttons {"Cancel", "Unlock"} default button "Unlock" with title "Vault Unlock"
            if button returned of dialogResult is "Unlock" then
                return text returned of dialogResult
            else
                error number -128
            end if
        on error number -128
            error number -128
        end try
    "#;

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| DialogError::Failed(format!("osascript spawn: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("-128") || stderr.contains("User canceled") {
            return Err(DialogError::Cancelled);
        }
        return Err(DialogError::Failed(format!(
            "osascript exited {}: {}",
            output.status,
            stderr.trim()
        )));
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
