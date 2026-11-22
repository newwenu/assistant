import { describe, expect, it } from "vitest";
import { CANVAS_METRICS, layoutTemplate } from "@/features/canvas/layout";
import { readRepoFile } from "@/test-utils/paths";

const entries = [
  { id: "demo-a", row: 0, column: 0 },
  { id: "demo-b", row: 0, column: 1 },
  { id: "demo-c", row: 1, column: 3 },
] as const;

describe("layoutTemplate", () => {
  it("places entries on the row/column template grid", () => {
    const layout = layoutTemplate(entries);

    expect(layout.boxes.get("demo-a")).toEqual({
      x: 0,
      y: 24,
      w: 132,
      h: 78,
    });
    expect(layout.boxes.get("demo-b")).toEqual({
      x: 140,
      y: 24,
      w: 132,
      h: 78,
    });
    expect(layout.boxes.get("demo-c")).toEqual({
      x: 420,
      y: 110,
      w: 132,
      h: 78,
    });
  });

  it("sizes the world from the farthest row and column", () => {
    const layout = layoutTemplate(entries);

    expect(layout.width).toBe(552);
    expect(layout.height).toBe(188);
  });

  it("returns an empty world for no entries", () => {
    const layout = layoutTemplate([]);

    expect(layout.boxes.size).toBe(0);
    expect(layout.width).toBe(0);
    expect(layout.height).toBe(0);
  });

  it("is idempotent for identical entries", () => {
    const first = layoutTemplate(entries);
    const second = layoutTemplate([...entries]);

    expect(Array.from(second.boxes.entries())).toEqual(
      Array.from(first.boxes.entries()),
    );
    expect(second.width).toBe(first.width);
    expect(second.height).toBe(first.height);
  });

  it("keys boxes by string id and ignores duplicate slots", () => {
    const layout = layoutTemplate([
      { id: "demo-x", row: 0, column: 0 },
      { id: "demo-y", row: 0, column: 0 },
    ]);

    expect(layout.boxes.get("demo-x")).toEqual(
      expect.objectContaining({ x: 0, y: 24, w: 132, h: 78 }),
    );
    expect(layout.boxes.get("demo-y")).toEqual(
      expect.objectContaining({ x: 0, y: 24, w: 132, h: 78 }),
    );
  });

  it("carries no measurement call in its source", () => {
    const source = readRepoFile("src/features/canvas/layout.ts");

    expect(source).not.toContain("getBoundingClientRect");
    expect(source).not.toContain("offsetWidth");
    expect(source).not.toContain("offsetHeight");
    expect(source).not.toContain("ResizeObserver");
  });

  it("keeps metrics numeric and positive", () => {
    for (const value of Object.values(CANVAS_METRICS)) {
      expect(typeof value).toBe("number");
      expect(value).toBeGreaterThan(0);
    }
  });
});
