use crate::biometric::BiometricKeyStore;
use crate::types::VaultError;

use core_foundation::base::{CFRelease, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use security_framework::item::{ItemClass, ItemSearchOptions, ReferenceClass, SearchResult};
use security_framework::keychain_item::KeychainItem;

pub struct TouchIdKeyStore;

const SERVICE_NAME: &str = "assistant-vault-biometric";

fn item_search_options() -> ItemSearchOptions {
    ItemSearchOptions::new()
        .class(ItemClass::GenericPassword)
        .service(SERVICE_NAME)
}

impl BiometricKeyStore for TouchIdKeyStore {
    fn is_available(&self) -> Result<bool, VaultError> {
        let access_control = unsafe {
            SecAccessControlCreateWithFlags(
                kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
                kSecAccessControlBiometryCurrentSet,
                std::ptr::null_mut(),
            )
        };
        Ok(!access_control.is_null())
    }

    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        let access_control = unsafe {
            SecAccessControlCreateWithFlags(
                kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
                kSecAccessControlBiometryCurrentSet,
                std::ptr::null_mut(),
            )
        };
        if access_control.is_null() {
            return Err(VaultError::BiometricUnavailable);
        }

        let existing = item_search_options().account(key_id).load_data().search();
        if let Ok(results) = existing {
            for result in results {
                if let SearchResult::Data(_) = result {
                    let delete_result = item_search_options().account(key_id).delete();
                    if let Err(e) = delete_result {
                        return Err(map_security_error(&e));
                    }
                }
            }
        }

        let cf_service = CFString::new(SERVICE_NAME);
        let cf_account = CFString::new(key_id);
        let cf_data = CFData::from_buffer(data);
        let cf_synchronizable = CFBoolean::from(false);

        let mut keys: Vec<*const c_void> = Vec::new();
        let mut values: Vec<*const c_void> = Vec::new();

        unsafe {
            keys.push(kSecClass as *const c_void);
            values.push(kSecClassGenericPassword as *const c_void);

            keys.push(kSecAttrService as *const c_void);
            values.push(cf_service.as_CFTypeRef() as *const c_void);

            keys.push(kSecAttrAccount as *const c_void);
            values.push(cf_account.as_CFTypeRef() as *const c_void);

            keys.push(kSecValueData as *const c_void);
            values.push(cf_data.as_CFTypeRef() as *const c_void);

            keys.push(kSecAttrAccessControl as *const c_void);
            values.push(access_control as *const c_void);

            keys.push(kSecAttrSynchronizable as *const c_void);
            values.push(cf_synchronizable.as_CFTypeRef() as *const c_void);

            let query = CFDictionary::from_keys_and_values(
                keys.as_ptr() as *const *const c_void,
                values.as_ptr() as *const *const c_void,
                keys.len() as isize,
            );

            let status = SecItemAdd(query.as_CFTypeRef() as *const c_void, std::ptr::null_mut());

            if status == errSecSuccess {
                Ok(())
            } else if status == errSecUserCanceled {
                Err(VaultError::BiometricCancelled)
            } else if status == errSecDuplicateItem {
                Err(VaultError::BiometricFailed(
                    "duplicate item after delete".into(),
                ))
            } else {
                Err(VaultError::BiometricFailed(format!("SecItemAdd: {status}")))
            }
        }
    }

    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        let results = item_search_options()
            .account(key_id)
            .load_data()
            .search()
            .map_err(|e| map_search_error(&e, key_id))?;

        match results.first() {
            Some(SearchResult::Data(data)) => Ok(data.to_vec()),
            Some(SearchResult::Ref(_)) => {
                Err(VaultError::BiometricFailed("unexpected ref result".into()))
            }
            None => Err(VaultError::BiometricUnavailable),
        }
    }

    fn delete_key(&self, key_id: &str) -> Result<(), VaultError> {
        let result = item_search_options().account(key_id).delete();

        match result {
            Ok(()) => Ok(()),
            Err(ref e) if is_item_not_found(e) => Ok(()),
            Err(e) => Err(map_security_error(&e)),
        }
    }

    fn has_key(&self, key_id: &str) -> Result<bool, VaultError> {
        let results = item_search_options().account(key_id).limit(1).search();

        match results {
            Ok(r) => Ok(!r.is_empty()),
            Err(_) => Ok(false),
        }
    }
}

fn map_security_error(e: &security_framework::Error) -> VaultError {
    match e.code() {
        -25300 => VaultError::BiometricUnavailable,
        -128 => VaultError::BiometricCancelled,
        -25293 => VaultError::BiometricFailed("auth failed".into()),
        code => VaultError::BiometricFailed(format!("Security framework error: {code}")),
    }
}

fn map_search_error(e: &security_framework::Error, _key_id: &str) -> VaultError {
    match e.code() {
        -25300 => VaultError::BiometricUnavailable,
        -128 => VaultError::BiometricCancelled,
        -25293 => VaultError::BiometricFailed("auth failed".into()),
        code => VaultError::BiometricFailed(format!("search error: {code}")),
    }
}

fn is_item_not_found(e: &security_framework::Error) -> bool {
    e.code() == -25300
}

use std::os::raw::c_void;

const errSecSuccess: i32 = 0;
const errSecDuplicateItem: i32 = -25299;
const errSecUserCanceled: i32 = -128;

extern "C" {
    static kSecClass: *const c_void;
    static kSecClassGenericPassword: *const c_void;
    static kSecAttrService: *const c_void;
    static kSecAttrAccount: *const c_void;
    static kSecValueData: *const c_void;
    static kSecAttrAccessControl: *const c_void;
    static kSecAttrSynchronizable: *const c_void;
    static kSecAttrAccessibleWhenUnlockedThisDeviceOnly: *const c_void;
    static kSecAccessControlBiometryCurrentSet: u32;

    fn SecItemAdd(query: *const c_void, result: *mut c_void) -> i32;
    fn SecAccessControlCreateWithFlags(
        protection: *const c_void,
        flags: u32,
        error: *mut c_void,
    ) -> *const c_void;
}
