use std::ffi::c_void;

use windows::core::w;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_DWORD};

pub(super) const DARK_BG: u32 = 0x001E1E1E;
pub(super) const DARK_TEXT: u32 = 0x00FFFFFF;
pub(super) const DARK_BTN_FACE: u32 = 0x002D2D2D;
pub(super) const DARK_BTN_BORDER: u32 = 0x004D4D4D;
pub(super) const DARK_BTN_HOVER: u32 = 0x00504040;
pub(super) const DARK_BTN_PRESSED: u32 = 0x005C4848;

pub(super) fn is_system_dark_mode() -> bool {
    use windows::Win32::System::Registry::RegGetValueW;

    let mut buf: u32 = 0;
    let mut buf_size: u32 = std::mem::size_of::<u32>() as u32;

    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_DWORD,
            None,
            Some(&mut buf as *mut u32 as *mut c_void),
            Some(&mut buf_size),
        )
    };

    let dark = result.is_ok() && buf == 0;
    log::info!(
        "is_system_dark_mode: {} (AppsUseLightTheme={}, result={:?})",
        dark,
        buf,
        result
    );
    dark
}
