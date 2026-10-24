use crate::biometric::BiometricKeyStore;
use crate::types::VaultError;

use log::{debug, info, warn};

use base64::Engine;

use windows::core::HRESULT;
use windows::core::HSTRING;
use windows::Security::Credentials::{
    KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
};
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Storage::Streams::IBuffer;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};
use windows_future::{IAsyncAction, IAsyncOperation};

const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106u32 as i32);

pub struct WindowsHelloKeyStore;

const SERVICE_NAME: &str = "assistant-vault-biometric";
const KEYRING_SERVICE: &str = "assistant-vault-biometric";

struct WinRtGuard;

impl WinRtGuard {
    fn new() -> Result<Self, VaultError> {
        let hr = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        if let Err(e) = &hr {
            if e.code() != RPC_E_CHANGED_MODE {
                return Err(VaultError::BiometricFailed(format!("RoInitialize: {e}")));
            }
        }
        Ok(Self)
    }
}

impl Drop for WinRtGuard {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

fn allow_foreground_window() {
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
}

fn wait_for_async<T>(async_op: IAsyncOperation<T>) -> windows::core::Result<T>
where
    T: windows::core::RuntimeType,
{
    async_op.join()
}

fn wait_for_action(async_op: IAsyncAction) -> windows::core::Result<()> {
    async_op.join()
}

fn bytes_to_buffer(data: &[u8]) -> windows::core::Result<IBuffer> {
    CryptographicBuffer::CreateFromByteArray(data)
}

fn map_win_error(e: windows::core::Error) -> VaultError {
    VaultError::BiometricFailed(e.message().to_string())
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

fn keyring_entry(key_id: &str) -> Result<keyring::Entry, VaultError> {
    keyring::Entry::new(KEYRING_SERVICE, key_id)
        .map_err(|e| VaultError::BiometricFailed(format!("keyring entry: {e}")))
}

impl BiometricKeyStore for WindowsHelloKeyStore {
    fn is_available(&self) -> Result<bool, VaultError> {
        let _guard = WinRtGuard::new()?;
        let supported = KeyCredentialManager::IsSupportedAsync().map_err(|e| {
            warn!("Windows Hello: IsSupportedAsync failed: {}", e.message());
            map_win_error(e)
        })?;
        let result = wait_for_async(supported).map_err(|e| {
            warn!(
                "Windows Hello: IsSupportedAsync.get() failed: {}",
                e.message()
            );
            map_win_error(e)
        })?;
        debug!("Windows Hello available: {}", result);
        Ok(result)
    }

    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        let _guard = WinRtGuard::new()?;
        let name = credential_name(key_id);

        let open_result = KeyCredentialManager::OpenAsync(&name)
            .map_err(map_win_error)?
            .join()
            .map_err(map_win_error)?;

        if get_credential_status(&open_result)? == KeyCredentialStatus::NotFound {
            allow_foreground_window();

            let create_result = KeyCredentialManager::RequestCreateAsync(
                &name,
                KeyCredentialCreationOption::FailIfExists,
            )
            .map_err(map_win_error)?;

            let create_result = wait_for_async(create_result).map_err(map_win_error)?;

            match get_credential_status(&create_result)? {
                KeyCredentialStatus::Success => {}
                KeyCredentialStatus::CredentialAlreadyExists => {}
                KeyCredentialStatus::UserCanceled => {
                    warn!("Windows Hello: biometric enrollment cancelled by user");
                    return Err(VaultError::BiometricCancelled);
                }
                status => {
                    warn!("Windows Hello: KeyCredential creation failed: {:?}", status);
                    return Err(VaultError::BiometricFailed(format!(
                        "KeyCredential creation failed: {status:?}"
                    )));
                }
            }
        }

        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let entry = keyring_entry(key_id)?;
        entry
            .set_password(&encoded)
            .map_err(|e| VaultError::BiometricFailed(format!("keyring set: {e}")))?;
        info!("Windows Hello: biometric key stored for '{}'", key_id);
        Ok(())
    }

    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        let _guard = WinRtGuard::new()?;
        let name = credential_name(key_id);

        let open_result = KeyCredentialManager::OpenAsync(&name)
            .map_err(map_win_error)?
            .join()
            .map_err(map_win_error)?;

        if get_credential_status(&open_result)? != KeyCredentialStatus::Success {
            return Err(VaultError::BiometricUnavailable);
        }

        let credential = open_result.Credential().map_err(map_win_error)?;

        let mut challenge = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut challenge);

        let challenge_buffer = bytes_to_buffer(&challenge).map_err(map_win_error)?;

        allow_foreground_window();

        let sign_result = credential
            .RequestSignAsync(&challenge_buffer)
            .map_err(map_win_error)?;

        let sign_result = wait_for_async(sign_result).map_err(map_win_error)?;

        match get_operation_status(&sign_result)? {
            KeyCredentialStatus::Success => {}
            KeyCredentialStatus::UserCanceled => {
                warn!("Windows Hello: biometric auth cancelled by user");
                return Err(VaultError::BiometricCancelled);
            }
            status => {
                warn!("Windows Hello: TPM sign failed: {:?}", status);
                return Err(VaultError::BiometricFailed(format!(
                    "TPM sign failed: {status:?}"
                )));
            }
        }

        let entry = keyring_entry(key_id)?;
        let encoded = entry
            .get_password()
            .map_err(|e| VaultError::BiometricFailed(format!("keyring get: {e}")))?;
        let data = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .map_err(|e| VaultError::BiometricFailed(format!("base64 decode: {e}")))?;
        info!("Windows Hello: biometric key retrieved for '{}'", key_id);
        Ok(data)
    }

    fn delete_key(&self, key_id: &str) -> Result<(), VaultError> {
        let _guard = WinRtGuard::new()?;
        let name = credential_name(key_id);

        let delete_op = KeyCredentialManager::DeleteAsync(&name);
        if let Ok(op) = delete_op {
            let _ = wait_for_action(op);
        }

        let entry = keyring_entry(key_id)?;
        let _ = entry.delete_credential();
        info!("Windows Hello: biometric key deleted for '{}'", key_id);
        Ok(())
    }

    fn has_key(&self, key_id: &str) -> Result<bool, VaultError> {
        let _guard = WinRtGuard::new()?;
        let name = credential_name(key_id);

        let open_result = KeyCredentialManager::OpenAsync(&name)
            .map_err(map_win_error)?
            .join()
            .map_err(map_win_error)?;

        let has_tpm = get_credential_status(&open_result)? == KeyCredentialStatus::Success;
        if !has_tpm {
            return Ok(false);
        }

        let entry = keyring_entry(key_id)?;
        match entry.get_password() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(VaultError::BiometricFailed(format!("keyring check: {e}"))),
        }
    }
}
