// 裁剪归档：选区 → 像素矩形 → 单次编码 → 归档 → 返回 attachmentId。

use log::{info, warn};
use tauri::{AppHandle, Manager};

use crate::protocol::{ScreenshotCropArgsDto, ScreenshotCropResultDto};

use super::crop::{encode_crop, to_image_rect};
use super::orb_overlay;
use super::state::{restore_selecting, Phase, ScreenshotState};
use super::{ARCHIVE_FAILED, ENCODE_FAILED, TOKEN_INVALID, TOO_SMALL};

/// 裁剪归档：选区 → 像素矩形 → 单次编码 → 归档 → 返回 attachmentId。
pub async fn crop(
    app: &AppHandle,
    args: ScreenshotCropArgsDto,
) -> Result<ScreenshotCropResultDto, String> {
    let state = app.state::<ScreenshotState>();
    let _guard = state.op.lock().await;

    let (image, orb_snapshot, target) = {
        let mut inner = state.lock_inner();
        if inner.phase != Phase::Selecting {
            return Err(TOKEN_INVALID.to_string());
        }
        let (image, orb, target) = match inner.session.as_ref() {
            Some(session) if session.token == args.token => {
                (session.image.clone(), session.orb, session.target)
            }
            _ => return Err(TOKEN_INVALID.to_string()),
        };
        inner.phase = Phase::Cropping;
        (image, orb, target)
    };
    info!(
        "screenshot: crop on {}x{} monitor at ({}, {}), scale {:.2}",
        target.monitor_width,
        target.monitor_height,
        target.monitor_x,
        target.monitor_y,
        target.scale_factor
    );

    let rect = match to_image_rect(
        args.x,
        args.y,
        args.width,
        args.height,
        image.width(),
        image.height(),
    ) {
        Some(rect) => rect,
        None => {
            restore_selecting(&state);
            return Err(TOO_SMALL.to_string());
        }
    };

    let encoded = tokio::task::spawn_blocking(move || encode_crop(image.as_ref(), rect))
        .await
        .map_err(|_| ENCODE_FAILED.to_string())?;

    let encoded = match encoded {
        Ok(encoded) => encoded,
        Err(error) => {
            restore_selecting(&state);
            return Err(error);
        }
    };

    let file_name = format!("{}.{}", uuid::Uuid::new_v4().simple(), encoded.extension);
    let archived =
        crate::cmd::archive_bytes(app, &encoded.bytes, &file_name, encoded.media_type).await;

    let archived = match archived {
        Ok(archived) => archived,
        Err(error) => {
            warn!("screenshot: archive failed: {}", error);
            restore_selecting(&state);
            return Err(ARCHIVE_FAILED.to_string());
        }
    };

    // 归档成功后重新校验令牌再清会话，避免与取消/新会话交错。
    let cleared = {
        let mut inner = state.lock_inner();
        match inner.session.as_ref() {
            Some(session) if session.token == args.token => {
                inner.session = None;
                inner.phase = Phase::Idle;
                true
            }
            _ => false,
        }
    };
    if !cleared {
        return Err(TOKEN_INVALID.to_string());
    }

    let _ = orb_overlay::exit(app, orb_snapshot);

    Ok(ScreenshotCropResultDto {
        attachment_id: archived.attachment_id,
        file_name: archived.file_name,
        media_type: archived.media_type,
        bytes: archived.bytes,
        width: encoded.width,
        height: encoded.height,
    })
}
