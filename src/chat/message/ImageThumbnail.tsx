import React, { useCallback, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";
import { useTranslation } from "react-i18next";
import { FileImageOutlined, ReloadOutlined } from "@ant-design/icons";
import type { ImageAttachmentRef } from "../core/contentBlockTypes";
import { getAttachmentsDir, openAttachmentFile } from "../attachment/paths";
import css from "./ImageThumbnail.module.css";

const ERROR_CONFIRM_DELAY_MS = 1500;

interface ImageThumbnailProps {
  attachment: ImageAttachmentRef;
  cachedData?: string;
  offloaded?: true;
}

export const ImageThumbnail: React.FC<ImageThumbnailProps> = ({
  attachment,
  cachedData,
  offloaded,
}) => {
  const { t } = useTranslation();
  const [assetUrl, setAssetUrl] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState(false);
  const [retryTick, setRetryTick] = useState(0);
  const errorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const hasValidId = attachment.attachmentId.length > 0;

  React.useEffect(() => {
    setLoaded(false);
    setError(false);
    if (errorTimerRef.current !== null) {
      clearTimeout(errorTimerRef.current);
      errorTimerRef.current = null;
    }

    if (!hasValidId) {
      if (cachedData) {
        const mime = attachment.mediaType || "image/png";
        setAssetUrl(`data:${mime};base64,${cachedData}`);
      } else {
        setAssetUrl(null);
      }
      return;
    }

    let cancelled = false;
    void (async () => {
      const dir = await getAttachmentsDir();
      const filePath = await join(dir, attachment.attachmentId);
      if (!cancelled) {
        setAssetUrl(convertFileSrc(filePath));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [attachment.attachmentId, hasValidId, cachedData, attachment.mediaType]);

  React.useEffect(() => {
    return () => {
      if (errorTimerRef.current !== null) {
        clearTimeout(errorTimerRef.current);
      }
    };
  }, []);

  const handleOpenViewer = useCallback(async () => {
    if (!hasValidId) return;
    try {
      await openAttachmentFile(attachment.attachmentId);
    } catch (e) {
      console.warn("Failed to open image viewer:", e);
    }
  }, [attachment.attachmentId, hasValidId]);

  const aspectStyle = useMemo<React.CSSProperties>(() => {
    if (attachment.width && attachment.height) {
      return { aspectRatio: `${attachment.width} / ${attachment.height}` };
    }
    return {};
  }, [attachment.width, attachment.height]);

  const handleLoad = useCallback(() => {
    if (errorTimerRef.current !== null) {
      clearTimeout(errorTimerRef.current);
      errorTimerRef.current = null;
    }
    setLoaded(true);
  }, []);

  const handleError = useCallback(() => {
    if (errorTimerRef.current !== null) return;
    errorTimerRef.current = setTimeout(() => {
      setError(true);
      errorTimerRef.current = null;
    }, ERROR_CONFIRM_DELAY_MS);
  }, []);

  const handleRetry = useCallback(() => {
    if (errorTimerRef.current !== null) {
      clearTimeout(errorTimerRef.current);
      errorTimerRef.current = null;
    }
    setError(false);
    setLoaded(false);
    // data: URL 无法通过换 URL 重试，只有文件型 URL 追加时间戳绕开缓存
    setRetryTick((tick) => tick + 1);
  }, []);

  const displayUrl = useMemo(() => {
    if (!assetUrl) return null;
    if (assetUrl.startsWith("data:")) return assetUrl;
    const sep = assetUrl.includes("?") ? "&" : "?";
    return `${assetUrl}${sep}retry=${retryTick}`;
  }, [assetUrl, retryTick]);

  if (offloaded) {
    return (
      <div className={css.offloaded} style={aspectStyle}>
        <FileImageOutlined className={css.offloadedIcon} />
        <span className={css.offloadedText}>
          {t("chat.input.image_not_found")}
        </span>
      </div>
    );
  }

  if (error) {
    return (
      <div className={css.error} style={aspectStyle}>
        <FileImageOutlined className={css.errorIcon} />
        <span className={css.errorText}>{t("chat.input.image_not_found")}</span>
        <button
          type="button"
          className={css.retryBtn}
          onClick={handleRetry}
          title={t("chat.input.image_retry")}
        >
          <ReloadOutlined />
        </button>
      </div>
    );
  }

  if (!displayUrl) {
    return <div className={css.skeleton} style={aspectStyle} />;
  }

  return (
    <div
      className={css.wrapper}
      style={aspectStyle}
      role="button"
      tabIndex={0}
      onClick={() => {
        void handleOpenViewer();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          void handleOpenViewer();
        }
      }}
    >
      {!loaded && <div className={css.skeleton} />}
      <img
        className={`${css.img}${loaded ? ` ${css.imgLoaded}` : ""}`}
        src={displayUrl}
        loading="lazy"
        alt={attachment.name ?? "image"}
        onLoad={handleLoad}
        onError={handleError}
      />
    </div>
  );
};
