import { describe, expect, it } from "vitest";
import {
  buildOrbCursorSvg,
  buildOrbCursorValue,
  cursorCacheKey,
} from "../orbCursor";

describe("buildOrbCursorSvg", () => {
  it("uses the provided accent color for the menu pointer", () => {
    expect(buildOrbCursorSvg("#3DD87E", "menu")).toContain("#3DD87E");
  });

  it("falls back to the default accent for invalid colors", () => {
    expect(buildOrbCursorSvg("red", "menu")).toContain("#1677ff");
    expect(buildOrbCursorSvg("#fff", "menu")).toContain("#1677ff");
    expect(buildOrbCursorSvg(null, "menu")).toContain("#1677ff");
  });

  it("draws the grab hand when idle and the gripping hand when active", () => {
    const idle = buildOrbCursorSvg("#1677ff", "idle");
    const active = buildOrbCursorSvg("#1677ff", "active");
    expect(idle).not.toBe(active);
    expect(idle.match(/<circle/g)).toBeNull();
    expect(active.match(/<circle/g)).toBeNull();
    expect(idle).toContain("#C96442");
    expect(active).toContain("#D97757");
    expect(idle).toContain("scale(0.82)");
    expect(idle).toContain("M16.25 3.5");
    expect(active).toContain("M16.25 9.5");
  });

  it("keeps the hand cursors independent from the accent color", () => {
    expect(buildOrbCursorSvg("#3DD87E", "idle")).toBe(
      buildOrbCursorSvg(null, "idle"),
    );
    expect(buildOrbCursorSvg("#3DD87E", "active")).toBe(
      buildOrbCursorSvg(null, "active"),
    );
  });

  it("draws a single pointed arrow path for the menu variant", () => {
    const menu = buildOrbCursorSvg("#1677ff", "menu");
    expect(menu.match(/<circle/g)).toBeNull();
    expect(menu.match(/<path/g)).toHaveLength(1);
    expect(menu).toContain("M3 3");
  });
});

describe("cursorCacheKey", () => {
  it("normalizes null and invalid colors onto the default accent entry", () => {
    expect(cursorCacheKey(null, "idle", 32)).toBe(
      cursorCacheKey(undefined, "idle", 32),
    );
    expect(cursorCacheKey("red", "idle", 32)).toBe(
      cursorCacheKey(null, "idle", 32),
    );
    expect(cursorCacheKey("#1677ff", "idle", 32)).toBe(
      cursorCacheKey(null, "idle", 32),
    );
  });

  it("separates variants, sizes and distinct colors", () => {
    expect(cursorCacheKey(null, "idle", 32)).not.toBe(
      cursorCacheKey(null, "menu", 32),
    );
    expect(cursorCacheKey(null, "idle", 32)).not.toBe(
      cursorCacheKey(null, "idle", 64),
    );
    expect(cursorCacheKey("#3DD87E", "menu", 32)).not.toBe(
      cursorCacheKey("#1677ff", "menu", 32),
    );
  });
});

describe("buildOrbCursorValue", () => {
  const png1x = "data:image/png;base64,AAA";
  const png2x = "data:image/png;base64,BBB";
  const center = { x: 16, y: 16 };

  it("prefers unprefixed image-set when supported", () => {
    const value = buildOrbCursorValue(
      png1x,
      png2x,
      "grab",
      center,
      "image-set",
    );
    expect(value).toContain(`image-set(url("${png1x}") 1x`);
    expect(value).toContain(`url("${png2x}") 2x`);
    expect(value.endsWith("16 16, grab")).toBe(true);
  });

  it("falls back to -webkit-image-set when unprefixed is unsupported", () => {
    const value = buildOrbCursorValue(
      png1x,
      png2x,
      "grab",
      center,
      "-webkit-image-set",
    );
    expect(value.startsWith(`-webkit-image-set(url("${png1x}") 1x`)).toBe(true);
    expect(value.endsWith("16 16, grab")).toBe(true);
  });

  it("falls back to a single bitmap when image-set is unsupported", () => {
    expect(buildOrbCursorValue(png1x, png2x, "grabbing", center, null)).toBe(
      `url("${png1x}") 16 16, grabbing`,
    );
  });

  it("falls back to a single bitmap when the 2x variant is missing", () => {
    expect(buildOrbCursorValue(png1x, null, "grab", center, "image-set")).toBe(
      `url("${png1x}") 16 16, grab`,
    );
  });

  it("places the hotspot at the arrow tip for the menu variant", () => {
    expect(
      buildOrbCursorValue(png1x, null, "default", { x: 3, y: 3 }, null),
    ).toBe(`url("${png1x}") 3 3, default`);
  });
});
