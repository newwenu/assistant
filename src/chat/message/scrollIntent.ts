/** 贴底判定阈值：程序性跟随与「重新贴底」都用它，流式期间也用它（I48 §6.3）。 */
export const FOLLOW_EPSILON = 2;

/** 判定用户已上翻的距底距离：超过它才算离开底部，也是未读累计门槛。 */
export const AWAY_THRESHOLD = 100;

/** 触顶加载更多：`scrollTop` 小于它时发起前插分页。 */
export const LOAD_MORE_TRIGGER = 100;

export interface ScrollMetrics {
  scrollTop: number;
  contentHeight: number;
  viewportHeight: number;
}

/** 位置记忆的最小形态：贴底优先于像素值，避免内容长度变化导致落点漂移。 */
export interface SavedPosition {
  top: number;
  /** 保存时是否贴底；贴底优先于像素值。 */
  pinned: boolean;
}

/** 可滚动上限；内容不足一屏时为 0。 */
export function maxTop(m: ScrollMetrics): number {
  return Math.max(0, m.contentHeight - m.viewportHeight);
}

/** 距底距离；内容不足一屏时为 0（视作贴底）。 */
export function distanceFromBottom(m: ScrollMetrics): number {
  return Math.max(0, maxTop(m) - m.scrollTop);
}

/**
 * 迟滞判定：贴底态要离底超过 `AWAY_THRESHOLD` 才脱离，
 * 离底态要回到 `FOLLOW_EPSILON` 才重新贴底，中间区间保持原状态不抖动。
 */
export function nextPinned(prev: boolean, distance: number): boolean {
  if (prev) return distance <= AWAY_THRESHOLD;
  return distance <= FOLLOW_EPSILON;
}

export interface EntryPlan {
  top: number;
  pinned: boolean;
}

/**
 * 进入会话的唯一落点决策（I48 §6.2）：内容不足一屏不产生滚动；
 * 保存值贴底优先；像素值按 `maxTop` 截断后重新判定是否贴底。
 */
export function planEntry(
  m: ScrollMetrics,
  saved: SavedPosition | null,
): EntryPlan {
  if (m.contentHeight <= m.viewportHeight) return { top: 0, pinned: true };
  const limit = maxTop(m);
  if (saved?.pinned) return { top: limit, pinned: true };
  if (saved) {
    const top = Math.min(Math.max(saved.top, 0), limit);
    return { top, pinned: top >= limit - FOLLOW_EPSILON };
  }
  return { top: limit, pinned: true };
}

export type ResizePlan =
  | { action: "none" }
  | { action: "follow" }
  | { action: "compensate"; top: number };

/**
 * 尺寸变化时的位移决策（I48 §6.2，按优先级）：
 * 前插补偿 > 未贴底不动 > 内容增高跟随 > 视口变小跟随。
 *
 * 第 4 条修的是「贴底用户凭空失去底部」：视口变小使 `maxTop` 变大，
 * 不跟随就会看到上方内容且按钮不出现。
 */
export function planResize(input: {
  metrics: ScrollMetrics;
  /** 内容高度增量（本次回调）。 */
  delta: number;
  /** 视口高度增量（本次回调）。 */
  viewportDelta: number;
  pinned: boolean;
  pendingPrepend: { height: number; top: number } | null;
}): ResizePlan {
  const { metrics, delta, viewportDelta, pinned, pendingPrepend } = input;
  if (pendingPrepend) {
    return {
      action: "compensate",
      top: pendingPrepend.top + (metrics.contentHeight - pendingPrepend.height),
    };
  }
  if (!pinned) return { action: "none" };
  if (delta > 0) return { action: "follow" };
  if (viewportDelta < 0) return { action: "follow" };
  return { action: "none" };
}
