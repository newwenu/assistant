import { describe, expect, it } from "vitest";
import {
  VIEWPORT_DEFAULT,
  VIEWPORT_SCALE_MAX,
  VIEWPORT_SCALE_MIN,
  VIEWPORT_ZOOM_STEP,
  clampScale,
  panBy,
  resetViewport,
  screenToWorld,
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
});
