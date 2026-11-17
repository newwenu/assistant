import { describe, it, expect } from "vitest";
import {
  clampToBounds,
  isValidSelection,
  normalizeDrag,
  resizeByHandle,
  toNormalizedRect,
} from "../selection";
import { SCREENSHOT_MIN_SELECTION_CSS_PX } from "@/config/constants";

const BOUNDS = { x: 0, y: 0, width: 1000, height: 800 };

describe("normalizeDrag", () => {
  it("builds a box for a top-left to bottom-right drag", () => {
    expect(normalizeDrag(10, 20, 210, 140)).toEqual({
      x: 10,
      y: 20,
      width: 200,
      height: 120,
    });
  });

  it("normalizes a reversed drag instead of producing negative sizes", () => {
    expect(normalizeDrag(210, 140, 10, 20)).toEqual({
      x: 10,
      y: 20,
      width: 200,
      height: 120,
    });
  });
});

describe("clampToBounds", () => {
  it("keeps an in-bounds rect untouched", () => {
    const rect = { x: 100, y: 50, width: 300, height: 200 };
    expect(clampToBounds(rect, BOUNDS)).toEqual(rect);
  });

  it("pulls an out-of-bounds rect back inside and caps its size", () => {
    expect(
      clampToBounds({ x: 900, y: 700, width: 400, height: 300 }, BOUNDS),
    ).toEqual({ x: 600, y: 500, width: 400, height: 300 });
  });

  it("caps a rect larger than the viewport", () => {
    expect(
      clampToBounds({ x: 0, y: 0, width: 2000, height: 1600 }, BOUNDS),
    ).toEqual({ x: 0, y: 0, width: 1000, height: 800 });
  });
});

describe("resizeByHandle", () => {
  const base = { x: 100, y: 100, width: 300, height: 200 };

  it("moves only the grabbed edge", () => {
    expect(resizeByHandle(base, "e", 50, 0, BOUNDS)).toEqual({
      x: 100,
      y: 100,
      width: 350,
      height: 200,
    });
    expect(resizeByHandle(base, "n", 0, 40, BOUNDS)).toEqual({
      x: 100,
      y: 140,
      width: 300,
      height: 160,
    });
  });

  it("swaps edges when the handle crosses the fixed side", () => {
    expect(resizeByHandle(base, "w", 400, 0, BOUNDS)).toEqual({
      x: 400,
      y: 100,
      width: 100,
      height: 200,
    });
  });

  it("clamps the result to the viewport", () => {
    expect(resizeByHandle(base, "se", 5000, 5000, BOUNDS)).toEqual({
      x: 0,
      y: 0,
      width: 1000,
      height: 800,
    });
  });
});

describe("isValidSelection", () => {
  it("rejects selections below the minimum edge", () => {
    const below = SCREENSHOT_MIN_SELECTION_CSS_PX - 1;
    expect(isValidSelection({ x: 0, y: 0, width: below, height: 100 })).toBe(
      false,
    );
    expect(isValidSelection({ x: 0, y: 0, width: 100, height: below })).toBe(
      false,
    );
  });

  it("accepts selections at the minimum edge", () => {
    expect(
      isValidSelection({
        x: 0,
        y: 0,
        width: SCREENSHOT_MIN_SELECTION_CSS_PX,
        height: SCREENSHOT_MIN_SELECTION_CSS_PX,
      }),
    ).toBe(true);
  });
});

describe("toNormalizedRect", () => {
  it("maps the display area to [0,1] ratios", () => {
    expect(
      toNormalizedRect({ x: 250, y: 100, width: 500, height: 400 }, BOUNDS),
    ).toEqual({ x: 0.25, y: 0.125, width: 0.5, height: 0.5 });
  });

  it("clips values outside the display area", () => {
    expect(
      toNormalizedRect({ x: 1200, y: 900, width: 300, height: 200 }, BOUNDS),
    ).toEqual({ x: 1, y: 1, width: 0, height: 0 });
  });

  it("never divides by zero on an unmeasured viewport", () => {
    expect(
      toNormalizedRect(
        { x: 0, y: 0, width: 10, height: 10 },
        { width: 0, height: 0 },
      ),
    ).toEqual({ x: 0, y: 0, width: 1, height: 1 });
  });
});
