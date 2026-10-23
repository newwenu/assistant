import React, { useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir } from "@tauri-apps/api/path";
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

  const aspectStyle = useMemo<React.CSSProperties>(() => {
    if (attachment.width && attachment.height) {
      return { aspectRatio: `${attachment.width} / ${attachment.height}` };
    }
    return {};
  }, [attachment.width, attachment.height]);

  if (offloaded) {
    return (
      <div className={css.offloaded}>
        <span className={css.offloadedText}>🖼️</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className={css.error} style={aspectStyle}>
        <span className={css.errorIcon}>⚠️</span>
        <button
          type="button"
          className={css.retryBtn}
          onClick={() => {
            setError(false);
            setLoaded(false);
          }}
        >
          ↻
        </button>
      </div>
    );
  }

  if (!assetUrl) {
    return <div className={css.skeleton} style={aspectStyle} />;
  }

  return (
    <div className={css.wrapper} style={aspectStyle}>
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
