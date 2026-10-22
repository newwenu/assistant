use crate::biometric::BiometricKeyStore;
use crate::types::VaultError;

use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

pub struct LibsecretKeyStore;

const SCHEMA_NAME: &str = "com.assistant.vault.biometric";

#[repr(C)]
struct SecretSchema {
    name: *const c_char,
    flags: c_int,
    attributes: [SecretSchemaAttribute; 32],
    reserved: [c_int; 2],
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

fn create_schema() -> Box<SecretSchema> {
    let c_name = CString::new(SCHEMA_NAME).expect("valid schema name");
    let c_account = CString::new("account").expect("valid attribute name");

    let mut schema = Box::new(SecretSchema {
        name: c_name.as_ptr(),
        flags: SECRET_SCHEMA_DONT_MATCH_NAME,
        attributes: [SecretSchemaAttribute {
            name: ptr::null(),
            r#type: 0,
        }; 32],
        reserved: [0; 2],
    });

    schema.attributes[0].name = c_account.as_ptr();
    schema.attributes[0].r#type = SECRET_SCHEMA_ATTRIBUTE_STRING;

    std::mem::forget(c_name);
    std::mem::forget(c_account);

    schema
}

impl BiometricKeyStore for LibsecretKeyStore {
    fn is_available(&self) -> Result<bool, VaultError> {
        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let service = secret_service_get_sync(SECRET_SERVICE_NONE, ptr::null(), &mut error);
            if !error.is_null() {
                g_free(error as *mut c_void);
                return Ok(false);
            }
            if service.is_null() {
                return Ok(false);
            }
            g_object_unref(service);
            Ok(true)
        }
    }

    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        let schema = create_schema();
        let c_label = CString::new(key_id)
            .map_err(|_| VaultError::BiometricFailed("invalid key_id".into()))?;
        let c_password = CString::new(data)
            .map_err(|_| VaultError::BiometricFailed("data contains null byte".into()))?;
        let c_account_key = CString::new("account")
            .map_err(|_| VaultError::BiometricFailed("invalid attribute key".into()))?;
        let c_account_val = CString::new(key_id)
            .map_err(|_| VaultError::BiometricFailed("invalid key_id".into()))?;

        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let result = secret_password_store_sync(
                &*schema,
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
                return Err(VaultError::BiometricFailed(
                    msg.into_string()
                        .unwrap_or_else(|_| "libsecret error".into()),
                ));
            }

            if result == 0 {
                return Err(VaultError::BiometricFailed(
                    "secret_password_store_sync failed".into(),
                ));
            }
            Ok(())
        }
    }

    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        let schema = create_schema();
        let c_account_key = CString::new("account")
            .map_err(|_| VaultError::BiometricFailed("invalid attribute key".into()))?;
        let c_account_val = CString::new(key_id)
            .map_err(|_| VaultError::BiometricFailed("invalid key_id".into()))?;

        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let result = secret_password_lookup_sync(
                &*schema,
                ptr::null(),
                &mut error,
                c_account_key.as_ptr(),
                c_account_val.as_ptr(),
                ptr::null(),
            );

            if !error.is_null() {
                let msg = CString::from_raw((*error).message);
                g_free(error as *mut c_void);
                return Err(VaultError::BiometricFailed(
                    msg.into_string()
                        .unwrap_or_else(|_| "libsecret error".into()),
                ));
            }

            if result.is_null() {
                return Err(VaultError::BiometricUnavailable);
            }

            let c_str = CString::from_raw(result);
            let bytes = c_str.into_bytes();
            Ok(bytes)
        }
    }

    fn delete_key(&self, key_id: &str) -> Result<(), VaultError> {
        let schema = create_schema();
        let c_account_key = CString::new("account")
            .map_err(|_| VaultError::BiometricFailed("invalid attribute key".into()))?;
        let c_account_val = CString::new(key_id)
            .map_err(|_| VaultError::BiometricFailed("invalid key_id".into()))?;

        unsafe {
            let mut error: *mut GError = ptr::null_mut();
            let _ = secret_password_clear_sync(
                &*schema,
                ptr::null(),
                &mut error,
                c_account_key.as_ptr(),
                c_account_val.as_ptr(),
                ptr::null(),
            );

            if !error.is_null() {
                g_free(error as *mut c_void);
            }
            Ok(())
        }
    }

    fn has_key(&self, key_id: &str) -> Result<bool, VaultError> {
        match self.get_key(key_id) {
            Ok(_) => Ok(true),
            Err(VaultError::BiometricUnavailable) => Ok(false),
            Err(e) => Err(e),
        }
    }
}
