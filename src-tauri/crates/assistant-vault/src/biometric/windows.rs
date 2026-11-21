use crate::biometric::BiometricKeyStore;
use assistant_crypto::error::VaultError;

use log::{debug, info, trace, warn};

use base64::Engine;
use sha2::{Digest, Sha256};

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

// HRESULT 以无符号位模式书写，转 `i32` 是有意的符号位解释。
#[allow(clippy::cast_sign_loss)]
const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106u32 as i32);

pub struct WindowsHelloKeyStore;

const SERVICE_NAME: &str = "assistant-vault-biometric";
const KEYRING_SERVICE: &str = "assistant-vault-biometric";

struct WinRtGuard {
    owned: bool,
}

impl WinRtGuard {
    fn new() -> Result<Self, VaultError> {
        match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => Ok(Self { owned: true }),
            Err(e) if e.code() == RPC_E_CHANGED_MODE => Ok(Self { owned: false }),
            Err(e) => Err(VaultError::BiometricFailed(format!("RoInitialize: {e}"))),
        }
    }
}

impl Drop for WinRtGuard {
    fn drop(&mut self) {
        if self.owned {
            unsafe { RoUninitialize() };
        }
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

fn buffer_to_bytes(buffer: &IBuffer) -> windows::core::Result<Vec<u8>> {
    let length = buffer.Length()? as usize;
    let mut array = windows::core::Array::with_len(length);
    CryptographicBuffer::CopyToByteArray(buffer, &mut array)?;
    Ok(array.to_vec())
}

// HRESULT 以无符号位模式书写，转 `i32` 是有意的符号位解释。
#[allow(clippy::cast_sign_loss)]
const ERROR_NOT_FOUND: HRESULT = HRESULT(0x80070490u32 as i32);
#[allow(clippy::cast_sign_loss)]
const ERROR_FILE_NOT_FOUND: HRESULT = HRESULT(0x80070002u32 as i32);

fn map_win_error(e: windows::core::Error) -> VaultError {
    VaultError::BiometricFailed(e.message().to_string())
}

fn is_not_found_error(e: &windows::core::Error) -> bool {
    e.code() == ERROR_NOT_FOUND || e.code() == ERROR_FILE_NOT_FOUND
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

fn biometric_challenge() -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"assistant-vault-biometric-v1");
    hasher.finalize().to_vec()
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
        trace!("Windows Hello available: {}", result);
        Ok(result)
    }

    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        let _guard = WinRtGuard::new()?;

        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let entry = keyring_entry(key_id)?;
        entry
            .set_password(&encoded)
            .map_err(|e| VaultError::BiometricFailed(format!("keyring set: {e}")))?;
        info!("Windows Hello: data stored for '{}'", key_id);
        Ok(())
    }

    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        let _guard = WinRtGuard::new()?;

        let entry = keyring_entry(key_id)?;
        let encoded = entry
            .get_password()
            .map_err(|e| VaultError::BiometricFailed(format!("keyring get: {e}")))?;
        let data = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .map_err(|e| VaultError::BiometricFailed(format!("base64 decode: {e}")))?;
        info!("Windows Hello: data retrieved for '{}'", key_id);
        Ok(data)
    }

    fn delete_key(&self, key_id: &str) -> Result<(), VaultError> {
        let _guard = WinRtGuard::new()?;
        let name = credential_name(key_id);

        let open_result = KeyCredentialManager::OpenAsync(&name)
            .map_err(map_win_error)?
            .join()
            .map_err(map_win_error)?;

        if get_credential_status(&open_result)? == KeyCredentialStatus::Success {
            match KeyCredentialManager::DeleteAsync(&name) {
                Ok(op) => match wait_for_action(op) {
                    Ok(()) => {}
                    Err(e) if is_not_found_error(&e) => {}
                    Err(e) => return Err(map_win_error(e)),
                },
                Err(e) if is_not_found_error(&e) => {}
                Err(e) => return Err(map_win_error(e)),
            }
        }

        let entry = keyring_entry(key_id)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(VaultError::BiometricFailed(format!("keyring delete: {e}"))),
        }
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

    fn ensure_credential(&self, key_id: &str) -> Result<(), VaultError> {
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

        debug!("Windows Hello: TPM credential ensured for '{}'", key_id);
        Ok(())
    }

    fn request_signature(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
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

        let challenge = biometric_challenge();
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

        let signature_buffer = sign_result.Result().map_err(map_win_error)?;
        let signature = buffer_to_bytes(&signature_buffer).map_err(map_win_error)?;

        debug!(
            "Windows Hello: TPM signature obtained ({} bytes) for '{}'",
            signature.len(),
            key_id
        );
        Ok(signature)
    }
}
