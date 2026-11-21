use assistant_crypto::error::VaultError;
use assistant_crypto::vault_crypto::types::SecureString;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub enum UnlockResult {
    Confirmed(SecureString),
    Cancelled,
}

pub trait UnlockDialog: Send + Sync {
    fn show(&self, on_result: Box<dyn FnOnce(UnlockResult) + Send>) -> Result<(), VaultError>;
}

pub fn create_unlock_dialog() -> Result<Box<dyn UnlockDialog>, VaultError> {
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(windows::Win32UnlockDialog::new()?))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacosUnlockDialog::new()?))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LinuxUnlockDialog::new()?))
    }
}
