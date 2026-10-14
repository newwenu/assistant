import { type FC, useEffect, useRef } from "react";

import type { MascotRendererProps, GifRendererConfig } from "../../core/types";
import { asFace } from "../../core/types";
import { isValidGifSrc } from "./gifUrlValidator";

const DEFAULT_GIF_SRC = "/mascot.gif";

function freezeGif(img: HTMLImageElement): void {
  const still = () => {
    if (img.dataset.mode !== "still" || img.naturalWidth === 0) return;
    const canvas = document.createElement("canvas");
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.drawImage(img, 0, 0);
    try {
      img.src = canvas.toDataURL("image/png");
    } catch {
      // cross-origin restriction, ignore
    }
  };
  if (img.complete && img.naturalWidth > 0) still();
  else img.addEventListener("load", still, { once: true });
}

export const GifMascot: FC<MascotRendererProps> = ({
  face = asFace("idle"),
  size = "100%",
  rendererConfig,
}) => {
  const config = rendererConfig as GifRendererConfig | undefined;
  const rawSrc = config?.src ?? DEFAULT_GIF_SRC;
  const gifSrc = isValidGifSrc(rawSrc) ? rawSrc : DEFAULT_GIF_SRC;
  const borderRadius = config?.borderRadius ?? 50;

  const imgRef = useRef<HTMLImageElement>(null);
  const originalSrcRef = useRef<string>(gifSrc);

  useEffect(() => {
    const img = imgRef.current;
    if (!img) return;

    const play = face !== "idle";
    if (play) {
      if (img.dataset.mode !== "play") {
        img.dataset.mode = "play";
        img.src = originalSrcRef.current;
      }
      return;
    }
    if (img.dataset.mode === "still") return;
    img.dataset.mode = "still";
    img.src = originalSrcRef.current;
    freezeGif(img);
  }, [face]);

  useEffect(() => {
    originalSrcRef.current = gifSrc;
    const img = imgRef.current;
    if (img) {
      img.dataset.mode = "play";
      img.src = gifSrc;
    }
  }, [gifSrc]);

  const sizeStyle = typeof size === "number" ? `${size}px` : size;
  const playing = face !== "idle";

  return (
    <div
      style={{
        width: sizeStyle,
        height: sizeStyle,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
      data-face={face}
    >
      <img
        ref={imgRef}
        src={gifSrc}
        alt=""
        aria-hidden="true"
        referrerPolicy="no-referrer"
        draggable={false}
        data-mode="play"
        data-playing={playing}
        style={{
          width: "100%",
          height: "100%",
          objectFit: "cover",
          borderRadius: `${borderRadius}%`,
          pointerEvents: "none",
          userSelect: "none",
        }}
      />
    </div>
  );
};
