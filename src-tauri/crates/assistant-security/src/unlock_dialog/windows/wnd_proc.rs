use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::WindowsAndMessaging::{
        DefWindowProcW, DC_HASDEFID, DM_GETDEFID, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORBTN,
        WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DRAWITEM, WM_ERASEBKGND, WM_KEYDOWN,
        WM_NCCREATE,
    },
};

use super::ffi::ID_BTN_UNLOCK;
use super::paint::{on_ctl_color, on_draw_item, on_erase_bkgnd};
use super::wm_handlers::{on_close, on_command, on_create, on_destroy, on_keydown, on_nc_create};

// ─── Window Procedure ────────────────────────────────────

pub(super) unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => on_nc_create(hwnd, msg, wparam, lparam),
        WM_CREATE => on_create(hwnd, msg, wparam, lparam),
        WM_ERASEBKGND => on_erase_bkgnd(hwnd, msg, wparam, lparam),
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
            on_ctl_color(hwnd, msg, wparam, lparam)
        }
        WM_DRAWITEM => on_draw_item(hwnd, msg, wparam, lparam),
        WM_COMMAND => on_command(hwnd, msg, wparam, lparam),
        DM_GETDEFID => LRESULT((DC_HASDEFID << 16 | ID_BTN_UNLOCK as u32) as isize),
        WM_KEYDOWN => on_keydown(hwnd, msg, wparam, lparam),
        WM_CLOSE => on_close(hwnd, msg, wparam, lparam),
        WM_DESTROY => on_destroy(hwnd, msg, wparam, lparam),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
