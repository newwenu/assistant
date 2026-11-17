import { SCREENSHOT_MIN_SELECTION_CSS_PX } from "@/config/constants";

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface NormRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type HandleId = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";

/** 拖拽起点与当前点的包围盒；反向拖拽由 min/abs 归一，不需要调用方判断方向。 */
export function normalizeDrag(
  startX: number,
  startY: number,
  curX: number,
  curY: number,
): Rect {
  return {
    x: Math.min(startX, curX),
    y: Math.min(startY, curY),
    width: Math.abs(curX - startX),
    height: Math.abs(curY - startY),
  };
}

/** 夹取到显示区域内：先夹位置，再夹尺寸，保证 `x + width <= bounds.width`。 */
export function clampToBounds(rect: Rect, bounds: Rect): Rect {
  const width = Math.max(0, Math.min(rect.width, bounds.width));
  const height = Math.max(0, Math.min(rect.height, bounds.height));
  const x = Math.max(
    bounds.x,
    Math.min(rect.x, bounds.x + bounds.width - width),
  );
  const y = Math.max(
    bounds.y,
    Math.min(rect.y, bounds.y + bounds.height - height),
  );
  return { x, y, width, height };
}

/**
 * 手柄缩放：只移动被抓住的边，另一侧边固定；反向越过固定边时交换两边，
 * 结果始终是非负宽高，避免出现「负宽度选区」再被后端判为 too_small。
 */
export function resizeByHandle(
  rect: Rect,
  handle: HandleId,
  dx: number,
  dy: number,
  bounds: Rect,
): Rect {
  let left = rect.x;
  let top = rect.y;
  let right = rect.x + rect.width;
  let bottom = rect.y + rect.height;

  if (handle.includes("w")) left += dx;
  if (handle.includes("e")) right += dx;
  if (handle.includes("n")) top += dy;
  if (handle.includes("s")) bottom += dy;

  const next: Rect = {
    x: Math.min(left, right),
    y: Math.min(top, bottom),
    width: Math.abs(right - left),
    height: Math.abs(bottom - top),
  };
  return clampToBounds(next, bounds);
}

export function isValidSelection(rect: Rect): boolean {
  return (
    rect.width >= SCREENSHOT_MIN_SELECTION_CSS_PX &&
    rect.height >= SCREENSHOT_MIN_SELECTION_CSS_PX
  );
}

/**
 * 归一化到 `[0,1]`：基于底图显示区域而非显示器物理尺寸（不变式 3），
 * 因此窗口尺寸与显示器分辨率有细微差异也不会错位。
 */
export function toNormalizedRect(
  rect: Rect,
  viewport: { width: number; height: number },
): NormRect {
  const width = Math.max(1, viewport.width);
  const height = Math.max(1, viewport.height);
  const x = Math.max(0, Math.min(1, rect.x / width));
  const y = Math.max(0, Math.min(1, rect.y / height));
  const w = Math.max(0, Math.min(1 - x, rect.width / width));
  const h = Math.max(0, Math.min(1 - y, rect.height / height));
  return { x, y, width: w, height: h };
}
