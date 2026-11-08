use std::ffi::c_void;

use windows::core::w;
use windows::Win32::{
    Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
    Graphics::{
        Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE},
        Gdi::{CreateSolidBrush, DeleteObject, InvalidateRect, HBRUSH, HFONT},
    },
    UI::HiDpi::GetDpiForSystem,
    UI::Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_ESCAPE, VK_RETURN},
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, PostMessageW, PostQuitMessage,
        SendMessageW, UnregisterClassW, CREATESTRUCTW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
        WM_COMMAND, WM_SETFONT, WS_BORDER, WS_CHILD, WS_CLIPSIBLINGS, WS_OVERLAPPED, WS_TABSTOP,
        WS_VISIBLE,
    },
};

use crate::unlock_dialog::UnlockResult;

use super::ffi::{
    make_hmenu, GetDlgItem, GetWindowLongPtrW, SetWindowLongPtrW, BS_DEFPUSHBUTTON, BS_NOTIFY,
    BS_OWNERDRAW, EC_RIGHTMARGIN, EM_SETMARGINS, ES_AUTOHSCROLL, ES_PASSWORD, GWLP_HINSTANCE,
    GWLP_USERDATA, GWLP_WNDPROC, ID_BTN_CLOSE, ID_BTN_UNLOCK, ID_EDIT_PASSWORD, ID_STATIC_TITLE,
    WINDOW_CLASS,
};
use super::layout::{create_default_font, LayoutMetrics};
use super::localization::{get_ui_strings, str_to_wide};
use super::state::{
    DialogStatePtr, BTN_HOVER_CLOSE, BTN_HOVER_UNLOCK, DEFAULT_FONT, ORIGINAL_BTN_PROC,
    ORIGINAL_EDIT_PROC,
};
use super::subclass::{btn_subclass_proc, edit_subclass_proc};
use super::text::{clear_edit_text, read_edit_text_secure};
use super::theme::DARK_BG;

// ─── WM_NCCREATE ─────────────────────────────────────────

pub(super) unsafe fn on_nc_create(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let create = lparam.0 as *const CREATESTRUCTW;
    if !create.is_null() {
        let state = (*create).lpCreateParams as DialogStatePtr;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

// ─── WM_CREATE ───────────────────────────────────────────

pub(super) unsafe fn on_create(hwnd: HWND, _msg: u32, _wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let hinstance = {
        let create = lparam.0 as *const CREATESTRUCTW;
        if !create.is_null() {
            (*create).hInstance
        } else {
            windows::Win32::Foundation::HINSTANCE::default()
        }
    };

    let dpi = GetDpiForSystem();
    let m = {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
        if !state_ptr.is_null() {
            &(*state_ptr).layout
        } else {
            &LayoutMetrics::new(dpi)
        }
    };

    let hfont = create_default_font(dpi);
    DEFAULT_FONT = hfont;

    let is_dark = {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
        if !state_ptr.is_null() {
            (*state_ptr).is_dark
        } else {
            false
        }
    };

    if is_dark {
        let value: i32 = 1;
        let hr = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &value as *const i32 as *mut c_void,
            std::mem::size_of::<i32>() as u32,
        );
        if let Err(e) = hr {
            log::warn!(
                "DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE) failed: {}",
                e
            );
        }
    }

    let ui = {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
        if !state_ptr.is_null() {
            &(*state_ptr).ui_strings
        } else {
            &get_ui_strings()
        }
    };

    let label_wide = str_to_wide(ui.label);
    let unlock_wide = str_to_wide(ui.unlock);
    let close_wide = str_to_wide(ui.close);

    let _title = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        windows::core::PCWSTR(label_wide.as_ptr()),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        m.margin,
        m.title_y,
        m.content_w,
        m.title_h,
        Some(hwnd),
        Some(make_hmenu(ID_STATIC_TITLE)),
        Some(hinstance),
        None,
    );
    if let Ok(h) = _title {
        let _ = PostMessageW(Some(h), WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
    }

    let _edit = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("EDIT"),
        w!(""),
        WS_CHILD
            | WS_VISIBLE
            | WS_BORDER
            | WS_CLIPSIBLINGS
            | WS_TABSTOP
            | WS_OVERLAPPED
            | WINDOW_STYLE(ES_PASSWORD)
            | WINDOW_STYLE(ES_AUTOHSCROLL),
        m.margin,
        m.edit_y,
        m.content_w,
        m.edit_h,
        Some(hwnd),
        Some(make_hmenu(ID_EDIT_PASSWORD)),
        Some(hinstance),
        None,
    );
    if let Ok(hedit) = _edit {
        let _ = PostMessageW(Some(hedit), WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
        let _ = SendMessageW(
            hedit,
            EM_SETMARGINS,
            Some(WPARAM(EC_RIGHTMARGIN as usize)),
            Some(LPARAM(m.icon_margin as isize)),
        );
        let orig_proc = SetWindowLongPtrW(
            hedit,
            GWLP_WNDPROC,
            edit_subclass_proc as *const () as isize,
        );
        ORIGINAL_EDIT_PROC = Some(std::mem::transmute::<
            isize,
            unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
        >(orig_proc));
    }

    let btn_unlock_style = if is_dark {
        WS_CHILD
            | WS_VISIBLE
            | WS_TABSTOP
            | WINDOW_STYLE(BS_DEFPUSHBUTTON | BS_OWNERDRAW | BS_NOTIFY)
    } else {
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON)
    };
    let _btn_unlock = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        windows::core::PCWSTR(unlock_wide.as_ptr()),
        btn_unlock_style,
        m.unlock_btn_x,
        m.btn_y,
        m.btn_w,
        m.btn_h,
        Some(hwnd),
        Some(make_hmenu(ID_BTN_UNLOCK)),
        Some(hinstance),
        None,
    );
    if let Ok(h) = _btn_unlock {
        let _ = PostMessageW(Some(h), WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
        if is_dark {
            SetWindowLongPtrW(h, GWLP_USERDATA, ID_BTN_UNLOCK as isize);
            let orig = SetWindowLongPtrW(h, GWLP_WNDPROC, btn_subclass_proc as *const () as isize);
            ORIGINAL_BTN_PROC = Some(std::mem::transmute::<
                isize,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >(orig));
        }
    }

    let btn_close_style = if is_dark {
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW | BS_NOTIFY)
    } else {
        WS_CHILD | WS_VISIBLE | WS_TABSTOP
    };
    let _btn_close = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        windows::core::PCWSTR(close_wide.as_ptr()),
        btn_close_style,
        m.close_btn_x,
        m.btn_y,
        m.btn_w,
        m.btn_h,
        Some(hwnd),
        Some(make_hmenu(ID_BTN_CLOSE)),
        Some(hinstance),
        None,
    );
    if let Ok(h) = _btn_close {
        let _ = PostMessageW(Some(h), WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
        if is_dark {
            SetWindowLongPtrW(h, GWLP_USERDATA, ID_BTN_CLOSE as isize);
            let orig = SetWindowLongPtrW(h, GWLP_WNDPROC, btn_subclass_proc as *const () as isize);
            ORIGINAL_BTN_PROC = Some(std::mem::transmute::<
                isize,
                unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            >(orig));
        }
    }

    if is_dark {
        let brush = CreateSolidBrush(COLORREF(DARK_BG));
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
        if !state_ptr.is_null() {
            (*state_ptr).dark_brush = brush;
        }
        let _ = InvalidateRect(Some(hwnd), None, true);
    }

    LRESULT(0)
}

// ─── WM_COMMAND / Keyboard ───────────────────────────────

pub(super) unsafe fn on_command(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let id = (wparam.0 & 0xFFFF) as u16;
    match id {
        ID_BTN_UNLOCK => {
            let password = read_edit_text_secure(hwnd, ID_EDIT_PASSWORD);
            clear_edit_text(hwnd, ID_EDIT_PASSWORD);

            let cb = {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
                if !state_ptr.is_null() {
                    let state = &mut *state_ptr;
                    state.closed = true;
                    state.on_result.take()
                } else {
                    None
                }
            };

            if let Some(cb) = cb {
                cb(UnlockResult::Confirmed(password));
            }

            let _ = DestroyWindow(hwnd);
        }
        ID_BTN_CLOSE => {
            let cb = take_cancel_callback(hwnd);
            if let Some(cb) = cb {
                cb(UnlockResult::Cancelled);
            }
            let _ = DestroyWindow(hwnd);
        }
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    LRESULT(0)
}

pub(super) unsafe fn on_keydown(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let vk = VIRTUAL_KEY(wparam.0 as u16);
    if vk == VK_RETURN {
        let _ = PostMessageW(
            Some(hwnd),
            WM_COMMAND,
            WPARAM(ID_BTN_UNLOCK as usize),
            LPARAM(0),
        );
        LRESULT(0)
    } else if vk == VK_ESCAPE {
        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM::default(), LPARAM::default());
        LRESULT(0)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ─── WM_CLOSE / WM_DESTROY ───────────────────────────────

pub(super) unsafe fn on_close(hwnd: HWND, _msg: u32, _wparam: WPARAM, _lparam: LPARAM) -> LRESULT {
    let cb = take_cancel_callback(hwnd);
    let _ = DestroyWindow(hwnd);
    if let Some(cb) = cb {
        cb(UnlockResult::Cancelled);
    }
    LRESULT(0)
}

pub(super) unsafe fn on_destroy(
    hwnd: HWND,
    _msg: u32,
    _wparam: WPARAM,
    _lparam: LPARAM,
) -> LRESULT {
    clear_edit_text(hwnd, ID_EDIT_PASSWORD);

    let hedit = GetDlgItem(hwnd, ID_EDIT_PASSWORD as i32);
    if !hedit.is_invalid() {
        if let Some(orig) = ORIGINAL_EDIT_PROC {
            SetWindowLongPtrW(hedit, GWLP_WNDPROC, orig as *const () as isize);
        }
    }
    ORIGINAL_EDIT_PROC = None;

    let hbtn_unlock = GetDlgItem(hwnd, ID_BTN_UNLOCK as i32);
    if !hbtn_unlock.is_invalid() {
        if let Some(orig) = ORIGINAL_BTN_PROC {
            SetWindowLongPtrW(hbtn_unlock, GWLP_WNDPROC, orig as *const () as isize);
        }
    }
    let hbtn_close = GetDlgItem(hwnd, ID_BTN_CLOSE as i32);
    if !hbtn_close.is_invalid() {
        if let Some(orig) = ORIGINAL_BTN_PROC {
            SetWindowLongPtrW(hbtn_close, GWLP_WNDPROC, orig as *const () as isize);
        }
    }
    ORIGINAL_BTN_PROC = None;

    if !DEFAULT_FONT.0.is_null() {
        let _ = DeleteObject(DEFAULT_FONT.into());
        DEFAULT_FONT = HFONT(std::ptr::null_mut());
    }
    {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
        if !state_ptr.is_null() {
            let brush = (*state_ptr).dark_brush;
            if !brush.0.is_null() {
                let _ = DeleteObject(brush.into());
            }
            (*state_ptr).dark_brush = HBRUSH(std::ptr::null_mut());
        }
    }
    BTN_HOVER_UNLOCK = false;
    BTN_HOVER_CLOSE = false;
    PostQuitMessage(0);

    let _ = UnregisterClassW(
        WINDOW_CLASS,
        Some(HINSTANCE(
            GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) as *mut c_void
        )),
    );

    LRESULT(0)
}

// ─── Dialog Actions ──────────────────────────────────────

unsafe fn take_cancel_callback(hwnd: HWND) -> Option<Box<dyn FnOnce(UnlockResult) + Send>> {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
    if state_ptr.is_null() {
        return None;
    }
    let state = &mut *state_ptr;
    state.closed = true;
    state.on_result.take()
}
