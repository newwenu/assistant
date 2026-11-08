use std::ffi::c_void;

use windows::core::w;
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT},
    UI::WindowsAndMessaging::HMENU,
};

// ─── Control IDs & Style Constants ───────────────────────

pub(super) const ID_EDIT_PASSWORD: u16 = 101;
pub(super) const ID_BTN_UNLOCK: u16 = 201;
pub(super) const ID_BTN_CLOSE: u16 = 202;
pub(super) const ID_STATIC_TITLE: u16 = 301;

pub(super) const ES_PASSWORD: u32 = 0x0020;
pub(super) const ES_AUTOHSCROLL: u32 = 0x0080;
pub(super) const BS_DEFPUSHBUTTON: u32 = 0x0001;
pub(super) const BS_OWNERDRAW: u32 = 0x000B;
pub(super) const BS_NOTIFY: u32 = 0x0400;
pub(super) const EM_EMPTYUNDOBUFFER: u32 = 0x00CE;
pub(super) const EM_SETMARGINS: u32 = 0x00D3;
pub(super) const EM_SETPASSWORDCHAR: u32 = 0x00CC;
pub(super) const EC_RIGHTMARGIN: u32 = 0x0002;

pub(super) const DT_CENTER: u32 = 0x0001;
pub(super) const DT_VCENTER: u32 = 0x0004;
pub(super) const DT_SINGLELINE: u32 = 0x0020;
pub(super) const BKMODE_TRANSPARENT: u32 = 1;

pub(super) const WINDOW_CLASS: windows::core::PCWSTR = w!("AssistantUnlockDlg");

pub(super) const GWLP_USERDATA: i32 = -21;
pub(super) const GWL_STYLE: i32 = -16;
pub(super) const GWLP_WNDPROC: i32 = -4;
pub(super) const GWLP_HINSTANCE: i32 = -6;

pub(super) fn make_hmenu(id: u16) -> HMENU {
    HMENU(id as *mut c_void)
}

pub(super) fn get_x_lparam(lparam: LPARAM) -> i32 {
    (lparam.0 as u16) as i16 as i32
}

pub(super) fn get_y_lparam(lparam: LPARAM) -> i32 {
    ((lparam.0 >> 16) as u16) as i16 as i32
}

#[link(name = "user32")]
extern "system" {
    pub(super) fn GetWindowLongPtrW(hwnd: HWND, nindex: i32) -> isize;
    pub(super) fn SetWindowLongPtrW(hwnd: HWND, nindex: i32, newlong: isize) -> isize;
    pub(super) fn GetDlgItem(hDlg: HWND, nIDDlgItem: i32) -> HWND;
    pub(super) fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> i32;
    pub(super) fn GetCursorPos(lpPoint: *mut POINT) -> i32;
}
