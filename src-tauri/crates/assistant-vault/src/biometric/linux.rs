use crate::biometric::BiometricKeyStore;
use assistant_crypto::error::VaultError;

use base64::Engine;
use log::{debug, info, warn};
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

pub struct LibsecretKeyStore;

#[repr(C)]
struct SecretSchema {
    name: *const c_char,
    flags: c_int,
    attributes: [SecretSchemaAttribute; 32],
    reserved: [c_int; 2],
    _name: CString,
    _account: CString,
}

#[repr(C)]
struct SecretSchemaAttribute {
    name: *const c_char,
    r#type: c_int,
}

const SECRET_SCHEMA_DONT_MATCH_NAME: c_int = 2;
const SECRET_SCHEMA_ATTRIBUTE_STRING: c_int = 0;
const SECRET_COLLECTION_DEFAULT: *const c_char = ptr::null();
const SECRET_SERVICE_NONE: c_int = 0;

extern "C" {
    fn secret_password_store_sync(
        schema: *const SecretSchema,
        collection: *const c_char,
        label: *const c_char,
        password: *const c_char,
        cancellable: *const c_void,
        error: *mut *mut GError,
        ...
    ) -> c_int;

    fn secret_password_lookup_sync(
        schema: *const SecretSchema,
        cancellable: *const c_void,
        error: *mut *mut GError,
        ...
    ) -> *mut c_char;

    fn secret_password_clear_sync(
        schema: *const SecretSchema,
        cancellable: *const c_void,
        error: *mut *mut GError,
        ...
    ) -> c_int;

    fn secret_service_get_sync(
        flags: c_int,
        cancellable: *const c_void,
        error: *mut *mut GError,
    ) -> *mut c_void;

    fn g_object_unref(object: *mut c_void);
    fn g_free(mem: *mut c_void);
}

#[repr(C)]
struct GError {
    domain: u32,
    code: c_int,
    message: *mut c_char,
}

static SCHEMA_NAME_C: &str = "com.assistant.vault.biometric";
static ATTR_ACCOUNT_C: &str = "account";

fn get_schema() -> &'static SecretSchema {
    use std::sync::OnceLock;
    static SCHEMA: OnceLock<SecretSchema> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let c_name = CString::new(SCHEMA_NAME_C).expect("valid schema name");
        let c_account = CString::new(ATTR_ACCOUNT_C).expect("valid attribute name");
        SecretSchema {
            name: c_name.as_ptr(),
            flags: SECRET_SCHEMA_DONT_MATCH_NAME,
            attributes: {
                let mut attrs = [SecretSchemaAttribute {
                    name: ptr::null(),
                    r#type: 0,
                }; 32];
                attrs[0].name = c_account.as_ptr();
                attrs[0].r#type = SECRET_SCHEMA_ATTRIBUTE_STRING;
                attrs
            },
            reserved: [0; 2],
            _name: c_name,
            _account: c_account,
        }
    })
}

impl BiometricKeyStore for LibsecretKeyStore {
    fn is_available(&self) -> Result<bool, VaultError> {
        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let service = secret_service_get_sync(SECRET_SERVICE_NONE, ptr::null(), &mut error);
            if !error.is_null() {
                g_free(error as *mut c_void);
                debug!("libsecret: service not available (error)");
                return Ok(false);
            }
            if service.is_null() {
                debug!("libsecret: service not available (null)");
                return Ok(false);
            }
            g_object_unref(service);
            debug!("libsecret: service available");
            Ok(true)
        }
    }

    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        let schema = get_schema();
        let c_label = CString::new(key_id)
            .map_err(|_| VaultError::BiometricFailed("invalid key_id".into()))?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        let c_password = CString::new(encoded)
            .map_err(|_| VaultError::BiometricFailed("base64 encode failed".into()))?;
        let c_account_key = CString::new("account")
            .map_err(|_| VaultError::BiometricFailed("invalid attribute key".into()))?;
        let c_account_val = CString::new(key_id)
            .map_err(|_| VaultError::BiometricFailed("invalid key_id".into()))?;

        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let result = secret_password_store_sync(
                schema,
                SECRET_COLLECTION_DEFAULT,
                c_label.as_ptr(),
                c_password.as_ptr(),
                ptr::null(),
                &mut error,
                c_account_key.as_ptr(),
                c_account_val.as_ptr(),
                ptr::null(),
            );

            if !error.is_null() {
                let msg = CString::from_raw((*error).message);
                g_free(error as *mut c_void);
                warn!(
                    "libsecret: store failed for '{}': {}",
                    key_id,
                    msg.into_string().unwrap_or_else(|_| "unknown".into())
                );
                return Err(VaultError::BiometricFailed(
                    msg.into_string()
                        .unwrap_or_else(|_| "libsecret error".into()),
                ));
            }

            if result == 0 {
                warn!(
                    "libsecret: secret_password_store_sync failed for '{}'",
                    key_id
                );
                return Err(VaultError::BiometricFailed(
                    "secret_password_store_sync failed".into(),
                ));
            }
            info!("libsecret: biometric key stored for '{}'", key_id);
            Ok(())
        }
    }

    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        let schema = get_schema();
        let c_account_key = CString::new("account")
            .map_err(|_| VaultError::BiometricFailed("invalid attribute key".into()))?;
        let c_account_val = CString::new(key_id)
            .map_err(|_| VaultError::BiometricFailed("invalid key_id".into()))?;

        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let result = secret_password_lookup_sync(
                schema,
                ptr::null(),
                &mut error,
                c_account_key.as_ptr(),
                c_account_val.as_ptr(),
                ptr::null(),
            );

            if !error.is_null() {
                let msg = CString::from_raw((*error).message);
                g_free(error as *mut c_void);
                warn!(
                    "libsecret: lookup failed for '{}': {}",
                    key_id,
                    msg.into_string().unwrap_or_else(|_| "unknown".into())
                );
                return Err(VaultError::BiometricFailed(
                    msg.into_string()
                        .unwrap_or_else(|_| "libsecret error".into()),
                ));
            }

            if result.is_null() {
                return Err(VaultError::BiometricUnavailable);
            }

            let c_str = CString::from_raw(result);
            let encoded = c_str.into_string().map_err(|e| {
                VaultError::BiometricFailed(format!("libsecret: non-utf8 value: {e}"))
            })?;
            let data = base64::engine::general_purpose::STANDARD
                .decode(&encoded)
                .map_err(|e| VaultError::BiometricFailed(format!("base64 decode: {e}")))?;
            info!("libsecret: biometric key retrieved for '{}'", key_id);
            Ok(data)
        }
    }

    fn delete_key(&self, key_id: &str) -> Result<(), VaultError> {
        let schema = get_schema();
        let c_account_key = CString::new("account")
            .map_err(|_| VaultError::BiometricFailed("invalid attribute key".into()))?;
        let c_account_val = CString::new(key_id)
            .map_err(|_| VaultError::BiometricFailed("invalid key_id".into()))?;

        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let _ = secret_password_clear_sync(
                schema,
                ptr::null(),
                &mut error,
                c_account_key.as_ptr(),
                c_account_val.as_ptr(),
                ptr::null(),
            );

            if !error.is_null() {
                g_free(error as *mut c_void);
            }
            info!("libsecret: biometric key deleted for '{}'", key_id);
            Ok(())
        }
    }

    fn has_key(&self, key_id: &str) -> Result<bool, VaultError> {
        let schema = get_schema();
        let c_account_key = CString::new("account")
            .map_err(|_| VaultError::BiometricFailed("invalid attribute key".into()))?;
        let c_account_val = CString::new(key_id)
            .map_err(|_| VaultError::BiometricFailed("invalid key_id".into()))?;

        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let result = secret_password_lookup_sync(
                schema,
                ptr::null(),
                &mut error,
                c_account_key.as_ptr(),
                c_account_val.as_ptr(),
                ptr::null(),
            );

            if !error.is_null() {
                g_free(error as *mut c_void);
                return Ok(false);
            }

            if result.is_null() {
                return Ok(false);
            }

            g_free(result as *mut c_void);
            Ok(true)
        }
    }
}
