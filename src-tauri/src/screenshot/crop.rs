// 裁剪与编码：归一化选区 → 原图像素矩形 → 单次编码（PNG 快压/Sub 滤波，超限退 JPEG）
// 参考: Cap — Fast PNG + Sub filter；超限再降级 JPEG 保证体积

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::FilterType as ResizeFilter;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, RgbaImage};

use assistant_config::constant::SCREENSHOT_MAX_BYTES;

use super::{ENCODE_FAILED, TOO_LARGE};

/// 裁剪产物长边上限：超过则等比缩小，避免长截图把上下文窗口撑爆。
pub const CROP_MAX_LONG_EDGE: u32 = 2048;
/// PNG 超限后的 JPEG 质量；只在体积兜底时使用。
pub const CROP_JPEG_QUALITY: u8 = 85;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// 归一化选区（0..=1，相对底图显示区域）换算为原图像素矩形。
///
/// 两端取整后夹取到图像边界；宽或高为 0 时返回 `None`（对应 `too_small`）。
pub fn to_image_rect(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    image_width: u32,
    image_height: u32,
) -> Option<CropRect> {
    if image_width == 0 || image_height == 0 {
        return None;
    }
    if !(x.is_finite() && y.is_finite() && width.is_finite() && height.is_finite()) {
        return None;
    }

    let image_w = f64::from(image_width);
    let image_h = f64::from(image_height);

    let start_x = (x * image_w).round().clamp(0.0, image_w);
    let start_y = (y * image_h).round().clamp(0.0, image_h);
    let end_x = ((x + width) * image_w).round().clamp(0.0, image_w);
    let end_y = ((y + height) * image_h).round().clamp(0.0, image_h);

    let (left, right) = if start_x <= end_x {
        (start_x, end_x)
    } else {
        (end_x, start_x)
    };
    let (top, bottom) = if start_y <= end_y {
        (start_y, end_y)
    } else {
        (end_y, start_y)
    };

    let rect_width = (right - left) as u32;
    let rect_height = (bottom - top) as u32;
    if rect_width == 0 || rect_height == 0 {
        return None;
    }

    Some(CropRect {
        x: left as u32,
        y: top as u32,
        width: rect_width,
        height: rect_height,
    })
}

pub struct EncodedCrop {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
    pub extension: &'static str,
    pub width: u32,
    pub height: u32,
}

/// 裁剪并单次编码：先 PNG（快压/Sub 滤波），超限再 JPEG，仍超限报 `too_large`。
pub fn encode_crop(image: &RgbaImage, rect: CropRect) -> Result<EncodedCrop, String> {
    // 走 DynamicImage 的非泛型包装：裁剪与重采样在 image crate 内单态化，随依赖一起优化；
    // 直接调用 imageops::crop_imm / resize 会把像素拷贝循环编进本 crate 的 codegen（dev 为 opt-level=0）。
    let cropped =
        DynamicImage::ImageRgba8(image.clone()).crop_imm(rect.x, rect.y, rect.width, rect.height);
    let (crop_width, crop_height) = (cropped.width(), cropped.height());
    if crop_width == 0 || crop_height == 0 {
        return Err(ENCODE_FAILED.to_string());
    }

    let longest = crop_width.max(crop_height);
    let (width, height) = if longest > CROP_MAX_LONG_EDGE {
        let ratio = f64::from(CROP_MAX_LONG_EDGE) / f64::from(longest);
        (
            (f64::from(crop_width) * ratio).round().max(1.0) as u32,
            (f64::from(crop_height) * ratio).round().max(1.0) as u32,
        )
    } else {
        (crop_width, crop_height)
    };

    let final_image = if (width, height) == (crop_width, crop_height) {
        cropped
    } else {
        cropped.resize_exact(width, height, ResizeFilter::CatmullRom)
    };

    let mut png_bytes = Vec::new();
    PngEncoder::new_with_quality(&mut png_bytes, CompressionType::Fast, PngFilterType::Sub)
        .write_image(
            final_image.as_bytes(),
            width,
            height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|_| ENCODE_FAILED.to_string())?;

    if png_bytes.len() as u64 <= SCREENSHOT_MAX_BYTES {
        return Ok(EncodedCrop {
            bytes: png_bytes,
            media_type: "image/png",
            extension: "png",
            width,
            height,
        });
    }

    let rgb = final_image.to_rgb8();
    let mut jpeg_bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg_bytes, CROP_JPEG_QUALITY)
        .write_image(rgb.as_raw(), width, height, ExtendedColorType::Rgb8)
        .map_err(|_| ENCODE_FAILED.to_string())?;

    if jpeg_bytes.len() as u64 > SCREENSHOT_MAX_BYTES {
        return Err(TOO_LARGE.to_string());
    }

    Ok(EncodedCrop {
        bytes: jpeg_bytes,
        media_type: "image/jpeg",
        extension: "jpg",
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn to_image_rect_maps_normalized_region() {
        let rect = to_image_rect(0.5, 0.5, 0.5, 0.5, 1920, 1080).expect("rect");
        assert_eq!(
            rect,
            CropRect {
                x: 960,
                y: 540,
                width: 960,
                height: 540
            }
        );
    }

    #[test]
    fn to_image_rect_normalizes_reversed_drag() {
        let forward = to_image_rect(0.2, 0.3, 0.4, 0.2, 1000, 1000).expect("rect");
        let reversed = to_image_rect(0.6, 0.5, -0.4, -0.2, 1000, 1000).expect("rect");
        assert_eq!(forward, reversed);
    }

    #[test]
    fn to_image_rect_clamps_out_of_bounds_and_rejects_degenerate() {
        let rect = to_image_rect(-0.5, -0.5, 3.0, 3.0, 800, 600).expect("rect");
        assert_eq!(
            rect,
            CropRect {
                x: 0,
                y: 0,
                width: 800,
                height: 600
            }
        );
        assert_eq!(to_image_rect(0.1, 0.1, 0.0, 0.5, 800, 600), None);
        assert_eq!(to_image_rect(0.1, 0.1, 0.5, 0.0, 800, 600), None);
        assert_eq!(to_image_rect(f64::NAN, 0.1, 0.5, 0.5, 800, 600), None);
        assert_eq!(to_image_rect(0.1, 0.1, 0.5, 0.5, 0, 0), None);
    }

    #[test]
    fn encode_crop_returns_png_with_crop_dimensions() {
        let image = RgbaImage::from_pixel(200, 120, Rgba([10, 20, 30, 255]));
        let rect = to_image_rect(0.25, 0.25, 0.5, 0.5, 200, 120).expect("rect");
        let encoded = encode_crop(&image, rect).expect("encode");
        assert_eq!((encoded.width, encoded.height), (100, 60));
        assert_eq!(encoded.media_type, "image/png");
        assert_eq!(encoded.extension, "png");
        assert!(!encoded.bytes.is_empty());
    }

    #[test]
    fn encode_crop_downscales_oversized_crops() {
        let image = RgbaImage::from_pixel(4096, 4096, Rgba([0, 0, 0, 255]));
        let rect = CropRect {
            x: 0,
            y: 0,
            width: 4096,
            height: 4096,
        };
        let encoded = encode_crop(&image, rect).expect("encode");
        assert_eq!(
            (encoded.width, encoded.height),
            (CROP_MAX_LONG_EDGE, CROP_MAX_LONG_EDGE)
        );
    }
}
