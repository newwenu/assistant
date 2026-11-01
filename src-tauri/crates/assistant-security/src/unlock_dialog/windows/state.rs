use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{HBRUSH, HFONT},
};

use crate::unlock_dialog::UnlockResult;

use super::layout::LayoutMetrics;
use super::localization::UiStrings;

pub(super) type DialogStatePtr = *mut DialogState;

pub(super) struct DialogState {
    pub(super) on_result: Option<Box<dyn FnOnce(UnlockResult) + Send>>,
    pub(super) closed: bool,
    pub(super) password_visible: bool,
    pub(super) layout: LayoutMetrics,
    pub(super) is_dark: bool,
    pub(super) dark_brush: HBRUSH,
    pub(super) ui_strings: UiStrings,
}

pub(super) static mut ORIGINAL_EDIT_PROC: Option<
    unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
> = None;
pub(super) static mut ORIGINAL_BTN_PROC: Option<
    unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
> = None;
pub(super) static mut BTN_HOVER_UNLOCK: bool = false;
pub(super) static mut BTN_HOVER_CLOSE: bool = false;
pub(super) static mut DEFAULT_FONT: HFONT = HFONT(std::ptr::null_mut());
