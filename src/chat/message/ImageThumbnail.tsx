import React, { useCallback, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir } from "@tauri-apps/api/path";
import { openPath } from "@tauri-apps/plugin-opener";
import { useTranslation } from "react-i18next";
import { FileImageOutlined } from "@ant-design/icons";
import type { ImageAttachmentRef } from "../core/contentBlockTypes";
import css from "./ImageThumbnail.module.css";

let attachmentsDirCache: string | null = null;

async function getAttachmentsDir(): Promise<string> {
  if (!attachmentsDirCache) {
    attachmentsDirCache = (await appDataDir()) + "attachments";
  }
  return attachmentsDirCache;
}

interface ImageThumbnailProps {
  attachment: ImageAttachmentRef;
  offloaded?: true;
}

export const ImageThumbnail: React.FC<ImageThumbnailProps> = ({
  attachment,
  offloaded,
}) => {
  const { t } = useTranslation();
  const [assetUrl, setAssetUrl] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState(false);

  React.useEffect(() => {
    let cancelled = false;
    void getAttachmentsDir().then((dir) => {
      if (!cancelled) {
        setAssetUrl(convertFileSrc(`${dir}/${attachment.attachmentId}`));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [attachment.attachmentId]);

  const handleOpenViewer = useCallback(async () => {
    const dir = await getAttachmentsDir();
    const filePath = `${dir}/${attachment.attachmentId}`;
    try {
      await openPath(filePath);
    } catch (e) {
      console.warn("Failed to open image viewer:", e);
    }
  }, [attachment.attachmentId]);

  const aspectStyle = useMemo<React.CSSProperties>(() => {
    if (attachment.width && attachment.height) {
      return { aspectRatio: `${attachment.width} / ${attachment.height}` };
    }
    return {};
  }, [attachment.width, attachment.height]);

  if (offloaded || error) {
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
        onLoad={() => setLoaded(true)}
        onError={() => setError(true)}
      />
    </div>
  );
};
