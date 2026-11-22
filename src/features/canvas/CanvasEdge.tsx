import { memo, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { anchorOf, edgeGeometryOf } from "./geometry";
import type { AnchorSide, ArrowDirection, EdgeSegment } from "./geometry";
import type { CanvasBox, CanvasEdgeView } from "./types";
import styles from "./canvas.module.css";

/** 端点手柄的方位（I68 §6.3）：源 / 目标各一个，拖拽后落到目标卡片锚点。 */
export type EdgeEndpointSide = "source" | "target";

export interface CanvasEdgeProps {
  edge: CanvasEdgeView;
  from: CanvasBox;
  to: CanvasBox;
  gutter: number;
  selected: boolean;
  onSelect: (id: string) => void;
  onHandleDown: (
    id: string,
    side: EdgeEndpointSide,
    event: React.PointerEvent<HTMLButtonElement>,
  ) => void;
}

const ARROW_HALF_WIDTH = 5;
const ARROW_LENGTH = 7;
const HIT_PADDING = 6;
const HANDLE_SIZE = 12;

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

/** 命中区 = 路径包围盒外扩，细线也能点中（E1）。 */
function hitBoxOf(segments: readonly EdgeSegment[]): CanvasBox {
  const minX = Math.min(...segments.map((segment) => segment.x));
  const minY = Math.min(...segments.map((segment) => segment.y));
  const maxX = Math.max(...segments.map((segment) => segment.x + segment.w));
  const maxY = Math.max(...segments.map((segment) => segment.y + segment.h));
  return {
    x: minX - HIT_PADDING,
    y: minY - HIT_PADDING,
    w: maxX - minX + HIT_PADDING * 2,
    h: maxY - minY + HIT_PADDING * 2,
  };
}

function anchorSidesOf(route: CanvasEdgeView["route"]): {
  source: AnchorSide;
  target: AnchorSide;
} {
  return route === "vertical"
    ? { source: "bottom", target: "top" }
    : { source: "right", target: "left" };
}

/**
 * 连线渲染器（I68 §6.3 / §6.4）：视觉层只认相位与选中态；命中区、端点手柄是编辑层。
 * 端点坐标全部由 `anchorOf(box, side)` 推出，不读 DOM（A10）。
 */
function CanvasEdgeImpl({
  edge,
  from,
  to,
  gutter,
  selected,
  onSelect,
  onHandleDown,
}: CanvasEdgeProps) {
  const { t } = useTranslation();
  const geometry = useMemo(
    () => edgeGeometryOf(from, to, edge.route, gutter),
    [from, to, edge.route, gutter],
  );
  const hitBox = useMemo(() => hitBoxOf(geometry.segments), [geometry]);
  const sides = anchorSidesOf(edge.route);
  const sourcePoint = anchorOf(from, sides.source);
  const targetPoint = anchorOf(to, sides.target);
  const showArrow = edge.phase === "passed" || edge.phase === "active";

  return (
    <div
      className={styles.edge}
      data-phase={edge.phase}
      data-selected={selected}
    >
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
      <button
        type="button"
        className={styles.edgeHit}
        data-edge-id={edge.id}
        aria-pressed={selected}
        aria-label={`${t("canvas.edge.label")} ${edge.from} → ${edge.to}`}
        style={{
          left: hitBox.x,
          top: hitBox.y,
          width: hitBox.w,
          height: hitBox.h,
        }}
        onClick={() => onSelect(edge.id)}
      />
      {selected && (
        <>
          <button
            type="button"
            className={styles.edgeHandle}
            data-edge-handle="source"
            aria-label={`${t("canvas.edge.label")} source`}
            style={{
              left: sourcePoint.x - HANDLE_SIZE / 2,
              top: sourcePoint.y - HANDLE_SIZE / 2,
              width: HANDLE_SIZE,
              height: HANDLE_SIZE,
            }}
            onPointerDown={(event) => onHandleDown(edge.id, "source", event)}
          />
          <button
            type="button"
            className={styles.edgeHandle}
            data-edge-handle="target"
            aria-label={`${t("canvas.edge.label")} target`}
            style={{
              left: targetPoint.x - HANDLE_SIZE / 2,
              top: targetPoint.y - HANDLE_SIZE / 2,
              width: HANDLE_SIZE,
              height: HANDLE_SIZE,
            }}
            onPointerDown={(event) => onHandleDown(edge.id, "target", event)}
          />
        </>
      )}
    </div>
  );
}

export const CanvasEdge = memo(CanvasEdgeImpl);
