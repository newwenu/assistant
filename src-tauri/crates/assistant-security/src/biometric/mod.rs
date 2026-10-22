use crate::types::VaultError;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub trait BiometricKeyStore: Send + Sync {
    fn is_available(&self) -> Result<bool, VaultError>;
    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError>;
    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError>;
    fn delete_key(&self, key_id: &str) -> Result<(), VaultError>;
    fn has_key(&self, key_id: &str) -> Result<bool, VaultError>;
}

pub fn create_key_store() -> Box<dyn BiometricKeyStore> {
    #[cfg(target_os = "windows")]
    {
        log::debug!("Biometric key store: Windows Hello");
        return Box::new(windows::WindowsHelloKeyStore);
    }
    #[cfg(target_os = "macos")]
    {
        log::debug!("Biometric key store: Touch ID");
        return Box::new(macos::TouchIdKeyStore);
    }
    #[cfg(target_os = "linux")]
    {
        log::debug!("Biometric key store: libsecret");
        return Box::new(linux::LibsecretKeyStore);
    }
}
