use windows::Win32::{
    Foundation::RECT,
    Graphics::Gdi::{CreateFontIndirectW, HFONT, LF_FACESIZE, LOGFONTW},
    UI::HiDpi::AdjustWindowRectExForDpi,
    UI::WindowsAndMessaging::{
        WS_CAPTION, WS_EX_CONTROLPARENT, WS_EX_DLGMODALFRAME, WS_EX_TOPMOST, WS_OVERLAPPED,
        WS_SYSMENU,
    },
};

const BASE_WINDOW_WIDTH: i32 = 380;
const BASE_WINDOW_HEIGHT: i32 = 150;

fn dpi_factor(dpi: u32) -> f64 {
    dpi as f64 / 96.0
}

fn scale(value: i32, factor: f64) -> i32 {
    (value as f64 * factor).round() as i32
}

pub(super) struct LayoutMetrics {
    pub margin: i32,
    pub title_y: i32,
    pub title_h: i32,
    pub edit_y: i32,
    pub edit_h: i32,
    pub btn_y: i32,
    pub btn_h: i32,
    pub btn_w: i32,
    pub icon_margin: i32,
    pub content_w: i32,
    pub unlock_btn_x: i32,
    pub close_btn_x: i32,
}

impl LayoutMetrics {
    pub fn new(dpi: u32) -> Self {
        let f = dpi_factor(dpi);
        Self {
            margin: scale(14, f),
            title_y: scale(10, f),
            title_h: scale(18, f),
            edit_y: scale(36, f),
            edit_h: scale(24, f),
            btn_y: scale(72, f),
            btn_h: scale(26, f),
            btn_w: scale(80, f),
            icon_margin: scale(26, f),
            content_w: scale(344, f),
            unlock_btn_x: scale(192, f),
            close_btn_x: scale(280, f),
        }
    }
}

pub(super) fn calc_window_rect(dpi: u32) -> RECT {
    let factor = dpi_factor(dpi);
    let client_w = scale(BASE_WINDOW_WIDTH, factor);
    let client_h = scale(BASE_WINDOW_HEIGHT, factor);

    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
    let ex_style = WS_EX_DLGMODALFRAME | WS_EX_TOPMOST | WS_EX_CONTROLPARENT;

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: client_w,
        bottom: client_h,
    };
    let _ = unsafe { AdjustWindowRectExForDpi(&mut rect, style, false, ex_style, dpi) };
    rect
}

pub(super) fn create_icon_font(dpi: u32) -> HFONT {
    let mut lf = LOGFONTW {
        lfHeight: -scale(14, dpi_factor(dpi)),
        lfWeight: 400,
        ..Default::default()
    };
    let face: Vec<u16> = "Segoe UI Emoji\0".encode_utf16().collect();
    let copy_len = face.len().min(LF_FACESIZE as usize);
    lf.lfFaceName[..copy_len].copy_from_slice(&face[..copy_len]);
    unsafe { CreateFontIndirectW(&lf) }
}

pub(super) fn create_default_font(dpi: u32) -> HFONT {
    let mut lf = LOGFONTW {
        lfHeight: -scale(12, dpi_factor(dpi)),
        lfWeight: 400,
        ..Default::default()
    };
    let face: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
    let copy_len = face.len().min(LF_FACESIZE as usize);
    lf.lfFaceName[..copy_len].copy_from_slice(&face[..copy_len]);
    unsafe { CreateFontIndirectW(&lf) }
}
