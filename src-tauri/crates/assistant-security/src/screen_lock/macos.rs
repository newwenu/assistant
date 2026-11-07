use std::sync::{Arc, Mutex};

pub(super) fn macos_listen(
    locked: Arc<Mutex<bool>>,
    on_lock: impl Fn() + Send + Sync,
    on_unlock: impl Fn() + Send + Sync,
) {
    use std::ffi::{c_void, CString};
    use std::ptr;

    type CFTypeRef = *const c_void;
    type CFIndex = i64;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGSessionCopyCurrentDictionary() -> CFTypeRef;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFStringCreateWithCString(alloc: CFTypeRef, c: *const i8, enc: u32) -> CFTypeRef;
        fn CFGetTypeID(v: CFTypeRef) -> usize;
        fn CFBooleanGetTypeID() -> usize;
        fn CFBooleanGetValue(b: CFTypeRef) -> u8;
        fn CFNumberGetTypeID() -> usize;
        fn CFNumberGetValue(n: CFTypeRef, the_type: CFIndex, out: *mut c_void) -> u8;
        fn CFRelease(v: CFTypeRef);
    }

    const KCF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const KCF_NUMBER_SINT32: CFIndex = 3;

    fn is_screen_locked() -> bool {
        unsafe {
            let dict = CGSessionCopyCurrentDictionary();
            if dict.is_null() {
                return true;
            }
            let Ok(key_c) = CString::new("CGSSessionScreenIsLocked") else {
                CFRelease(dict);
                return false;
            };
            let key =
                CFStringCreateWithCString(ptr::null(), key_c.as_ptr(), KCF_STRING_ENCODING_UTF8);
            if key.is_null() {
                CFRelease(dict);
                return false;
            }
            let value = CFDictionaryGetValue(dict, key);
            let locked = if value.is_null() {
                false
            } else {
                let tid = CFGetTypeID(value);
                if tid == CFBooleanGetTypeID() {
                    CFBooleanGetValue(value) != 0
                } else if tid == CFNumberGetTypeID() {
                    let mut n: i32 = 0;
                    CFNumberGetValue(value, KCF_NUMBER_SINT32, &mut n as *mut i32 as *mut c_void);
                    n != 0
                } else {
                    false
                }
            };
            CFRelease(key);
            CFRelease(dict);
            locked
        }
    }

    let mut last = false;
    loop {
        let now = is_screen_locked();
        if now != last {
            if now {
                *locked.lock().unwrap_or_else(|e| e.into_inner()) = true;
                on_lock();
                log::info!("ScreenLockListener: screen locked");
            } else {
                *locked.lock().unwrap_or_else(|e| e.into_inner()) = false;
                on_unlock();
                log::info!("ScreenLockListener: screen unlocked");
            }
            last = now;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
