import type { CanvasBox, EdgeRoute, Point } from "./types";

export type AnchorSide = "left" | "right" | "top" | "bottom";

export interface EdgeSegment {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type ArrowDirection = "left" | "right" | "down";

export interface EdgeArrow {
  x: number;
  y: number;
  direction: ArrowDirection;
}

export interface EdgeGeometry {
  segments: readonly EdgeSegment[];
  arrow: EdgeArrow | null;
}

export function anchorOf(box: CanvasBox, side: AnchorSide): Point {
  switch (side) {
    case "right":
      return { x: box.x + box.w, y: box.y + box.h / 2 };
    case "left":
      return { x: box.x, y: box.y + box.h / 2 };
    case "bottom":
      return { x: box.x + box.w / 2, y: box.y + box.h };
    case "top":
      return { x: box.x + box.w / 2, y: box.y };
  }
}

export function edgeGeometryOf(
  from: CanvasBox,
  to: CanvasBox,
  route: EdgeRoute,
  gutter: number,
): EdgeGeometry {
  const fromRight = from.x + from.w;
  const fromMidY = from.y + from.h / 2;
  const fromMidX = from.x + from.w / 2;
  const fromBottom = from.y + from.h;
  const toLeft = to.x;
  const toMidY = to.y + to.h / 2;
  const toMidX = to.x + to.w / 2;
  const toTop = to.y;

  switch (route) {
    case "horizontal":
      return {
        segments: [
          { x: fromRight, y: fromMidY - 0.5, w: toLeft - fromRight, h: 1 },
        ],
        arrow: { x: toLeft, y: toMidY, direction: "right" },
      };
    case "vertical":
      return {
        segments: [
          { x: fromMidX - 0.5, y: fromBottom, w: 1, h: toTop - fromBottom },
        ],
        arrow: { x: toMidX, y: toTop, direction: "down" },
      };
    case "outer-track":
      return {
        segments: [
          { x: fromRight - 0.5, y: gutter, w: 1, h: from.y - gutter },
          { x: toLeft, y: gutter - 0.5, w: fromRight - toLeft, h: 1 },
          { x: toLeft, y: gutter, w: 1, h: to.y - gutter },
        ],
        arrow: { x: toLeft, y: gutter, direction: "left" },
      };
  }
}
