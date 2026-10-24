import React, { useCallback, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir, join } from "@tauri-apps/api/path";
import { openPath } from "@tauri-apps/plugin-opener";
import { useTranslation } from "react-i18next";
import { FileImageOutlined } from "@ant-design/icons";
import type { ImageAttachmentRef } from "../core/contentBlockTypes";
import css from "./ImageThumbnail.module.css";

const ERROR_CONFIRM_DELAY_MS = 1500;

let attachmentsDirCache: string | null = null;

async function getAttachmentsDir(): Promise<string> {
  if (!attachmentsDirCache) {
    attachmentsDirCache = await join(await appDataDir(), "attachments");
  }
  return attachmentsDirCache;
}

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
    const dir = await getAttachmentsDir();
    const filePath = await join(dir, attachment.attachmentId);
    try {
      await openPath(filePath);
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
      <div className={css.offloaded} style={aspectStyle}>
        <FileImageOutlined className={css.offloadedIcon} />
        <span className={css.offloadedText}>
          {t("chat.input.image_not_found")}
        </span>
      </div>
    );
  }

  if (!assetUrl) {
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
        src={assetUrl}
        loading="lazy"
        alt={attachment.name ?? "image"}
        onLoad={handleLoad}
        onError={handleError}
      />
    </div>
  );
};
