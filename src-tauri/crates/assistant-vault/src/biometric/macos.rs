use crate::biometric::BiometricKeyStore;
use assistant_crypto::error::VaultError;

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFMutableDictionary;
use core_foundation::string::CFString;
use log::{debug, info, warn};
use security_framework::access_control::{ProtectionMode, SecAccessControl};
use security_framework::base::Error as SecurityError;
use security_framework::item::{ItemClass, ItemSearchOptions, SearchResult};
use security_framework_sys::access_control::{
    kSecAccessControlBiometryCurrentSet, kSecAccessControlUserPresence,
};
use security_framework_sys::base::{errSecDuplicateItem, errSecSuccess};

pub struct TouchIdKeyStore;

const SERVICE_NAME: &str = "assistant-vault-biometric";

fn item_search_options() -> ItemSearchOptions {
    let mut options = ItemSearchOptions::new();
    options
        .class(ItemClass::generic_password())
        .service(SERVICE_NAME);
    options
}

impl BiometricKeyStore for TouchIdKeyStore {
    fn is_available(&self) -> Result<bool, VaultError> {
        // 只探测能否创建生物识别 ACL 对象；对象随作用域自动释放。
        let available = SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
            kSecAccessControlBiometryCurrentSet,
        )
        .is_ok();
        debug!("Touch ID available: {}", available);
        Ok(available)
    }

    fn acl_satisfied(&self) -> Result<bool, VaultError> {
        if SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
            kSecAccessControlUserPresence,
        )
        .is_err()
        {
            warn!("Touch ID ACL (user presence) unsatisfied; biometric disabled");
            return Ok(false);
        }
        Ok(true)
    }

    fn set_key(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        let access_control = SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
            kSecAccessControlBiometryCurrentSet,
        )
        .map_err(|_| VaultError::BiometricUnavailable)?;

        let existing = item_search_options()
            .account(key_id)
            .load_data(true)
            .search();
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

        let mut query = CFMutableDictionary::<*const c_void, *const c_void>::new();

        unsafe {
            query.set(kSecClass, kSecClassGenericPassword);
            query.set(kSecAttrService, cf_service.as_CFTypeRef());
            query.set(kSecAttrAccount, cf_account.as_CFTypeRef());
            query.set(kSecValueData, cf_data.as_CFTypeRef());
            query.set(kSecAttrAccessControl, access_control.as_CFTypeRef());
            query.set(kSecAttrSynchronizable, cf_synchronizable.as_CFTypeRef());

            let status = SecItemAdd(query.as_CFTypeRef(), std::ptr::null_mut());

            if status == errSecSuccess {
                info!("Touch ID: biometric key stored for '{}'", key_id);
                Ok(())
            } else if status == ERR_SEC_USER_CANCELED {
                warn!("Touch ID: biometric enrollment cancelled by user");
                Err(VaultError::BiometricCancelled)
            } else if status == errSecDuplicateItem {
                warn!("Touch ID: duplicate item after delete for '{}'", key_id);
                Err(VaultError::BiometricFailed(
                    "duplicate item after delete".into(),
                ))
            } else {
                warn!("Touch ID: SecItemAdd failed with status {}", status);
                Err(VaultError::BiometricFailed(format!("SecItemAdd: {status}")))
            }
        }
    }

    fn get_key(&self, key_id: &str) -> Result<Vec<u8>, VaultError> {
        let results = item_search_options()
            .account(key_id)
            .load_data(true)
            .search()
            .map_err(|e| map_search_error(&e, key_id))?;

        match results.first() {
            Some(SearchResult::Data(data)) => {
                info!("Touch ID: biometric key retrieved for '{}'", key_id);
                Ok(data.to_vec())
            }
            Some(_) => {
                warn!("Touch ID: unexpected search result for '{}'", key_id);
                Err(VaultError::BiometricFailed(
                    "unexpected search result".into(),
                ))
            }
            None => Err(VaultError::BiometricUnavailable),
        }
    }

    fn delete_key(&self, key_id: &str) -> Result<(), VaultError> {
        let result = item_search_options().account(key_id).delete();

        match result {
            Ok(()) => {
                info!("Touch ID: biometric key deleted for '{}'", key_id);
                Ok(())
            }
            Err(ref e) if is_item_not_found(e) => Ok(()),
            Err(e) => Err(map_security_error(&e)),
        }
    }

    fn has_key(&self, key_id: &str) -> Result<bool, VaultError> {
        // 只取属性（不取密文），避免触发 Touch ID；未设置 kSecReturn* 时
        // SecItemCopyMatching 返回 NULL，crate 会折成空结果，语义就丢了。
        let results = item_search_options()
            .account(key_id)
            .load_attributes(true)
            .limit(1)
            .search();

        match results {
            Ok(r) => Ok(!r.is_empty()),
            Err(_) => Ok(false),
        }
    }

    fn set_key_unprotected(&self, key_id: &str, data: &[u8]) -> Result<(), VaultError> {
        let existing = item_search_options()
            .account(key_id)
            .load_data(true)
            .search();
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

        let mut query = CFMutableDictionary::<*const c_void, *const c_void>::new();

        unsafe {
            query.set(kSecClass, kSecClassGenericPassword);
            query.set(kSecAttrService, cf_service.as_CFTypeRef());
            query.set(kSecAttrAccount, cf_account.as_CFTypeRef());
            query.set(kSecValueData, cf_data.as_CFTypeRef());
            query.set(
                kSecAttrAccessible,
                kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            );
            query.set(kSecAttrSynchronizable, cf_synchronizable.as_CFTypeRef());

            let status = SecItemAdd(query.as_CFTypeRef(), std::ptr::null_mut());

            if status == errSecSuccess {
                info!("Touch ID: data stored (unprotected) for '{}'", key_id);
                Ok(())
            } else if status == errSecDuplicateItem {
                warn!("Touch ID: duplicate item after delete for '{}'", key_id);
                Err(VaultError::BiometricFailed(
                    "duplicate item after delete".into(),
                ))
            } else {
                warn!("Touch ID: SecItemAdd failed with status {}", status);
                Err(VaultError::BiometricFailed(format!("SecItemAdd: {status}")))
            }
        }
    }
}

fn map_security_error(e: &SecurityError) -> VaultError {
    match e.code() {
        -25300 => VaultError::BiometricUnavailable,
        -128 => VaultError::BiometricCancelled,
        -25293 => VaultError::BiometricFailed("auth failed".into()),
        code => VaultError::BiometricFailed(format!("Security framework error: {code}")),
    }
}

fn map_search_error(e: &SecurityError, _key_id: &str) -> VaultError {
    match e.code() {
        -25300 => VaultError::BiometricUnavailable,
        -128 => VaultError::BiometricCancelled,
        -25293 => VaultError::BiometricFailed("auth failed".into()),
        code => VaultError::BiometricFailed(format!("search error: {code}")),
    }
}

fn is_item_not_found(e: &SecurityError) -> bool {
    e.code() == -25300
}

use std::os::raw::c_void;

// `errSecUserCanceled` 未在 security-framework-sys 中导出，按 Security 框架定义取 -128。
const ERR_SEC_USER_CANCELED: i32 = -128;

extern "C" {
    static kSecClass: *const c_void;
    static kSecClassGenericPassword: *const c_void;
    static kSecAttrService: *const c_void;
    static kSecAttrAccount: *const c_void;
    static kSecValueData: *const c_void;
    static kSecAttrAccessControl: *const c_void;
    static kSecAttrAccessible: *const c_void;
    static kSecAttrSynchronizable: *const c_void;
    static kSecAttrAccessibleWhenUnlockedThisDeviceOnly: *const c_void;

    fn SecItemAdd(query: *const c_void, result: *mut *const c_void) -> i32;
}
