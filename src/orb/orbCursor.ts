import { useEffect, useRef, type RefObject } from "react";
import grabHandSvg from "@/assets/cursors/hand-grab.svg?raw";
import grabbingHandSvg from "@/assets/cursors/hand-grabbing.svg?raw";
import { createLogger } from "@/services/system/logger";

const log = createLogger("OrbCursor");

export type OrbCursorVariant = "idle" | "active" | "menu";

const CURSOR_SIZE = 32;
const RETINA_SCALE = 2;
const DEFAULT_ACCENT = "#1677ff";
const HEX_COLOR = /^#[0-9a-f]{6}$/i;

const HOTSPOTS: Record<OrbCursorVariant, { x: number; y: number }> = {
  idle: { x: 16, y: 19 },
  active: { x: 16, y: 19 },
  menu: { x: 3, y: 3 },
};

const FALLBACKS: Record<OrbCursorVariant, string> = {
  idle: "grab",
  active: "grabbing",
  menu: "default",
};

const CURSOR_VARS: Record<OrbCursorVariant, string> = {
  idle: "--orb-cursor-idle",
  active: "--orb-cursor-active",
  menu: "--orb-cursor-menu",
};

const VARIANTS: OrbCursorVariant[] = ["idle", "active", "menu"];

const MENU_ARROW_PATH = "M3 3 L3 23 L10 18 L14 27 L18 25 L14 16 L22 16 Z";

/** 手型沿用 Phinger Cursors 的 grab / grabbing 剪影：深色实心 + 半透明白外光晕，与原生光标同构。 */
const HAND_SVGS: Record<"idle" | "active", string> = {
  idle: grabHandSvg,
  active: grabbingHandSvg,
};

/** 3 变体 × 2 倍率 = 6 张/颜色，保留 4 种颜色的额度后整体清空，避免无界增长。 */
const CURSOR_CACHE = new Map<string, Promise<string | null>>();
const MAX_CACHED_ENTRIES = 24;

function resolveAccent(color: string | null | undefined): string {
  return color && HEX_COLOR.test(color) ? color : DEFAULT_ACCENT;
}

/** 缓存键按「归一化主色」而非原始入参：`null` / 非法值 / 缺省都会落到同一个默认色条目。 */
export function cursorCacheKey(
  color: string | null | undefined,
  variant: OrbCursorVariant,
  size: number,
): string {
  return `${resolveAccent(color)}|${variant}|${size}`;
}

export function buildOrbCursorSvg(
  color: string | null | undefined,
  variant: OrbCursorVariant,
): string {
  if (variant !== "menu") return HAND_SVGS[variant];
  const accent = resolveAccent(color);
  const shape = `<path d="${MENU_ARROW_PATH}" fill="${accent}" stroke="#ffffff" stroke-width="1.6" stroke-linejoin="round"/>`;
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${CURSOR_SIZE}" height="${CURSOR_SIZE}" viewBox="0 0 32 32">` +
    `<defs><filter id="shadow" x="-50%" y="-50%" width="200%" height="200%">` +
    `<feDropShadow dx="0" dy="1" stdDeviation="1.2" flood-color="#000000" flood-opacity="0.35"/>` +
    `</filter></defs>` +
    `<g filter="url(#shadow)">${shape}</g>` +
    `</svg>`
  );
}

export function rasterizeOrbCursor(
  svg: string,
  size: number,
): Promise<string | null> {
  return new Promise((resolve) => {
    const image = new Image();
    image.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = size;
      canvas.height = size;
      const context = canvas.getContext("2d");
      if (!context) {
        resolve(null);
        return;
      }
      context.drawImage(image, 0, 0, size, size);
      resolve(canvas.toDataURL("image/png"));
    };
    image.onerror = () => resolve(null);
    image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  });
}

/** 同一 key 只光栅化一次；StrictMode 的双次 effect 与配色反复切换都复用同一张位图。 */
function rasterizeCached(
  color: string | null | undefined,
  variant: OrbCursorVariant,
  size: number,
): Promise<string | null> {
  const key = cursorCacheKey(color, variant, size);
  const hit = CURSOR_CACHE.get(key);
  if (hit) return hit;
  if (CURSOR_CACHE.size >= MAX_CACHED_ENTRIES) CURSOR_CACHE.clear();
  const task = rasterizeOrbCursor(buildOrbCursorSvg(color, variant), size).then(
    (png) => {
      if (!png) CURSOR_CACHE.delete(key);
      return png;
    },
  );
  CURSOR_CACHE.set(key, task);
  return task;
}

export type CursorImageSet = "image-set" | "-webkit-image-set" | null;

export function resolveCursorImageSet(): CursorImageSet {
  if (typeof CSS === "undefined" || typeof CSS.supports !== "function") {
    return null;
  }
  const probe = (fn: string) =>
    `${fn}(url("cursor.png") 1x, url("cursor@2x.png") 2x) 16 16, auto`;
  if (CSS.supports("cursor", probe("image-set"))) return "image-set";
  if (CSS.supports("cursor", probe("-webkit-image-set"))) {
    return "-webkit-image-set";
  }
  return null;
}

export function buildOrbCursorValue(
  png1x: string,
  png2x: string | null,
  fallback: string,
  hotspot: { x: number; y: number },
  imageSet: CursorImageSet = resolveCursorImageSet(),
): string {
  const point = `${hotspot.x} ${hotspot.y}`;
  if (png2x && imageSet) {
    return `${imageSet}(url("${png1x}") 1x, url("${png2x}") 2x) ${point}, ${fallback}`;
  }
  return `url("${png1x}") ${point}, ${fallback}`;
}

export function useOrbCursor(
  color: string | null | undefined,
): RefObject<HTMLDivElement | null> {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const host = ref.current;
    if (!host) return;
    let disposed = false;

    void (async () => {
      const imageSet = resolveCursorImageSet();
      const results = await Promise.all(
        VARIANTS.map(async (variant) => {
          const [png1x, png2x] = await Promise.all([
            rasterizeCached(color, variant, CURSOR_SIZE),
            rasterizeCached(color, variant, CURSOR_SIZE * RETINA_SCALE),
          ]);
          return { variant, png1x, png2x };
        }),
      );
      if (disposed) return;
      let applied = 0;
      for (const { variant, png1x, png2x } of results) {
        if (!png1x) continue;
        host.style.setProperty(
          CURSOR_VARS[variant],
          buildOrbCursorValue(
            png1x,
            png2x,
            FALLBACKS[variant],
            HOTSPOTS[variant],
            imageSet,
          ),
        );
        applied += 1;
      }
      if (applied === 0) {
        log.warn("custom orb cursor unavailable, keep native cursor");
      }
    })();

    return () => {
      disposed = true;
      for (const variant of VARIANTS) {
        host.style.removeProperty(CURSOR_VARS[variant]);
      }
    };
  }, [color]);

  return ref;
}
