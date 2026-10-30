use crate::unlock_dialog::{UnlockDialog, UnlockResult};
use crate::vault_crypto::types::SecureString;

use std::ffi::c_void;

use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::{
        Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE},
        Gdi::{
            CreateSolidBrush, DeleteObject, DrawFocusRect, DrawTextW, FillRect, FrameRect, GetDC,
            GetSysColor, InvalidateRect, ReleaseDC, SelectObject, SetBkColor, SetBkMode,
            SetTextColor, COLOR_GRAYTEXT, DRAW_TEXT_FORMAT, HBRUSH, HDC, HFONT,
        },
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::Controls::{DRAWITEMSTRUCT, ODS_FOCUS, ODS_SELECTED, WM_MOUSELEAVE},
    UI::HiDpi::GetDpiForSystem,
    UI::Input::KeyboardAndMouse::{
        TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VIRTUAL_KEY, VK_ESCAPE, VK_RETURN,
    },
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetAncestor,
        GetClientRect, GetMessageW, GetSystemMetrics, GetWindowTextLengthW, GetWindowTextW,
        IsDialogMessageW, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW, SendMessageW,
        SetCursor, SetWindowTextW, TranslateMessage, UnregisterClassW, CREATESTRUCTW, DC_HASDEFID,
        DLGC_WANTMESSAGE, DM_GETDEFID, GA_PARENT, HMENU, IDC_HAND, MSG, SM_CXSCREEN, SM_CYSCREEN,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORBTN,
        WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DRAWITEM, WM_ERASEBKGND, WM_GETDLGCODE,
        WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_NCCREATE, WM_SETCURSOR, WM_SETFONT, WNDCLASSW,
        WS_BORDER, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME,
        WS_EX_TOPMOST, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
    },
};

use windows::core::w;
use zeroize::Zeroize;

mod layout;
mod localization;
mod theme;

use layout::{calc_window_rect, create_default_font, create_icon_font, LayoutMetrics};
use localization::{get_ui_strings, str_to_wide, UiStrings};
use theme::{
    is_system_dark_mode, DARK_BG, DARK_BTN_BORDER, DARK_BTN_FACE, DARK_BTN_HOVER, DARK_BTN_PRESSED,
    DARK_TEXT,
};

// ─── Control IDs & Style Constants ───────────────────────

const ID_EDIT_PASSWORD: u16 = 101;
const ID_BTN_UNLOCK: u16 = 201;
const ID_BTN_CLOSE: u16 = 202;
const ID_STATIC_TITLE: u16 = 301;

const ES_PASSWORD: u32 = 0x0020;
const ES_AUTOHSCROLL: u32 = 0x0080;
const BS_DEFPUSHBUTTON: u32 = 0x0001;
const BS_OWNERDRAW: u32 = 0x000B;
const BS_NOTIFY: u32 = 0x0400;
const EM_EMPTYUNDOBUFFER: u32 = 0x00CE;
const EM_SETMARGINS: u32 = 0x00D3;
const EM_SETPASSWORDCHAR: u32 = 0x00CC;
const EC_RIGHTMARGIN: u32 = 0x0002;

const DT_CENTER: u32 = 0x0001;
const DT_VCENTER: u32 = 0x0004;
const DT_SINGLELINE: u32 = 0x0020;
const BKMODE_TRANSPARENT: u32 = 1;

const WINDOW_CLASS: windows::core::PCWSTR = w!("AssistantUnlockDlg");

// ─── Dialog State & FFI ──────────────────────────────────

type DialogStatePtr = *mut DialogState;

struct DialogState {
    on_result: Option<Box<dyn FnOnce(UnlockResult) + Send>>,
    closed: bool,
    password_visible: bool,
    layout: LayoutMetrics,
    is_dark: bool,
    dark_brush: HBRUSH,
    ui_strings: UiStrings,
}

const GWLP_USERDATA: i32 = -21;
const GWL_STYLE: i32 = -16;
const GWLP_WNDPROC: i32 = -4;
const GWLP_HINSTANCE: i32 = -6;

static mut ORIGINAL_EDIT_PROC: Option<
    unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
> = None;
static mut ORIGINAL_BTN_PROC: Option<
    unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
> = None;
static mut BTN_HOVER_UNLOCK: bool = false;
static mut BTN_HOVER_CLOSE: bool = false;
static mut ICON_FONT: HFONT = HFONT(std::ptr::null_mut());
static mut DEFAULT_FONT: HFONT = HFONT(std::ptr::null_mut());

fn make_hmenu(id: u16) -> HMENU {
    HMENU(id as *mut c_void)
}

fn get_x_lparam(lparam: LPARAM) -> i32 {
    (lparam.0 as u16) as i16 as i32
}

fn get_y_lparam(lparam: LPARAM) -> i32 {
    ((lparam.0 >> 16) as u16) as i16 as i32
}

#[link(name = "user32")]
extern "system" {
    fn GetWindowLongPtrW(hwnd: HWND, nindex: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: HWND, nindex: i32, newlong: isize) -> isize;
    fn GetDlgItem(hDlg: HWND, nIDDlgItem: i32) -> HWND;
    fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> i32;
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
}

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

// ─── Edit Subclass ───────────────────────────────────────

unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN => {
            if is_in_icon_area(hwnd, get_x_lparam(lparam), get_y_lparam(lparam)) {
                let parent = get_parent_dialog(hwnd);
                if !parent.is_invalid() {
                    toggle_password_visibility(parent);
                }
                draw_visibility_icon(hwnd);
                return LRESULT(0);
            }
        }
        WM_SETCURSOR => {
            if is_cursor_in_icon_area(hwnd) {
                let hcursor = LoadCursorW(None, IDC_HAND).unwrap_or_default();
                SetCursor(Some(hcursor));
                return LRESULT(1);
            }
        }
        WM_GETDLGCODE => {
            let mut code = call_original_edit_proc(hwnd, msg, wparam, lparam).0 as u32;
            if lparam.0 != 0 {
                let pmsg = lparam.0 as *const MSG;
                if (*pmsg).message == WM_KEYDOWN {
                    let vk = VIRTUAL_KEY((*pmsg).wParam.0 as u16);
                    if vk == VK_RETURN || vk == VK_ESCAPE {
                        code |= DLGC_WANTMESSAGE;
                    }
                }
            }
            return LRESULT(code as isize);
        }
        WM_KEYDOWN => {
            let vk = VIRTUAL_KEY(wparam.0 as u16);
            let parent = get_parent_dialog(hwnd);
            if !parent.is_invalid() {
                if vk == VK_RETURN {
                    let _ = PostMessageW(
                        Some(parent),
                        WM_COMMAND,
                        WPARAM(ID_BTN_UNLOCK as usize),
                        LPARAM(0),
                    );
                    return LRESULT(0);
                } else if vk == VK_ESCAPE {
                    let _ =
                        PostMessageW(Some(parent), WM_CLOSE, WPARAM::default(), LPARAM::default());
                    return LRESULT(0);
                }
            }
        }
        _ => {}
    }

    let result = call_original_edit_proc(hwnd, msg, wparam, lparam);
    draw_visibility_icon(hwnd);
    result
}

unsafe fn call_original_edit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(orig) = ORIGINAL_EDIT_PROC {
        orig(hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ─── Button Subclass ─────────────────────────────────────

unsafe extern "system" fn btn_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let btn_id = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u32;

    match msg {
        WM_MOUSEMOVE => {
            let was_hover = if btn_id == ID_BTN_UNLOCK as u32 {
                BTN_HOVER_UNLOCK
            } else if btn_id == ID_BTN_CLOSE as u32 {
                BTN_HOVER_CLOSE
            } else {
                return if let Some(orig) = ORIGINAL_BTN_PROC {
                    orig(hwnd, msg, wparam, lparam)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                };
            };
            if !was_hover {
                if btn_id == ID_BTN_UNLOCK as u32 {
                    BTN_HOVER_UNLOCK = true;
                } else {
                    BTN_HOVER_CLOSE = true;
                }
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
        }
        WM_MOUSELEAVE => {
            if btn_id == ID_BTN_UNLOCK as u32 {
                BTN_HOVER_UNLOCK = false;
            } else if btn_id == ID_BTN_CLOSE as u32 {
                BTN_HOVER_CLOSE = false;
            }
            let _ = InvalidateRect(Some(hwnd), None, true);
        }
        _ => {}
    }

    if let Some(orig) = ORIGINAL_BTN_PROC {
        orig(hwnd, msg, wparam, lparam)
    } else {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ─── Icon / Visibility ───────────────────────────────────

unsafe fn get_parent_dialog(hwnd: HWND) -> HWND {
    let mut parent = hwnd;
    for _ in 0..10 {
        let owner = GetWindowLongPtrW(parent, GWLP_USERDATA) as DialogStatePtr;
        if !owner.is_null() {
            return parent;
        }
        let style = GetWindowLongPtrW(parent, GWL_STYLE) as u32;
        let has_caption = (style & WS_CAPTION.0) != 0;
        if has_caption {
            return parent;
        }
        let p = GetAncestor(parent, GA_PARENT);
        if p.is_invalid() || p == parent {
            break;
        }
        parent = p;
    }
    parent
}

unsafe fn get_icon_rect(hwnd: HWND) -> RECT {
    let state_ptr = get_parent_dialog(hwnd);
    let margin = if !state_ptr.is_invalid() {
        let sp = GetWindowLongPtrW(state_ptr, GWLP_USERDATA) as DialogStatePtr;
        if !sp.is_null() {
            (*sp).layout.icon_margin
        } else {
            26
        }
    } else {
        26
    };
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    RECT {
        left: rect.right - margin,
        top: 0,
        right: rect.right - 2,
        bottom: rect.bottom,
    }
}

unsafe fn is_in_icon_area(hwnd: HWND, x: i32, y: i32) -> bool {
    let r = get_icon_rect(hwnd);
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

unsafe fn is_cursor_in_icon_area(hwnd: HWND) -> bool {
    let mut pt = POINT { x: 0, y: 0 };
    if GetCursorPos(&mut pt) == 0 {
        return false;
    }
    if ScreenToClient(hwnd, &mut pt) == 0 {
        return false;
    }
    is_in_icon_area(hwnd, pt.x, pt.y)
}

unsafe fn draw_visibility_icon(hwnd: HWND) {
    let dc = GetDC(Some(hwnd));
    if dc.is_invalid() {
        return;
    }

    let icon_rect = get_icon_rect(hwnd);
    let mut draw_rect = icon_rect;

    let font_to_use = if !ICON_FONT.0.is_null() {
        ICON_FONT
    } else {
        HFONT(std::ptr::null_mut())
    };
    let old_font = if !font_to_use.0.is_null() {
        SelectObject(dc, font_to_use.into())
    } else {
        windows::Win32::Graphics::Gdi::HGDIOBJ(std::ptr::null_mut())
    };

    let _ = SetBkMode(
        dc,
        windows::Win32::Graphics::Gdi::BACKGROUND_MODE(BKMODE_TRANSPARENT),
    );

    let state_ptr = get_parent_dialog(hwnd);
    let is_dark = if !state_ptr.is_invalid() {
        let sp = GetWindowLongPtrW(state_ptr, GWLP_USERDATA) as DialogStatePtr;
        if !sp.is_null() {
            (*sp).is_dark
        } else {
            false
        }
    } else {
        false
    };

    let icon_color = if is_dark {
        COLORREF(DARK_TEXT)
    } else {
        COLORREF(GetSysColor(COLOR_GRAYTEXT))
    };
    let _ = SetTextColor(dc, icon_color);

    let state_ptr = get_parent_dialog(hwnd);
    let visible = if !state_ptr.is_invalid() {
        let sp = GetWindowLongPtrW(state_ptr, GWLP_USERDATA) as DialogStatePtr;
        if !sp.is_null() {
            (*sp).password_visible
        } else {
            false
        }
    } else {
        false
    };

    let mut icon_text: Vec<u16> = if visible {
        "\u{1F648}".encode_utf16().collect()
    } else {
        "\u{1F441}".encode_utf16().collect()
    };

    let _ = DrawTextW(
        dc,
        &mut icon_text,
        &mut draw_rect as *mut RECT,
        DRAW_TEXT_FORMAT(DT_CENTER | DT_VCENTER | DT_SINGLELINE),
    );

    if !old_font.0.is_null() {
        SelectObject(dc, old_font);
    }
    let _ = ReleaseDC(Some(hwnd), dc);
}

// ─── Window Procedure ────────────────────────────────────

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let create = lparam.0 as *const CREATESTRUCTW;
            if !create.is_null() {
                let state = (*create).lpCreateParams as DialogStatePtr;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CREATE => {
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
                ICON_FONT = create_icon_font(dpi);
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
                    let orig =
                        SetWindowLongPtrW(h, GWLP_WNDPROC, btn_subclass_proc as *const () as isize);
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
                    let orig =
                        SetWindowLongPtrW(h, GWLP_WNDPROC, btn_subclass_proc as *const () as isize);
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
        WM_ERASEBKGND => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
            let is_dark = !state_ptr.is_null() && (*state_ptr).is_dark;
            if is_dark {
                let brush = (*state_ptr).dark_brush;
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let _ = FillRect(HDC(wparam.0 as *mut c_void), &rect, brush);
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
            let is_dark = !state_ptr.is_null() && (*state_ptr).is_dark;
            if is_dark {
                let dc = HDC(wparam.0 as *mut c_void);
                let _ = SetTextColor(dc, COLORREF(DARK_TEXT));
                let _ = SetBkColor(dc, COLORREF(DARK_BG));
                return LRESULT((*state_ptr).dark_brush.0 as isize);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DRAWITEM => {
            let dis = lparam.0 as *const DRAWITEMSTRUCT;
            if dis.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            unsafe { draw_dark_button(&*dis) };
            LRESULT(1)
        }
        WM_COMMAND => {
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
        DM_GETDEFID => LRESULT((DC_HASDEFID << 16 | ID_BTN_UNLOCK as u32) as isize),
        WM_KEYDOWN => {
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
        WM_CLOSE => {
            let cb = take_cancel_callback(hwnd);
            let _ = DestroyWindow(hwnd);
            if let Some(cb) = cb {
                cb(UnlockResult::Cancelled);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
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

            if !ICON_FONT.0.is_null() {
                let _ = DeleteObject(ICON_FONT.into());
                ICON_FONT = HFONT(std::ptr::null_mut());
            }
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
                Some(windows::Win32::Foundation::HINSTANCE(
                    GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) as *mut c_void,
                )),
            );

            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
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

unsafe fn toggle_password_visibility(hwnd: HWND) {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as DialogStatePtr;
    if state_ptr.is_null() {
        return;
    }
    let state = &mut *state_ptr;
    state.password_visible = !state.password_visible;

    let hedit = GetDlgItem(hwnd, ID_EDIT_PASSWORD as i32);
    if hedit.is_invalid() {
        return;
    }

    if state.password_visible {
        let _ = SendMessageW(hedit, EM_SETPASSWORDCHAR, Some(WPARAM(0)), Some(LPARAM(0)));
    } else {
        let _ = SendMessageW(
            hedit,
            EM_SETPASSWORDCHAR,
            Some(WPARAM(0x25CF)),
            Some(LPARAM(0)),
        );
    }

    let mut text_utf16 = read_edit_text(hedit);
    let _ = SetWindowTextW(hedit, w!(""));
    text_utf16.push(0u16);
    let _ = SetWindowTextW(hedit, windows::core::PCWSTR(text_utf16.as_ptr()));
    text_utf16.zeroize();

    let _ = InvalidateRect(Some(hedit), None, true);
}

// ─── Text Helpers ────────────────────────────────────────

unsafe fn read_edit_text(hedit: HWND) -> Vec<u16> {
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

fn read_edit_text_secure(hwnd: HWND, id: u16) -> SecureString {
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

fn clear_edit_text(hwnd: HWND, id: u16) {
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

// ─── Custom Drawing (Dark Theme) ─────────────────────────

unsafe fn draw_dark_button(dis: &DRAWITEMSTRUCT) {
    let dc = dis.hDC;
    let rect = dis.rcItem;

    let is_selected = (dis.itemState.0 & ODS_SELECTED.0) != 0;
    let btn_id = GetWindowLongPtrW(dis.hwndItem, GWLP_USERDATA) as u32;
    let is_hot = if btn_id == ID_BTN_UNLOCK as u32 {
        BTN_HOVER_UNLOCK
    } else if btn_id == ID_BTN_CLOSE as u32 {
        BTN_HOVER_CLOSE
    } else {
        false
    };
    let is_focused = (dis.itemState.0 & ODS_FOCUS.0) != 0;

    let bg_color = if is_selected {
        COLORREF(DARK_BTN_PRESSED)
    } else if is_hot {
        COLORREF(DARK_BTN_HOVER)
    } else {
        COLORREF(DARK_BTN_FACE)
    };

    let bg_brush = CreateSolidBrush(bg_color);
    let _ = FillRect(dc, &rect, bg_brush);
    let _ = DeleteObject(bg_brush.into());

    let border_brush = CreateSolidBrush(COLORREF(DARK_BTN_BORDER));
    let _ = FrameRect(dc, &rect, border_brush);
    let _ = DeleteObject(border_brush.into());

    let _ = SetBkMode(
        dc,
        windows::Win32::Graphics::Gdi::BACKGROUND_MODE(BKMODE_TRANSPARENT),
    );
    let _ = SetTextColor(dc, COLORREF(DARK_TEXT));

    let len = GetWindowTextLengthW(dis.hwndItem) as usize;
    if len > 0 {
        let mut buf = vec![0u16; len + 1];
        let copied = GetWindowTextW(dis.hwndItem, &mut buf);
        if copied > 0 {
            let mut draw_rect = rect;
            let _ = DrawTextW(
                dc,
                &mut buf[..(copied as usize + 1)],
                &mut draw_rect as *mut RECT,
                DRAW_TEXT_FORMAT(DT_CENTER | DT_VCENTER | DT_SINGLELINE),
            );
        }
    }

    if is_focused {
        let mut focus_rect = rect;
        focus_rect.left += 2;
        focus_rect.top += 2;
        focus_rect.right -= 2;
        focus_rect.bottom -= 2;
        let _ = DrawFocusRect(dc, &focus_rect);
    }
}
