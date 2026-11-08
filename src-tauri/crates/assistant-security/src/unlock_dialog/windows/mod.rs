use crate::unlock_dialog::{UnlockDialog, UnlockResult};

use std::ffi::c_void;

use windows::Win32::{
    Graphics::Gdi::HBRUSH,
    System::LibraryLoader::GetModuleHandleW,
    UI::HiDpi::GetDpiForSystem,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, GetSystemMetrics, IsDialogMessageW,
        RegisterClassW, TranslateMessage, SM_CXSCREEN, SM_CYSCREEN, WNDCLASSW, WS_CAPTION,
        WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_EX_TOPMOST, WS_OVERLAPPED, WS_SYSMENU,
        WS_VISIBLE,
    },
};

mod ffi;
mod layout;
mod localization;
mod paint;
mod state;
mod subclass;
mod text;
mod theme;
mod wm_handlers;
mod wnd_proc;

use ffi::{GetWindowLongPtrW, GWLP_USERDATA, WINDOW_CLASS};
use layout::{calc_window_rect, LayoutMetrics};
use localization::{get_ui_strings, str_to_wide};
use state::{DialogState, DialogStatePtr};
use theme::is_system_dark_mode;
use wnd_proc::wnd_proc;

// ─── Dialog Entry Point ──────────────────────────────────

pub struct Win32UnlockDialog {
    hinstance: usize,
}

unsafe impl Send for Win32UnlockDialog {}
unsafe impl Sync for Win32UnlockDialog {}

impl Win32UnlockDialog {
    pub fn new() -> Result<Self, assistant_crypto::error::VaultError> {
        let instance = unsafe { GetModuleHandleW(None) }.map_err(|e| {
            assistant_crypto::error::VaultError::Other(format!("GetModuleHandleW: {e}"))
        })?;
        Ok(Self {
            hinstance: instance.0 as usize,
        })
    }
}

impl UnlockDialog for Win32UnlockDialog {
    fn show(
        &self,
        on_result: Box<dyn FnOnce(UnlockResult) + Send>,
    ) -> Result<(), assistant_crypto::error::VaultError> {
        let hinstance_val = self.hinstance;

        let hinstance = windows::Win32::Foundation::HINSTANCE(hinstance_val as *mut c_void);
        let class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            lpszClassName: WINDOW_CLASS,
            ..Default::default()
        };
        unsafe { RegisterClassW(&class) };

        let dpi = unsafe { GetDpiForSystem() };
        let layout = LayoutMetrics::new(dpi);
        let is_dark = is_system_dark_mode();

        let ui_strings = get_ui_strings();

        let state: DialogStatePtr = Box::into_raw(Box::new(DialogState {
            on_result: Some(on_result),
            closed: false,
            password_visible: false,
            layout,
            is_dark,
            dark_brush: HBRUSH(std::ptr::null_mut()),
            ui_strings,
        }));
        let state_raw = state as usize;

        std::thread::spawn(move || {
            let hinstance = windows::Win32::Foundation::HINSTANCE(hinstance_val as *mut c_void);
            let state = state_raw as DialogStatePtr;

            let win_rect = calc_window_rect(dpi);
            let win_w = win_rect.right - win_rect.left;
            let win_h = win_rect.bottom - win_rect.top;

            let (x, y) = unsafe {
                let cx = GetSystemMetrics(SM_CXSCREEN);
                let cy = GetSystemMetrics(SM_CYSCREEN);
                (
                    (cx - win_w) / 2 - win_rect.left,
                    (cy - win_h) / 2 - win_rect.top,
                )
            };

            let title_wide = unsafe { str_to_wide((*state).ui_strings.title) };

            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_DLGMODALFRAME | WS_EX_TOPMOST | WS_EX_CONTROLPARENT,
                    WINDOW_CLASS,
                    windows::core::PCWSTR(title_wide.as_ptr()),
                    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
                    x,
                    y,
                    win_w,
                    win_h,
                    None,
                    None,
                    Some(hinstance),
                    Some(state as *mut c_void),
                )
            };

            let hwnd = match hwnd {
                Ok(h) => h,
                Err(e) => {
                    log::error!("CreateWindowExW failed: {}", e);
                    unsafe { drop(Box::from_raw(state)) };
                    return;
                }
            };

            unsafe {
                let mut msg = std::mem::zeroed();
                loop {
                    let ret = GetMessageW(&mut msg, None, 0, 0).0;
                    if ret == 0 || ret == -1 {
                        break;
                    }
                    if !IsDialogMessageW(hwnd, &msg).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }

            unsafe {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
                if !state_ptr.is_null() {
                    let mut state = Box::from_raw(state_ptr);
                    if let Some(cb) = state.on_result.take() {
                        if !state.closed {
                            cb(UnlockResult::Cancelled);
                        }
                    }
                }
            }
        });

        Ok(())
    }
}
