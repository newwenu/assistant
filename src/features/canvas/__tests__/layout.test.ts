import { describe, expect, it } from "vitest";
import { layoutTemplate } from "@/features/canvas/layout";

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
});
