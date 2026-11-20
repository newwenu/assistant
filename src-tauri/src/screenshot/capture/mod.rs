// 参考: Kiri/src-tauri/src/capture/windows.rs — xcap 枚举/抓帧、按光标选屏、首帧空则重试一次
// 参考: Cap — 预览编码参数（长边上限 + JPEG 质量）

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use image::codecs::jpeg::JpegEncoder;
use image::{imageops, DynamicImage, ExtendedColorType, ImageEncoder, RgbaImage};
use log::{info, warn};
use tauri::{AppHandle, Manager};

use assistant_config::constant::{SCREENSHOT_PREVIEW_JPEG_QUALITY, SCREENSHOT_PREVIEW_LONG_EDGE};
use assistant_orb::hover::ScreenBounds;
use assistant_orb::placement::{self, WindowBounds};

use super::{CAPTURE_FAILED, ENCODE_FAILED, NO_MONITOR};

/// 单个显示器的物理几何，与 Tauri `Monitor` 物理坐标同一坐标系。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorInfo {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl MonitorInfo {
    fn screen_bounds(self) -> ScreenBounds {
        ScreenBounds {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    fn contains(self, point: (i32, i32)) -> bool {
        let (px, py) = point;
        let right = self.x.saturating_add(self.width as i32);
        let bottom = self.y.saturating_add(self.height as i32);
        px >= self.x && px < right && py >= self.y && py < bottom
    }
}

/// 抓屏后端：生产实现走 `xcap`，单测可注入假实现，避免依赖真实桌面。
pub trait CaptureBackend: Send + Sync {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, String>;
    fn capture(&self, monitor: MonitorInfo) -> Result<RgbaImage, String>;
}

pub struct XcapBackend;

impl CaptureBackend for XcapBackend {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, String> {
        let monitors = xcap::Monitor::all().map_err(|_| NO_MONITOR.to_string())?;
        let mut out = Vec::with_capacity(monitors.len());
        for monitor in monitors {
            let (Ok(x), Ok(y), Ok(width), Ok(height)) =
                (monitor.x(), monitor.y(), monitor.width(), monitor.height())
            else {
                warn!("screenshot: skip monitor with unreadable geometry");
                continue;
            };
            out.push(MonitorInfo {
                x,
                y,
                width,
                height,
            });
        }
        if out.is_empty() {
            return Err(NO_MONITOR.to_string());
        }
        Ok(out)
    }

    fn capture(&self, monitor: MonitorInfo) -> Result<RgbaImage, String> {
        let monitors = xcap::Monitor::all().map_err(|_| CAPTURE_FAILED.to_string())?;
        let target = monitors
            .iter()
            .find(|candidate| {
                let (Ok(x), Ok(y), Ok(width), Ok(height)) = (
                    candidate.x(),
                    candidate.y(),
                    candidate.width(),
                    candidate.height(),
                ) else {
                    return false;
                };
                x == monitor.x
                    && y == monitor.y
                    && width == monitor.width
                    && height == monitor.height
            })
            .ok_or_else(|| CAPTURE_FAILED.to_string())?;
        target
            .capture_image()
            .map_err(|_| CAPTURE_FAILED.to_string())
    }
}

mod permission;

pub use permission::permission_granted;

/// 按光标物理坐标选屏：命中包含它的显示器优先，否则复用 Orb 的 `nearest_screen` 度量。
pub fn select_monitor(monitors: &[MonitorInfo], point: (i32, i32)) -> Option<MonitorInfo> {
    if let Some(hit) = monitors.iter().copied().find(|m| m.contains(point)) {
        return Some(hit);
    }
    let screens: Vec<ScreenBounds> = monitors.iter().map(|m| m.screen_bounds()).collect();
    let picked = placement::nearest_screen(&screens, WindowBounds::new(point.0, point.1, 1, 1))?;
    monitors
        .iter()
        .copied()
        .find(|m| m.screen_bounds() == picked)
}

/// 抓屏并做空帧兜底：首帧为空/全黑时重试一次，仍失败才报错。
pub fn capture_monitor(
    backend: &dyn CaptureBackend,
    monitor: MonitorInfo,
) -> Result<RgbaImage, String> {
    let first = backend.capture(monitor)?;
    if !is_blank(&first) {
        return Ok(first);
    }
    warn!("screenshot: first frame is blank, retrying once");
    let second = backend.capture(monitor)?;
    if is_blank(&second) {
        return Err(CAPTURE_FAILED.to_string());
    }
    Ok(second)
}

/// 采样网格判定空帧：全透明或全近黑视为抓屏未就绪。
fn is_blank(image: &RgbaImage) -> bool {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return true;
    }
    let step_x = (width / 32).max(1);
    let step_y = (height / 32).max(1);
    let mut samples = 0usize;
    let mut blank = 0usize;
    let mut y = 0;
    while y < height {
        let mut x = 0;
        while x < width {
            let pixel = image.get_pixel(x, y).0;
            samples += 1;
            if pixel[3] == 0 || (pixel[0] < 8 && pixel[1] < 8 && pixel[2] < 8) {
                blank += 1;
            }
            x += step_x;
        }
        y += step_y;
    }
    samples > 0 && blank == samples
}

pub struct PreviewImage {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

/// 底图预览：长边压到 `SCREENSHOT_PREVIEW_LONG_EDGE` 后单次 JPEG 编码，直接内联为 data URL。
pub fn encode_preview(image: &RgbaImage) -> Result<PreviewImage, String> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err(ENCODE_FAILED.to_string());
    }
    let (target_width, target_height) = fit_long_edge(width, height, SCREENSHOT_PREVIEW_LONG_EDGE);
    // 走 DynamicImage 的非泛型包装：缩放与颜色转换在 image crate 内单态化，随依赖一起优化；
    // 直接调用 imageops::resize 会把重采样循环编进本 crate 的 codegen（dev 为 opt-level=0）。
    let resized = DynamicImage::ImageRgba8(image.clone());
    let resized = if (target_width, target_height) == (width, height) {
        resized
    } else {
        resized.resize_exact(
            target_width,
            target_height,
            imageops::FilterType::CatmullRom,
        )
    };

    let rgb = resized.to_rgb8();
    let mut buffer = Vec::new();
    JpegEncoder::new_with_quality(&mut buffer, SCREENSHOT_PREVIEW_JPEG_QUALITY)
        .write_image(
            rgb.as_raw(),
            target_width,
            target_height,
            ExtendedColorType::Rgb8,
        )
        .map_err(|_| ENCODE_FAILED.to_string())?;

    Ok(PreviewImage {
        data_url: format!("data:image/jpeg;base64,{}", B64.encode(&buffer)),
        width: target_width,
        height: target_height,
    })
}

/// 等比缩放到长边上限；未超限时原样返回。
pub(crate) fn fit_long_edge(width: u32, height: u32, long_edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest == 0 || longest <= long_edge {
        return (width, height);
    }
    let ratio = f64::from(long_edge) / f64::from(longest);
    let scaled_width = (f64::from(width) * ratio).round().max(1.0) as u32;
    let scaled_height = (f64::from(height) * ratio).round().max(1.0) as u32;
    (scaled_width, scaled_height)
}

/// 目标显示器的 `scale_factor`：用于诊断与日志；裁剪换算全部在 Rust 侧完成。
pub fn scale_factor_for(app: &AppHandle, monitor: MonitorInfo) -> f64 {
    if let Ok(monitors) = app.available_monitors() {
        for candidate in monitors {
            let position = candidate.position();
            let size = candidate.size();
            if position.x == monitor.x
                && position.y == monitor.y
                && size.width == monitor.width
                && size.height == monitor.height
            {
                return candidate.scale_factor();
            }
        }
    }
    1.0
}

/// 光标物理坐标：主路径取 Tauri 光标，其次 `mouse_position`，再退 Orb 中心与主屏中心。
pub fn cursor_point(app: &AppHandle) -> (i32, i32) {
    if let Ok(position) = app.cursor_position() {
        return (position.x.round() as i32, position.y.round() as i32);
    }
    if let Some(position) = crate::cmd::get_mouse_position() {
        return (position.client_x, position.client_y);
    }
    if let Some(window) = app.get_webview_window(assistant_config::constant::WINDOW_ORB) {
        if let (Ok(origin), Ok(size)) = (window.outer_position(), window.outer_size()) {
            return (
                origin.x.saturating_add(size.width as i32 / 2),
                origin.y.saturating_add(size.height as i32 / 2),
            );
        }
    }
    if let Some(primary) = app.primary_monitor().ok().flatten() {
        let position = primary.position();
        let size = primary.size();
        return (
            position.x.saturating_add(size.width as i32 / 2),
            position.y.saturating_add(size.height as i32 / 2),
        );
    }
    info!("screenshot: cursor position unavailable, falling back to (0, 0)");
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn monitor(x: i32, y: i32, width: u32, height: u32) -> MonitorInfo {
        MonitorInfo {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn select_monitor_prefers_containing_screen() {
        let monitors = vec![monitor(0, 0, 1920, 1080), monitor(1920, 0, 1280, 720)];
        assert_eq!(
            select_monitor(&monitors, (1900, 500)),
            Some(monitor(0, 0, 1920, 1080))
        );
        assert_eq!(
            select_monitor(&monitors, (1930, 100)),
            Some(monitor(1920, 0, 1280, 720))
        );
    }

    #[test]
    fn select_monitor_falls_back_to_nearest_when_outside_all() {
        let monitors = vec![monitor(0, 0, 1920, 1080), monitor(1920, 0, 1280, 720)];
        assert_eq!(
            select_monitor(&monitors, (2500, -100)),
            Some(monitor(1920, 0, 1280, 720))
        );
        assert_eq!(select_monitor(&[], (10, 10)), None);
    }

    #[test]
    fn fit_long_edge_keeps_small_and_caps_large() {
        assert_eq!(fit_long_edge(1920, 1080, 2560), (1920, 1080));
        assert_eq!(fit_long_edge(3840, 2160, 2560), (2560, 1440));
        assert_eq!(fit_long_edge(1080, 3840, 2560), (720, 2560));
        assert_eq!(fit_long_edge(0, 0, 2560), (0, 0));
    }

    #[test]
    fn is_blank_detects_uniform_dark_and_transparent_frames() {
        let dark = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 255]));
        assert!(is_blank(&dark));
        let transparent = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
        assert!(is_blank(&transparent));
        let mut mixed = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 255]));
        mixed.put_pixel(10, 10, Rgba([255, 255, 255, 255]));
        assert!(!is_blank(&mixed));
    }

    #[test]
    fn encode_preview_caps_long_edge_and_returns_data_url() {
        let image = RgbaImage::from_pixel(3840, 2160, Rgba([30, 60, 90, 255]));
        let preview = encode_preview(&image).expect("preview encodes");
        assert_eq!((preview.width, preview.height), (2560, 1440));
        assert!(preview.data_url.starts_with("data:image/jpeg;base64,"));
    }
}
