import { describe, expect, it } from "vitest";
import type {
  CanvasBadgeView,
  CanvasCardView,
  CanvasEdgeView,
  CanvasLabelView,
} from "@/features/canvas/types";

type ForbiddenKey = "state" | "source" | "order" | "iteration" | "nodeType";

type Overlap<T> = Extract<keyof T, ForbiddenKey>;

const cardHasNoObservationField: Overlap<CanvasCardView> extends never
  ? true
  : false = true;
const edgeHasNoObservationField: Overlap<CanvasEdgeView> extends never
  ? true
  : false = true;
const labelHasNoObservationField: Overlap<CanvasLabelView> extends never
  ? true
  : false = true;
const badgeHasNoObservationField: Overlap<CanvasBadgeView> extends never
  ? true
  : false = true;

describe("canvas view model", () => {
  it("stays free of observation-domain fields", () => {
    expect(cardHasNoObservationField).toBe(true);
    expect(edgeHasNoObservationField).toBe(true);
    expect(labelHasNoObservationField).toBe(true);
    expect(badgeHasNoObservationField).toBe(true);
  });

  it("keys cards and edges by plain string id", () => {
    const card: CanvasCardView = {
      id: "demo-a",
      box: { x: 0, y: 0, w: 132, h: 78 },
      title: "demo",
    };
    const edge: CanvasEdgeView = {
      id: "demo-edge",
      from: "demo-a",
      to: "demo-a",
      route: "horizontal",
      phase: "untaken",
    };

    expect(typeof card.id).toBe("string");
    expect(typeof edge.from).toBe("string");
    expect(typeof edge.to).toBe("string");
  });
});
