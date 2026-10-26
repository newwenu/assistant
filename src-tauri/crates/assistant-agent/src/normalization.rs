use image::codecs::jpeg::JpegEncoder;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use std::io::Cursor;

/// 长边上限：超过后等比缩放。
const MAX_LONG_EDGE: u32 = 2048;
/// JPEG 重编码质量。
const JPEG_QUALITY: u8 = 85;

pub struct NormalizedImage {
    pub data: Vec<u8>,
    pub media_type: String,
    pub width: u32,
    pub height: u32,
}

/// 图片规范化：校验真实格式、读取 EXIF 方向、按长边缩放、重新编码。
///
/// PNG / JPEG 保留原格式；BMP / TIFF 统一转为 PNG（浏览器与 Provider 均不保证支持原格式）。
/// GIF / WebP 原样透传（GIF 缩放会丢帧，WebP 未启用解码 feature），宽高为 0 由调用方保留原值。
pub fn normalize_image(data: &[u8], media_type: &str) -> Result<NormalizedImage, String> {
    // GIF 缩放会丢帧、WebP 未启用解码 feature，两者原样透传
    if matches!(media_type, "image/gif" | "image/webp") {
        return Ok(NormalizedImage {
            data: data.to_vec(),
            media_type: media_type.to_string(),
            width: 0,
            height: 0,
        });
    }

    let expected = match media_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/bmp" => ImageFormat::Bmp,
        "image/tiff" => ImageFormat::Tiff,
        _ => return Err(format!("unsupported image media type: {}", media_type)),
    };

    // 以实际字节判断格式，避免客户端伪造 media_type
    let guessed = image::guess_format(data).map_err(|_| "invalid image data".to_string())?;
    if guessed != expected {
        return Err("image content does not match media type".to_string());
    }

    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("failed to read image: {}", e))?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("failed to decode image: {}", e))?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("failed to decode image: {}", e))?;
    img.apply_orientation(orientation);

    let output_is_png = !matches!(expected, ImageFormat::Jpeg);
    if img.width().max(img.height()) > MAX_LONG_EDGE {
        // resize 会等比缩放到指定边界内，不会拉伸变形
        img = img.resize(
            MAX_LONG_EDGE,
            MAX_LONG_EDGE,
            image::imageops::FilterType::Lanczos3,
        );
    }

    let width = img.width();
    let height = img.height();
    let mut out = Vec::new();
    if output_is_png {
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .map_err(|e| format!("failed to encode png: {}", e))?;
    } else {
        let encoder = JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
        img.write_with_encoder(encoder)
            .map_err(|e| format!("failed to encode jpeg: {}", e))?;
    }

    Ok(NormalizedImage {
        data: out,
        media_type: if output_is_png {
            "image/png".to_string()
        } else {
            "image/jpeg".to_string()
        },
        width,
        height,
    })
}
