import { memo, useMemo } from "react";
import { edgeGeometryOf } from "./geometry";
import type { ArrowDirection, EdgeSegment } from "./geometry";
import type { CanvasBox, CanvasEdgeView } from "./types";
import styles from "./canvas.module.css";

export interface CanvasEdgeProps {
  edge: CanvasEdgeView;
  from: CanvasBox;
  to: CanvasBox;
  gutter: number;
}

const ARROW_HALF_WIDTH = 5;
const ARROW_LENGTH = 7;

function arrowOffset(direction: ArrowDirection, x: number, y: number) {
  switch (direction) {
    case "right":
      return { left: x - ARROW_LENGTH, top: y - ARROW_HALF_WIDTH };
    case "left":
      return { left: x, top: y - ARROW_HALF_WIDTH };
    case "down":
      return { left: x - ARROW_HALF_WIDTH, top: y - ARROW_LENGTH };
  }
}

function segmentOrientation(segment: EdgeSegment): "horizontal" | "vertical" {
  return segment.w >= segment.h ? "horizontal" : "vertical";
}

/**
 * 边渲染器（§6.4 / §7.1）：坐标全部由两个盒子推出，颜色只有一个着色点
 * （`currentColor`），四相只由 `data-phase` 驱动（§7.4）。
 */
function CanvasEdgeImpl({ edge, from, to, gutter }: CanvasEdgeProps) {
  const geometry = useMemo(
    () => edgeGeometryOf(from, to, edge.route, gutter),
    [from, to, edge.route, gutter],
  );
  const showArrow = edge.phase === "passed" || edge.phase === "active";

  return (
    <div className={styles.edge} data-phase={edge.phase}>
      {geometry.segments.map((segment, index) => (
        <div
          key={`${edge.id}-segment-${index}`}
          className={styles.edgeLine}
          data-phase={edge.phase}
          data-orientation={segmentOrientation(segment)}
          style={{
            left: segment.x,
            top: segment.y,
            width: segment.w,
            height: segment.h,
          }}
        />
      ))}
      {showArrow && geometry.arrow !== null && (
        <div
          className={styles.edgeArrow}
          data-direction={geometry.arrow.direction}
          style={arrowOffset(
            geometry.arrow.direction,
            geometry.arrow.x,
            geometry.arrow.y,
          )}
        />
      )}
    </div>
  );
}

export const CanvasEdge = memo(CanvasEdgeImpl);
