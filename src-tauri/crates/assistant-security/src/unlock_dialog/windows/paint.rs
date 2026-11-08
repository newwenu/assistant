use std::ffi::c_void;

use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        CreateSolidBrush, DeleteObject, DrawFocusRect, DrawTextW, FillRect, FrameRect, SetBkColor,
        SetBkMode, SetTextColor, BACKGROUND_MODE, DRAW_TEXT_FORMAT, HDC,
    },
    UI::Controls::{DRAWITEMSTRUCT, ODS_FOCUS, ODS_SELECTED},
    UI::WindowsAndMessaging::{
        DefWindowProcW, GetClientRect, GetWindowTextLengthW, GetWindowTextW,
    },
};

use super::ffi::{
    GetWindowLongPtrW, BKMODE_TRANSPARENT, DT_CENTER, DT_SINGLELINE, DT_VCENTER, GWLP_USERDATA,
    ID_BTN_CLOSE, ID_BTN_UNLOCK,
};
use super::state::{DialogStatePtr, BTN_HOVER_CLOSE, BTN_HOVER_UNLOCK};
use super::theme::{
    DARK_BG, DARK_BTN_BORDER, DARK_BTN_FACE, DARK_BTN_HOVER, DARK_BTN_PRESSED, DARK_TEXT,
};

// ─── Dialog Background / Control Colors ──────────────────

pub(super) unsafe fn on_erase_bkgnd(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
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

pub(super) unsafe fn on_ctl_color(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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

pub(super) unsafe fn on_draw_item(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let dis = lparam.0 as *const DRAWITEMSTRUCT;
    if dis.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    draw_dark_button(&*dis);
    LRESULT(1)
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

    let _ = SetBkMode(dc, BACKGROUND_MODE(BKMODE_TRANSPARENT));
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
