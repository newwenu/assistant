use crate::biometric::BiometricKeyStore;
use crate::types::VaultError;

use std::fs;
use std::path::PathBuf;

use windows::core::HSTRING;
use windows::Foundation::{IAsyncAction, IAsyncOperation};
use windows::Security::Credentials::{
    KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
};
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Security::Cryptography::DataProtection::DataProtectionProvider;
use windows::Storage::Streams::IBuffer;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

pub struct WindowsHelloKeyStore;

const SERVICE_NAME: &str = "assistant-vault-biometric";

struct WinRtGuard;

impl WinRtGuard {
    fn new() -> Result<Self, VaultError> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .map_err(|e| VaultError::BiometricFailed(format!("RoInitialize: {e}")))?;
        Ok(Self)
    }
}

impl Drop for WinRtGuard {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

fn biometric_dir() -> Result<PathBuf, VaultError> {
    let local_app_data = dirs::data_local_dir()
        .ok_or_else(|| VaultError::BiometricFailed("LOCALAPPDATA not found".into()))?;
    Ok(local_app_data.join("assistant").join("biometric"))
}

fn key_file_path(key_id: &str) -> Result<PathBuf, VaultError> {
    let dir = biometric_dir()?;
    Ok(dir.join(format!("{key_id}.bin")))
}

fn wait_for_async<T>(async_op: IAsyncOperation<T>) -> windows::core::Result<T>
where
    T: windows::core::RuntimeType,
{
    async_op.get()
}

fn wait_for_action(async_op: IAsyncAction) -> windows::core::Result<()> {
    async_op.get()
}

fn bytes_to_buffer(data: &[u8]) -> windows::core::Result<IBuffer> {
    CryptographicBuffer::CreateFromByteArray(data)
}

fn buffer_to_bytes(buffer: &IBuffer) -> windows::core::Result<Vec<u8>> {
    let mut bytes = windows::core::Array::<u8>::new();
    CryptographicBuffer::CopyToByteArray(buffer, &mut bytes)?;
    Ok(bytes.as_ref().to_vec())
}

fn map_win_error(e: windows::core::Error) -> VaultError {
    VaultError::BiometricFailed(e.message().to_string())
}

fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, VaultError> {
    let provider = DataProtectionProvider::CreateOverloadExplicit(&HSTRING::from("LOCAL=user"))
        .map_err(map_win_error)?;
    let input = bytes_to_buffer(data).map_err(map_win_error)?;
    let output = provider
        .ProtectAsync(&input)
        .map_err(map_win_error)?
        .get()
        .map_err(map_win_error)?;
    buffer_to_bytes(&output).map_err(map_win_error)
}

fn dpapi_unprotect(encrypted: &[u8]) -> Result<Vec<u8>, VaultError> {
    let provider = DataProtectionProvider::new().map_err(map_win_error)?;
    let input = bytes_to_buffer(encrypted).map_err(map_win_error)?;
    let output = provider
        .UnprotectAsync(&input)
        .map_err(map_win_error)?
        .get()
        .map_err(map_win_error)?;
    buffer_to_bytes(&output).map_err(map_win_error)
}

fn write_dpapi_file(key_id: &str, encrypted: &[u8]) -> Result<(), VaultError> {
    let path = key_file_path(key_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, encrypted)?;
    Ok(())
}

fn read_dpapi_file(key_id: &str) -> Result<Vec<u8>, VaultError> {
    let path = key_file_path(key_id)?;
    if !path.exists() {
        return Err(VaultError::BiometricUnavailable);
    }
    Ok(fs::read(&path)?)
}

fn delete_dpapi_file(key_id: &str) -> Result<(), VaultError> {
    let path = key_file_path(key_id)?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

fn credential_name(key_id: &str) -> HSTRING {
    HSTRING::from(format!("{SERVICE_NAME}:{key_id}").as_str())
}

fn get_credential_status(
    result: &windows::Security::Credentials::KeyCredentialRetrievalResult,
) -> Result<KeyCredentialStatus, VaultError> {
    result.Status().map_err(map_win_error)
}

fn get_operation_status(
    result: &windows::Security::Credentials::KeyCredentialOperationResult,
) -> Result<KeyCredentialStatus, VaultError> {
    result.Status().map_err(map_win_error)
}

impl BiometricKeyStore for WindowsHelloKeyStore {
    fn is_available(&self) -> Result<bool, VaultError> {
        let _guard = WinRtGuard::new()?;
        let supported = KeyCredentialManager::IsSupportedAsync().map_err(map_win_error)?;
        let result = wait_for_async(supported).map_err(map_win_error)?;
        Ok(result)
    }

    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        let _guard = WinRtGuard::new()?;
        let name = credential_name(key_id);

        let open_result = KeyCredentialManager::OpenAsync(&name)
            .map_err(map_win_error)?
            .get()
            .map_err(map_win_error)?;

        if get_credential_status(&open_result)? == KeyCredentialStatus::NotFound {
            let create_result = KeyCredentialManager::RequestCreateAsync(
                &name,
                KeyCredentialCreationOption::FailIfExists,
            )
            .map_err(map_win_error)?
            .get()
            .map_err(map_win_error)?;

            match get_credential_status(&create_result)? {
                KeyCredentialStatus::Success => {}
                KeyCredentialStatus::CredentialAlreadyExists => {}
                KeyCredentialStatus::UserCanceled => {
                    return Err(VaultError::BiometricCancelled);
                }
                status => {
                    return Err(VaultError::BiometricFailed(format!(
                        "KeyCredential creation failed: {status:?}"
                    )));
                }
            }
        }

        let encrypted = dpapi_protect(data)?;
        write_dpapi_file(key_id, &encrypted)?;
        Ok(())
    }

    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        let _guard = WinRtGuard::new()?;
        let name = credential_name(key_id);

        let open_result = KeyCredentialManager::OpenAsync(&name)
            .map_err(map_win_error)?
            .get()
            .map_err(map_win_error)?;

        if get_credential_status(&open_result)? != KeyCredentialStatus::Success {
            return Err(VaultError::BiometricUnavailable);
        }

        let credential = open_result.Credential().map_err(map_win_error)?;

        let mut challenge = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut challenge);

        let challenge_buffer = bytes_to_buffer(&challenge).map_err(map_win_error)?;

        let sign_result = credential
            .RequestSignAsync(&challenge_buffer)
            .map_err(map_win_error)?
            .get()
            .map_err(map_win_error)?;

        match get_operation_status(&sign_result)? {
            KeyCredentialStatus::Success => {}
            KeyCredentialStatus::UserCanceled => {
                return Err(VaultError::BiometricCancelled);
            }
            status => {
                return Err(VaultError::BiometricFailed(format!(
                    "TPM sign failed: {status:?}"
                )));
            }
        }

        let encrypted = read_dpapi_file(key_id)?;
        let data = dpapi_unprotect(&encrypted)?;
        Ok(data)
    }

    fn delete_key(&self, key_id: &str) -> Result<(), VaultError> {
        let _guard = WinRtGuard::new()?;
        let name = credential_name(key_id);

        let delete_op = KeyCredentialManager::DeleteAsync(&name);
        if let Ok(op) = delete_op {
            let _ = wait_for_action(op);
        }

        let _ = delete_dpapi_file(key_id);
        Ok(())
    }

    fn has_key(&self, key_id: &str) -> Result<bool, VaultError> {
        let _guard = WinRtGuard::new()?;
        let name = credential_name(key_id);

        let open_result = KeyCredentialManager::OpenAsync(&name)
            .map_err(map_win_error)?
            .get()
            .map_err(map_win_error)?;

        Ok(get_credential_status(&open_result)? == KeyCredentialStatus::Success)
    }
}
