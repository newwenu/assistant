import { describe, expect, it } from "vitest";
import {
  AWAY_THRESHOLD,
  FOLLOW_EPSILON,
  distanceFromBottom,
  maxTop,
  nextPinned,
  planEntry,
  planResize,
} from "../scrollIntent";

/** 构造度量：`scrollTop` 缺省按贴底处理，便于表达「只看内容与视口」。 */
function metrics(
  contentHeight: number,
  viewportHeight: number,
  scrollTop = contentHeight - viewportHeight,
) {
  return { scrollTop, contentHeight, viewportHeight };
}

describe("maxTop / distanceFromBottom", () => {
  it("内容超过一屏时返回可滚动上限", () => {
    expect(maxTop(metrics(1000, 400))).toBe(600);
  });

  it("内容不足一屏时上限为 0，距底也为 0", () => {
    expect(maxTop(metrics(300, 400))).toBe(0);
    expect(distanceFromBottom(metrics(300, 400, 0))).toBe(0);
  });

  it("距底按 maxTop 与 scrollTop 之差计算", () => {
    expect(distanceFromBottom(metrics(1000, 400, 500))).toBe(100);
  });
});

describe("nextPinned：迟滞带", () => {
  it("贴底后距底 50px 仍为贴底", () => {
    expect(nextPinned(true, 50)).toBe(true);
  });

  it("贴底后距底超过阈值才脱离", () => {
    expect(nextPinned(true, AWAY_THRESHOLD + 1)).toBe(false);
    expect(nextPinned(true, AWAY_THRESHOLD)).toBe(true);
  });

  it("已上翻时距底 1px 重新贴底", () => {
    expect(nextPinned(false, FOLLOW_EPSILON - 1)).toBe(true);
  });

  it("已上翻时距底 50px 仍保持上翻（迟滞带内不抖动）", () => {
    expect(nextPinned(false, 50)).toBe(false);
  });
});

describe("planEntry", () => {
  it("内容超过一屏且无保存位置时落到底部", () => {
    expect(planEntry(metrics(1000, 400), null)).toEqual({
      top: 600,
      pinned: true,
    });
  });

  it("内容不足一屏时 top 为 0 且贴底", () => {
    expect(
      planEntry(metrics(300, 400, 0), { top: 120, pinned: false }),
    ).toEqual({ top: 0, pinned: true });
  });

  it("保存值超过 maxTop 时截断为 maxTop 并判为贴底", () => {
    expect(planEntry(metrics(1000, 400), { top: 900, pinned: false })).toEqual({
      top: 600,
      pinned: true,
    });
  });

  it("保存值贴底时无视内容长度直接落到底部", () => {
    expect(planEntry(metrics(5000, 400), { top: 0, pinned: true })).toEqual({
      top: 4600,
      pinned: true,
    });
  });

  it("保存值在区间内时原样恢复且保持上翻", () => {
    expect(planEntry(metrics(2000, 400), { top: 300, pinned: false })).toEqual({
      top: 300,
      pinned: false,
    });
  });

  it("保存值为负时钳到 0", () => {
    expect(planEntry(metrics(2000, 400), { top: -50, pinned: false })).toEqual({
      top: 0,
      pinned: false,
    });
  });
});

describe("planResize", () => {
  const base = {
    metrics: metrics(1000, 400),
    delta: 0,
    viewportDelta: 0,
    pinned: true,
    pendingPrepend: null,
  };

  it("前插待补偿时优先返回 compensate", () => {
    expect(
      planResize({
        ...base,
        metrics: metrics(1400, 400),
        delta: 400,
        pendingPrepend: { height: 1000, top: 120 },
      }),
    ).toEqual({ action: "compensate", top: 520 });
  });

  it("未贴底时内容增高不动", () => {
    expect(planResize({ ...base, pinned: false, delta: 200 })).toEqual({
      action: "none",
    });
  });

  it("贴底时内容增高跟随", () => {
    expect(planResize({ ...base, delta: 200 })).toEqual({ action: "follow" });
  });

  it("贴底时视口变小跟随", () => {
    expect(planResize({ ...base, viewportDelta: -80 })).toEqual({
      action: "follow",
    });
  });

  it("内容未增高且视口未变小不动", () => {
    expect(planResize({ ...base, delta: 0, viewportDelta: 0 })).toEqual({
      action: "none",
    });
    expect(planResize({ ...base, delta: -100, viewportDelta: 20 })).toEqual({
      action: "none",
    });
  });
});
