import { describe, expect, it } from "vitest";
import { anchorOf, edgeGeometryOf } from "@/features/canvas/geometry";
import type { CanvasBox } from "@/features/canvas/types";

const a: CanvasBox = { x: 0, y: 24, w: 132, h: 78 };
const b: CanvasBox = { x: 140, y: 24, w: 132, h: 78 };
const c: CanvasBox = { x: 420, y: 24, w: 132, h: 78 };
const d: CanvasBox = { x: 420, y: 110, w: 132, h: 78 };
const gutter = 16;

describe("anchorOf", () => {
  it("resolves the four sides from a box", () => {
    expect(anchorOf(a, "right")).toEqual({ x: 132, y: 63 });
    expect(anchorOf(a, "left")).toEqual({ x: 0, y: 63 });
    expect(anchorOf(a, "bottom")).toEqual({ x: 66, y: 102 });
    expect(anchorOf(a, "top")).toEqual({ x: 66, y: 24 });
  });
});

describe("edgeGeometryOf", () => {
  it("draws a horizontal edge from the right border to the left border", () => {
    const geometry = edgeGeometryOf(a, b, "horizontal", gutter);

    expect(geometry.segments).toEqual([{ x: 132, y: 62.5, w: 8, h: 1 }]);
    expect(geometry.arrow).toEqual({ x: 140, y: 63, direction: "right" });
  });

  it("draws a vertical edge from the bottom border to the top border", () => {
    const geometry = edgeGeometryOf(c, d, "vertical", gutter);

    expect(geometry.segments).toEqual([{ x: 485.5, y: 102, w: 1, h: 8 }]);
    expect(geometry.arrow).toEqual({ x: 486, y: 110, direction: "down" });
  });

  it("routes the outer track around the loop gutter", () => {
    const geometry = edgeGeometryOf(d, a, "outer-track", gutter);

    expect(geometry.segments).toEqual([
      { x: 551.5, y: 16, w: 1, h: 94 },
      { x: 0, y: 15.5, w: 552, h: 1 },
      { x: 0, y: 16, w: 1, h: 8 },
    ]);
    expect(geometry.arrow).toEqual({ x: 0, y: 16, direction: "left" });
  });

  it("derives the endpoints from the box anchors", () => {
    const horizontal = edgeGeometryOf(a, b, "horizontal", gutter);
    expect(horizontal.segments[0]).toEqual(
      expect.objectContaining({
        x: anchorOf(a, "right").x,
        y: anchorOf(a, "right").y - 0.5,
      }),
    );
    expect(horizontal.arrow).toEqual({
      x: anchorOf(b, "left").x,
      y: anchorOf(b, "left").y,
      direction: "right",
    });

    const vertical = edgeGeometryOf(c, d, "vertical", gutter);
    expect(vertical.segments[0]).toEqual(
      expect.objectContaining({
        x: anchorOf(c, "bottom").x - 0.5,
        y: anchorOf(c, "bottom").y,
      }),
    );
    expect(vertical.arrow).toEqual({
      x: anchorOf(d, "top").x,
      y: anchorOf(d, "top").y,
      direction: "down",
    });

    const outer = edgeGeometryOf(d, a, "outer-track", gutter);
    expect(outer.segments[0].x).toBe(anchorOf(d, "right").x - 0.5);
    expect(outer.segments[2].x).toBe(anchorOf(a, "left").x);
    expect(outer.arrow).toEqual({
      x: anchorOf(a, "left").x,
      y: gutter,
      direction: "left",
    });
  });

  it("keeps both ends of every segment on a box border within half a pixel", () => {
    const cases = [
      { route: "horizontal", from: a, to: b },
      { route: "vertical", from: c, to: d },
      { route: "outer-track", from: d, to: a },
    ] as const;

    const near = (value: number, target: number) =>
      Math.abs(value - target) <= 0.5;

    for (const { route, from, to } of cases) {
      const geometry = edgeGeometryOf(from, to, route, gutter);
      const first = geometry.segments[0];
      const last = geometry.segments[geometry.segments.length - 1];

      const firstOnFrom =
        near(first.x, from.x + from.w) || near(first.y, from.y + from.h);
      const lastOnTo =
        near(last.x, to.x) ||
        near(last.x + last.w, to.x) ||
        near(last.y, to.y) ||
        near(last.y + last.h, to.y);

      expect(firstOnFrom).toBe(true);
      expect(lastOnTo).toBe(true);
    }
  });

  it("never places a segment centre inside either endpoint box", () => {
    const geometry = edgeGeometryOf(a, b, "horizontal", gutter);
    const segment = geometry.segments[0];

    expect(segment.x).toBeGreaterThanOrEqual(a.x + a.w);
    expect(segment.x).toBeLessThanOrEqual(b.x);
    expect(segment.w).toBeGreaterThan(0);
  });
});
