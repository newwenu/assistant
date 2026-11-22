import { describe, expect, it } from "vitest";
import {
  VIEWPORT_DEFAULT,
  VIEWPORT_SCALE_MAX,
  VIEWPORT_SCALE_MIN,
  VIEWPORT_ZOOM_STEP,
  WHEEL_LINE_UNIT,
  WHEEL_PAGE_UNIT,
  WHEEL_PINCH_SENSITIVITY,
  anchorWithinViewport,
  clampScale,
  gestureCenter,
  gestureDistance,
  isPinchWheel,
  normalizeWheelDelta,
  panBy,
  pinchTo,
  resetViewport,
  screenToWorld,
  wheelZoomFactor,
  worldToScreen,
  zoomAt,
} from "@/features/canvas/viewport";
import type { CanvasViewportState } from "@/features/canvas/viewport";

const anchors = [
  { x: 0, y: 0 },
  { x: 100, y: 80 },
  { x: 360, y: 240 },
];

const viewports: CanvasViewportState[] = [
  { x: 0, y: 0, scale: 1 },
  { x: 40, y: -25, scale: 1.4 },
  { x: -12, y: 30, scale: 0.6 },
];

describe("screenToWorld / worldToScreen", () => {
  it("round-trips a point for every scale and pan", () => {
    for (const viewport of viewports) {
      for (const point of anchors) {
        const world = screenToWorld(point, viewport);
        const screen = worldToScreen(world, viewport);

        expect(Math.abs(screen.x - point.x)).toBeLessThanOrEqual(0.5);
        expect(Math.abs(screen.y - point.y)).toBeLessThanOrEqual(0.5);
      }
    }
  });

  it("treats the default viewport as identity", () => {
    expect(screenToWorld({ x: 12, y: 34 }, VIEWPORT_DEFAULT)).toEqual({
      x: 12,
      y: 34,
    });
    expect(worldToScreen({ x: 12, y: 34 }, VIEWPORT_DEFAULT)).toEqual({
      x: 12,
      y: 34,
    });
  });
});

describe("zoomAt", () => {
  it("keeps the anchor's world point fixed on screen", () => {
    const viewport = { x: 10, y: 20, scale: 1 };
    const anchor = { x: 100, y: 80 };
    const before = screenToWorld(anchor, viewport);
    const zoomed = zoomAt(viewport, anchor, VIEWPORT_ZOOM_STEP);
    const after = screenToWorld(anchor, zoomed);

    expect(Math.abs(after.x - before.x)).toBeLessThanOrEqual(0.5);
    expect(Math.abs(after.y - before.y)).toBeLessThanOrEqual(0.5);
  });

  it("clamps scale to the declared range", () => {
    expect(clampScale(0.2)).toBe(VIEWPORT_SCALE_MIN);
    expect(clampScale(3)).toBe(VIEWPORT_SCALE_MAX);

    const atMax = { x: 0, y: 0, scale: VIEWPORT_SCALE_MAX };
    expect(zoomAt(atMax, { x: 0, y: 0 }, VIEWPORT_ZOOM_STEP)).toBe(atMax);

    const atMin = { x: 0, y: 0, scale: VIEWPORT_SCALE_MIN };
    expect(zoomAt(atMin, { x: 0, y: 0 }, 1 / VIEWPORT_ZOOM_STEP)).toBe(atMin);
  });
});

describe("panBy", () => {
  it("adds the delta without touching the scale", () => {
    const viewport = { x: 10, y: 20, scale: 1.4 };

    expect(panBy(viewport, 30, -12)).toEqual({ x: 40, y: 8, scale: 1.4 });
  });
});

describe("resetViewport", () => {
  it("returns a fresh default state", () => {
    const reset = resetViewport();

    expect(reset).toEqual(VIEWPORT_DEFAULT);
    expect(reset).not.toBe(VIEWPORT_DEFAULT);
  });

  it("returns a copy of the caller baseline when one is given", () => {
    const baseline = { x: 48, y: 72, scale: 1 };
    const reset = resetViewport(baseline);

    expect(reset).toEqual(baseline);
    expect(reset).not.toBe(baseline);
  });
});

describe("gesture helpers", () => {
  it("derives the midpoint and spread of two pointers", () => {
    const a = { x: 10, y: 20 };
    const b = { x: 40, y: 60 };

    expect(gestureCenter(a, b)).toEqual({ x: 25, y: 40 });
    expect(gestureDistance(a, b)).toBe(50);
    expect(gestureDistance(a, a)).toBe(0);
  });
});

describe("pinchTo", () => {
  it("keeps the world point under the two-finger midpoint fixed", () => {
    const viewport = { x: 0, y: 0, scale: 1 };
    const fromCenter = { x: 200, y: 150 };
    const toCenter = { x: 260, y: 190 };
    const before = screenToWorld(fromCenter, viewport);

    const next = pinchTo(viewport, fromCenter, 100, toCenter, 200);
    const after = screenToWorld(toCenter, next);

    expect(next.scale).toBe(2);
    expect(Math.abs(after.x - before.x)).toBeLessThanOrEqual(0.5);
    expect(Math.abs(after.y - before.y)).toBeLessThanOrEqual(0.5);
  });

  it("pans when the spread is unchanged", () => {
    const viewport = { x: 10, y: 20, scale: 1.4 };
    const next = pinchTo(
      viewport,
      { x: 100, y: 100 },
      80,
      { x: 130, y: 70 },
      80,
    );

    expect(next.scale).toBe(1.4);
    expect(next.x).toBeCloseTo(40);
    expect(next.y).toBeCloseTo(-10);
  });

  it("clamps the scale and keeps the anchor fixed at the limit", () => {
    const viewport = { x: 0, y: 0, scale: VIEWPORT_SCALE_MAX };
    const fromCenter = { x: 120, y: 90 };
    const before = screenToWorld(fromCenter, viewport);

    const next = pinchTo(viewport, fromCenter, 50, fromCenter, 200);
    const after = screenToWorld(fromCenter, next);

    expect(next.scale).toBe(VIEWPORT_SCALE_MAX);
    expect(Math.abs(after.x - before.x)).toBeLessThanOrEqual(0.5);
    expect(Math.abs(after.y - before.y)).toBeLessThanOrEqual(0.5);
  });

  it("falls back to the current scale when the spread starts at zero", () => {
    const viewport = { x: 0, y: 0, scale: 1 };

    expect(pinchTo(viewport, { x: 0, y: 0 }, 0, { x: 0, y: 0 }, 0)).toEqual(
      viewport,
    );
  });
});

describe("normalizeWheelDelta", () => {
  it("keeps pixel deltas as-is", () => {
    expect(
      normalizeWheelDelta({ deltaX: 12, deltaY: -30, deltaMode: 0 }),
    ).toEqual({ x: 12, y: -30 });
  });

  it("converts line deltas to pixels", () => {
    expect(normalizeWheelDelta({ deltaX: 0, deltaY: 3, deltaMode: 1 })).toEqual(
      {
        x: 0,
        y: 3 * WHEEL_LINE_UNIT,
      },
    );
  });

  it("converts page deltas to pixels", () => {
    expect(normalizeWheelDelta({ deltaX: 0, deltaY: 2, deltaMode: 2 })).toEqual(
      {
        x: 0,
        y: 2 * WHEEL_PAGE_UNIT,
      },
    );
  });

  it("preserves both axes so trackpad two-finger pan works diagonally", () => {
    expect(normalizeWheelDelta({ deltaX: 5, deltaY: 8, deltaMode: 0 })).toEqual(
      { x: 5, y: 8 },
    );
  });
});

describe("wheelZoomFactor", () => {
  it("is neutral for a zero delta", () => {
    expect(wheelZoomFactor(0)).toBe(1);
  });

  it("zooms in on upward deltas and out on downward deltas", () => {
    expect(wheelZoomFactor(-100)).toBeGreaterThan(1);
    expect(wheelZoomFactor(100)).toBeLessThan(1);
  });

  it("is symmetric in log space, so a gesture can be undone", () => {
    expect(wheelZoomFactor(-100) * wheelZoomFactor(100)).toBeCloseTo(1, 10);
  });

  it("gives a mouse notch a step close to the button step", () => {
    expect(wheelZoomFactor(-100)).toBeCloseTo(VIEWPORT_ZOOM_STEP, 1);
  });

  it("uses the steeper pinch sensitivity for small trackpad deltas", () => {
    expect(Math.log(wheelZoomFactor(-10))).toBeCloseTo(
      10 * WHEEL_PINCH_SENSITIVITY,
      6,
    );
    expect(wheelZoomFactor(-10)).toBeGreaterThan(Math.exp(10 * 0.001));
  });
});

describe("isPinchWheel", () => {
  it("treats small high-frequency deltas as pinch", () => {
    expect(isPinchWheel(-8)).toBe(true);
    expect(isPinchWheel(30)).toBe(true);
  });

  it("treats a discrete wheel notch as a scroll step", () => {
    expect(isPinchWheel(-100)).toBe(false);
    expect(isPinchWheel(120)).toBe(false);
  });
});

describe("anchorWithinViewport", () => {
  it("keeps an anchor that lies inside the viewport rect", () => {
    expect(anchorWithinViewport({ x: 120, y: 80 }, 800, 600)).toEqual({
      x: 120,
      y: 80,
    });
  });

  it("falls back to the viewport center when the anchor is outside", () => {
    expect(anchorWithinViewport({ x: -40, y: 700 }, 800, 600)).toEqual({
      x: 400,
      y: 300,
    });
  });

  it("keeps the top-left corner as a valid anchor", () => {
    expect(anchorWithinViewport({ x: 0, y: 0 }, 800, 600)).toEqual({
      x: 0,
      y: 0,
    });
  });
});
