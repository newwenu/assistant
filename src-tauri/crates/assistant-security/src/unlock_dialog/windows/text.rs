use windows::core::w;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW, SendMessageW, SetWindowTextW},
};
use zeroize::Zeroize;

use crate::vault_crypto::types::SecureString;

use super::ffi::{GetDlgItem, EM_EMPTYUNDOBUFFER};

pub(super) unsafe fn read_edit_text(hedit: HWND) -> Vec<u16> {
    if hedit.is_invalid() {
        return Vec::new();
    }
    let len = GetWindowTextLengthW(hedit) as usize;
    if len == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u16; len + 1];
    let copied = GetWindowTextW(hedit, &mut buf);
    if copied == 0 {
        return Vec::new();
    }
    buf.truncate(copied as usize);
    buf
}

pub(super) fn read_edit_text_secure(hwnd: HWND, id: u16) -> SecureString {
    let hedit = unsafe { GetDlgItem(hwnd, id as i32) };
    if hedit.is_invalid() {
        return SecureString::new(String::new());
    }
    let len = unsafe { GetWindowTextLengthW(hedit) } as usize;
    if len == 0 {
        return SecureString::new(String::new());
    }
    let mut buf = vec![0u16; len + 1];
    let copied = unsafe { GetWindowTextW(hedit, &mut buf) };
    if copied == 0 {
        buf.zeroize();
        return SecureString::new(String::new());
    }
    let mut password = String::from_utf16_lossy(&buf[..copied as usize]);
    buf.zeroize();
    let secure = SecureString::new(password.clone());
    unsafe {
        let vec = password.as_mut_vec();
        vec.zeroize();
    }
    std::mem::forget(password);
    secure
}

pub(super) fn clear_edit_text(hwnd: HWND, id: u16) {
    let hedit = unsafe { GetDlgItem(hwnd, id as i32) };
    if hedit.is_invalid() {
        return;
    }
    let _ = unsafe { SetWindowTextW(hedit, w!("")) };
    let _ = unsafe {
        SendMessageW(
            hedit,
            EM_EMPTYUNDOBUFFER,
            Some(WPARAM::default()),
            Some(LPARAM::default()),
        )
    };
}
