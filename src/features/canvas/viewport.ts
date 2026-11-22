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

/**
 * 复位目标（§6.3）：给了基线就回到基线，否则回到世界原点。
 * 基线由调用方通过 `initialViewport` 提供——内容需要让开浮层时，复位不能跳回原点。
 */
export function resetViewport(
  baseline?: CanvasViewportState,
): CanvasViewportState {
  return baseline === undefined ? { ...VIEWPORT_DEFAULT } : { ...baseline };
}

/**
 * 双指手势（§6.3）：`from` 两指的屏幕中点与间距保持世界点不动，
 * `to` 为同一对手指的新中点与间距。缩放与平移一次算出，不做二次修正。
 */
export function pinchTo(
  viewport: CanvasViewportState,
  fromCenter: Point,
  fromDistance: number,
  toCenter: Point,
  toDistance: number,
): CanvasViewportState {
  const ratio = fromDistance > 0 ? toDistance / fromDistance : 1;
  const scale = clampScale(viewport.scale * ratio);
  const anchor = screenToWorld(fromCenter, viewport);
  return {
    x: toCenter.x - anchor.x * scale,
    y: toCenter.y - anchor.y * scale,
    scale,
  };
}

export function gestureCenter(a: Point, b: Point): Point {
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
}

export function gestureDistance(a: Point, b: Point): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

export const WHEEL_LINE_UNIT = 16;
export const WHEEL_PAGE_UNIT = 100;
/**
 * 离散滚轮格子的灵敏度：一格（`deltaY = 100`）≈ `1.10`，与缩放按钮同量级。
 */
export const WHEEL_ZOOM_SENSITIVITY = 0.001;
/**
 * 触控板捏合的灵敏度：WebView2 把捏合编码成高频、小幅的 `ctrlKey` wheel
 * （单个事件常常只有几像素），用滚轮那一档会被指数映射压平，手感只有触屏捏合的
 * 一半左右，因此单独给一档更陡的常数。
 */
export const WHEEL_PINCH_SENSITIVITY = 0.002;
/** 判定「捏合」的增量阈值：低于此值视为触控板捏合的细分事件，而非滚轮格子。 */
export const WHEEL_PINCH_MAX_DELTA = 50;

/** 是否为触控板捏合产生的细分 wheel（而不是一整格滚轮）。 */
export function isPinchWheel(deltaY: number): boolean {
  return Math.abs(deltaY) < WHEEL_PINCH_MAX_DELTA;
}

/** 滚轮 / 触控板事件的增量视图：只取几何需要的三个字段，不绑定 DOM 类型。 */
export interface WheelDelta {
  deltaX: number;
  deltaY: number;
  deltaMode: number;
}

/**
 * 滚轮增量归一化（§6.3）：`deltaMode` 为行或页时换算成像素，
 * 之后所有手势只面对像素量级，不再区分鼠标滚轮与触控板。
 */
export function normalizeWheelDelta(delta: WheelDelta): Point {
  const unit =
    delta.deltaMode === 1
      ? WHEEL_LINE_UNIT
      : delta.deltaMode === 2
        ? WHEEL_PAGE_UNIT
        : 1;
  return { x: delta.deltaX * unit, y: delta.deltaY * unit };
}

/**
 * 捏合缩放因子（§6.3）：按指数映射，使连续的高频 wheel 事件平滑叠加，
 * 而不是每个事件固定乘 1.1——触控板捏合会在一秒内发几十个事件。
 * 细分事件走捏合灵敏度，整格滚轮走滚轮灵敏度，两条通道手感才对得上。
 */
export function wheelZoomFactor(deltaY: number): number {
  const sensitivity = isPinchWheel(deltaY)
    ? WHEEL_PINCH_SENSITIVITY
    : WHEEL_ZOOM_SENSITIVITY;
  return Math.exp(-deltaY * sensitivity);
}

/**
 * 缩放锚点兜底（§6.3）：触控板捏合走的是 WebView2 合成的 `ctrlKey` wheel，
 * 其 `clientX/clientY` 是光标位置而非指尖位置，常常落在视口矩形之外（甚至是 `0, 0`），
 * 拿它当锚点会把内容甩出屏幕；此时退回视口中心。
 */
export function anchorWithinViewport(
  point: Point,
  width: number,
  height: number,
): Point {
  if (point.x >= 0 && point.x <= width && point.y >= 0 && point.y <= height) {
    return point;
  }
  return { x: width / 2, y: height / 2 };
}
