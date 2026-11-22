import type { Point } from "./types";

export interface CanvasViewportState {
  x: number;
  y: number;
  scale: number;
}

export const VIEWPORT_DEFAULT: CanvasViewportState = { x: 0, y: 0, scale: 1 };

export const VIEWPORT_SCALE_MIN = 0.5;
export const VIEWPORT_SCALE_MAX = 2;
export const VIEWPORT_ZOOM_STEP = 1.1;

export function clampScale(scale: number): number {
  if (scale < VIEWPORT_SCALE_MIN) return VIEWPORT_SCALE_MIN;
  if (scale > VIEWPORT_SCALE_MAX) return VIEWPORT_SCALE_MAX;
  return scale;
}

export function screenToWorld(
  point: Point,
  viewport: CanvasViewportState,
): Point {
  return {
    x: (point.x - viewport.x) / viewport.scale,
    y: (point.y - viewport.y) / viewport.scale,
  };
}

export function worldToScreen(
  point: Point,
  viewport: CanvasViewportState,
): Point {
  return {
    x: point.x * viewport.scale + viewport.x,
    y: point.y * viewport.scale + viewport.y,
  };
}

export function panBy(
  viewport: CanvasViewportState,
  dx: number,
  dy: number,
): CanvasViewportState {
  return { x: viewport.x + dx, y: viewport.y + dy, scale: viewport.scale };
}

export function zoomAt(
  viewport: CanvasViewportState,
  anchor: Point,
  factor: number,
): CanvasViewportState {
  const scale = clampScale(viewport.scale * factor);
  if (scale === viewport.scale) {
    return viewport;
  }
  const ratio = scale / viewport.scale;
  return {
    x: anchor.x - (anchor.x - viewport.x) * ratio,
    y: anchor.y - (anchor.y - viewport.y) * ratio,
    scale,
  };
}

export function resetViewport(): CanvasViewportState {
  return { ...VIEWPORT_DEFAULT };
}
