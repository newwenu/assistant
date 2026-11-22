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
  const source = anchorOf(from, "right");
  const sourceBottom = anchorOf(from, "bottom");
  const target = anchorOf(to, "left");
  const targetTop = anchorOf(to, "top");

  switch (route) {
    case "horizontal":
      return {
        segments: [
          { x: source.x, y: source.y - 0.5, w: target.x - source.x, h: 1 },
        ],
        arrow: { x: target.x, y: target.y, direction: "right" },
      };
    case "vertical":
      return {
        segments: [
          {
            x: sourceBottom.x - 0.5,
            y: sourceBottom.y,
            w: 1,
            h: targetTop.y - sourceBottom.y,
          },
        ],
        arrow: { x: targetTop.x, y: targetTop.y, direction: "down" },
      };
    case "outer-track":
      return {
        segments: [
          { x: source.x - 0.5, y: gutter, w: 1, h: from.y - gutter },
          { x: target.x, y: gutter - 0.5, w: source.x - target.x, h: 1 },
          { x: target.x, y: gutter, w: 1, h: to.y - gutter },
        ],
        arrow: { x: target.x, y: gutter, direction: "left" },
      };
  }
}
