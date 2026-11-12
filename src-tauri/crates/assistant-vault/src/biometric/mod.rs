use assistant_crypto::error::VaultError;

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
    fn ensure_credential(&self, key_id: &str) -> Result<(), VaultError> {
        let _ = key_id;
        Ok(())
    }
    fn request_signature(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        let _ = key_id;
        Err(VaultError::BiometricUnavailable)
    }
    fn set_key_unprotected(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        self.set_key(key_id, data)
    }

    /// R9：非 Windows 平台 wrapping key 与密文共用同一 key store。
    /// ACL 不满足时注册必须返回 `BiometricUnavailable`，禁止静默降级。
    /// Windows Hello（TPM）天然满足，默认返回 true。
    fn acl_satisfied(&self) -> Result<bool, VaultError> {
        Ok(true)
    }
}

#[cfg(target_os = "windows")]
pub fn warm_up_keyring_backend() -> Result<(), String> {
    match *keyring::Entry::store_status() {
        Ok(()) => Ok(()),
        Err(ref e) => Err(e.to_string()),
    }
}

pub fn create_key_store() -> Box<dyn BiometricKeyStore> {
    #[cfg(target_os = "windows")]
    {
        log::debug!("Biometric key store: Windows Hello");
        Box::new(windows::WindowsHelloKeyStore)
    }
    #[cfg(target_os = "macos")]
    {
        log::debug!("Biometric key store: Touch ID");
        Box::new(macos::TouchIdKeyStore)
    }
    #[cfg(target_os = "linux")]
    {
        log::debug!("Biometric key store: libsecret");
        Box::new(linux::LibsecretKeyStore)
    }
}
