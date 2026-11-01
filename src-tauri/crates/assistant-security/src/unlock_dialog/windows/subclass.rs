use windows::core::w;
use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{
        CreatePen, CreateSolidBrush, DeleteObject, Ellipse, GetDC, GetSysColor, InvalidateRect,
        LineTo, MoveToEx, Polyline, ReleaseDC, SelectObject, COLOR_GRAYTEXT, COLOR_WINDOWTEXT,
        PS_SOLID,
    },
    UI::Controls::WM_MOUSELEAVE,
    UI::Input::KeyboardAndMouse::{
        TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VIRTUAL_KEY, VK_ESCAPE, VK_RETURN,
    },
    UI::WindowsAndMessaging::{
        DefWindowProcW, GetAncestor, GetClientRect, LoadCursorW, PostMessageW, SendMessageW,
        SetCursor, SetWindowTextW, DLGC_WANTMESSAGE, GA_PARENT, IDC_HAND, MSG, WM_CLOSE,
        WM_COMMAND, WM_GETDLGCODE, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_SETCURSOR,
        WS_CAPTION,
    },
};
use zeroize::Zeroize;

use super::ffi::{
    get_x_lparam, get_y_lparam, GetCursorPos, GetDlgItem, GetWindowLongPtrW, ScreenToClient,
    EM_SETPASSWORDCHAR, GWLP_USERDATA, GWL_STYLE, ID_BTN_CLOSE, ID_BTN_UNLOCK, ID_EDIT_PASSWORD,
};
use super::state::{
    DialogStatePtr, BTN_HOVER_CLOSE, BTN_HOVER_UNLOCK, ORIGINAL_BTN_PROC, ORIGINAL_EDIT_PROC,
};
use super::text::read_edit_text;
use super::theme::DARK_TEXT;

// ─── Edit Subclass ───────────────────────────────────────

pub(super) unsafe extern "system" fn edit_subclass_proc(
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

pub(super) unsafe extern "system" fn btn_subclass_proc(
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

const ICON_REF_BOX: i32 = 24;
const LENS_SEGMENTS: i32 = 14;

unsafe fn lens_outline(cx: i32, cy: i32, half_w: i32, half_h: i32) -> Vec<POINT> {
    let mut points = Vec::with_capacity((LENS_SEGMENTS as usize + 1) * 2 + 1);
    let mut push = |t: f64, sign: f64| {
        let x = cx as f64 - half_w as f64 + (2 * half_w) as f64 * t;
        let y = cy as f64 + sign * half_h as f64 * (std::f64::consts::PI * t).sin();
        points.push(POINT {
            x: x.round() as i32,
            y: y.round() as i32,
        });
    };
    for i in 0..=LENS_SEGMENTS {
        push(i as f64 / LENS_SEGMENTS as f64, -1.0);
    }
    for i in (0..=LENS_SEGMENTS).rev() {
        push(i as f64 / LENS_SEGMENTS as f64, 1.0);
    }
    points.push(POINT {
        x: cx - half_w,
        y: cy,
    });
    points
}

unsafe fn draw_visibility_icon(hwnd: HWND) {
    let dc = GetDC(Some(hwnd));
    if dc.is_invalid() {
        return;
    }

    let icon_rect = get_icon_rect(hwnd);
    let box_h = icon_rect.bottom - icon_rect.top;
    let cx = (icon_rect.left + icon_rect.right) / 2;
    let cy = (icon_rect.top + icon_rect.bottom) / 2;

    let scale = (box_h as f64 / ICON_REF_BOX as f64).clamp(1.0, 3.0);
    let half_w = (9.0 * scale).round() as i32;
    let half_h = (5.0 * scale).round() as i32;

    let parent = get_parent_dialog(hwnd);
    let (is_dark, visible) = if !parent.is_invalid() {
        let sp = GetWindowLongPtrW(parent, GWLP_USERDATA) as DialogStatePtr;
        if !sp.is_null() {
            ((*sp).is_dark, (*sp).password_visible)
        } else {
            (false, false)
        }
    } else {
        (false, false)
    };

    let hover = is_cursor_in_icon_area(hwnd);
    let color = if is_dark {
        COLORREF(DARK_TEXT)
    } else if hover {
        COLORREF(GetSysColor(COLOR_WINDOWTEXT))
    } else {
        COLORREF(GetSysColor(COLOR_GRAYTEXT))
    };

    let stroke = ((1.0 * scale).round() as i32).max(1);
    let pen = CreatePen(PS_SOLID, stroke, color);
    let old_pen = SelectObject(dc, pen.into());

    let outline = lens_outline(cx, cy, half_w, half_h);
    let _ = Polyline(dc, &outline);

    let pupil_r = ((half_h as f64) * 0.62).round() as i32;
    let brush = CreateSolidBrush(color);
    let old_brush = SelectObject(dc, brush.into());
    let _ = Ellipse(dc, cx - pupil_r, cy - pupil_r, cx + pupil_r, cy + pupil_r);
    SelectObject(dc, old_brush);
    let _ = DeleteObject(brush.into());

    if visible {
        let slash_stroke = ((1.8 * scale).round() as i32).max(2);
        let slash_pen = CreatePen(PS_SOLID, slash_stroke, color);
        let prev_pen = SelectObject(dc, slash_pen.into());
        let overhang = (2.0 * scale).round() as i32;
        let rise = (3.0 * scale).round() as i32;
        let _ = MoveToEx(dc, cx - half_w - overhang, cy - half_h - rise, None);
        let _ = LineTo(dc, cx + half_w + overhang, cy + half_h + rise);
        SelectObject(dc, prev_pen);
        let _ = DeleteObject(slash_pen.into());
    }

    SelectObject(dc, old_pen);
    let _ = DeleteObject(pen.into());
    let _ = ReleaseDC(Some(hwnd), dc);
}

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

// ─── Dialog Actions ──────────────────────────────────────

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
