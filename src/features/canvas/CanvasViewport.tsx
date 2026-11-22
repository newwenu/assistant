import { MinusOutlined, PlusOutlined, ReloadOutlined } from "@ant-design/icons";
import { Button } from "antd";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { CANVAS_METRICS } from "./layout";
import {
  panBy,
  resetViewport,
  zoomAt,
  type CanvasViewportState,
} from "./viewport";
import type {
  CanvasCardView,
  CanvasEdgeView,
  CanvasLabelView,
  CanvasBox,
  Point,
} from "./types";
import { CanvasCard } from "./CanvasCard";
import { CanvasEdge } from "./CanvasEdge";
import { CanvasLabel } from "./CanvasLabel";
import { CanvasLegend, type CanvasLegendGroup } from "./CanvasLegend";
import styles from "./canvas.module.css";

const ZOOM_IN_FACTOR = 1.1;
const ZOOM_OUT_FACTOR = 1 / 1.1;

/**
 * 视口状态跨实例保留（§9.1）：内容换了，视口仍然有意义；选中则随内容清空。
 * 只存一份模块级快照，不写 store、不进任何 DTO（N1 / N2）。
 */
let persistedViewport: CanvasViewportState = resetViewport();

export interface CanvasViewportProps {
  cards: readonly CanvasCardView[];
  edges?: readonly CanvasEdgeView[];
  labels?: readonly CanvasLabelView[];
  legendGroups?: readonly CanvasLegendGroup[];
  showFacilityLegend?: boolean;
  header?: ReactNode;
  selectedId?: string | null;
  onSelect?: (id: string | null) => void;
}

function boxesOf(cards: readonly CanvasCardView[]): Map<string, CanvasBox> {
  const boxes = new Map<string, CanvasBox>();
  for (const card of cards) boxes.set(card.id, card.box);
  return boxes;
}

export function CanvasViewport({
  cards,
  edges = [],
  labels = [],
  legendGroups,
  showFacilityLegend,
  header,
  selectedId,
  onSelect,
}: CanvasViewportProps) {
  const { t } = useTranslation();
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  const rectRef = useRef<DOMRect | null>(null);
  const dragRef = useRef<{
    pointerId: number;
    x: number;
    y: number;
    dx: number;
    dy: number;
  } | null>(null);
  const frameRef = useRef<number | null>(null);
  const [viewport, setViewport] = useState<CanvasViewportState>(
    () => persistedViewport,
  );
  const viewportRef = useRef<CanvasViewportState>(viewport);
  const [panning, setPanning] = useState(false);
  const [internalSelected, setInternalSelected] = useState<string | null>(null);
  const cardsRef = useRef(cards);

  const selected = selectedId === undefined ? internalSelected : selectedId;

  const commit = useCallback((next: CanvasViewportState) => {
    persistedViewport = next;
    viewportRef.current = next;
    setViewport(next);
  }, []);

  useEffect(() => {
    if (cardsRef.current === cards) return;
    cardsRef.current = cards;
    if (selectedId === undefined) setInternalSelected(null);
  }, [cards, selectedId]);

  const select = useCallback(
    (id: string | null) => {
      if (selectedId === undefined) setInternalSelected(id);
      onSelect?.(id);
    },
    [onSelect, selectedId],
  );

  /**
   * 视口矩形只在指针 / 滚轮 / 按钮处理器里读一次（§6.2）：不进渲染路径，
   * 也不参与任何几何推导；拖拽期间沿用同一份快照。
   */
  const readRect = useCallback((): DOMRect | null => {
    const surface = surfaceRef.current;
    if (surface === null) return null;
    const rect = surface.getBoundingClientRect();
    rectRef.current = rect;
    return rect;
  }, []);

  const surfaceCenter = useCallback((): Point | null => {
    const rect = readRect();
    if (rect === null) return null;
    return { x: rect.width / 2, y: rect.height / 2 };
  }, [readRect]);

  const zoomBy = useCallback(
    (factor: number) => {
      const anchor = surfaceCenter();
      if (anchor === null) return;
      commit(zoomAt(viewportRef.current, anchor, factor));
    },
    [commit, surfaceCenter],
  );

  const reset = useCallback(() => {
    commit(resetViewport());
  }, [commit]);

  useEffect(() => {
    const surface = surfaceRef.current;
    if (surface === null) return undefined;

    const handleWheel = (event: WheelEvent) => {
      event.preventDefault();
      const current = viewportRef.current;
      if (event.ctrlKey || event.metaKey) {
        const rect = rectRef.current ?? readRect();
        if (rect === null) return;
        const anchor = {
          x: event.clientX - rect.left,
          y: event.clientY - rect.top,
        };
        const factor = event.deltaY < 0 ? ZOOM_IN_FACTOR : ZOOM_OUT_FACTOR;
        commit(zoomAt(current, anchor, factor));
        return;
      }
      commit(
        event.shiftKey
          ? panBy(current, event.deltaY, 0)
          : panBy(current, 0, event.deltaY),
      );
    };

    surface.addEventListener("wheel", handleWheel, { passive: false });
    return () => surface.removeEventListener("wheel", handleWheel);
  }, [commit, readRect]);

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;
      const target = event.target;
      if (target instanceof Element && target.closest("[data-canvas-card]")) {
        return;
      }
      readRect();
      dragRef.current = {
        pointerId: event.pointerId,
        x: event.clientX,
        y: event.clientY,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
      setPanning(true);
    },
    [readRect],
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (drag === null || drag.pointerId !== event.pointerId) return;
      const dx = event.clientX - drag.x;
      const dy = event.clientY - drag.y;
      drag.x = event.clientX;
      drag.y = event.clientY;
      commit(panBy(viewportRef.current, dx, dy));
    },
    [commit],
  );

  const endDrag = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (drag === null || drag.pointerId !== event.pointerId) return;
    dragRef.current = null;
    rectRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    setPanning(false);
  }, []);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape") {
        select(null);
        return;
      }
      if (!event.ctrlKey && !event.metaKey) return;
      if (event.key === "+" || event.key === "=") {
        event.preventDefault();
        zoomBy(ZOOM_IN_FACTOR);
        return;
      }
      if (event.key === "-") {
        event.preventDefault();
        zoomBy(ZOOM_OUT_FACTOR);
        return;
      }
      if (event.key === "0") {
        event.preventDefault();
        reset();
      }
    },
    [reset, select, zoomBy],
  );

  const boxes = useMemo(() => boxesOf(cards), [cards]);
  const edgeNodes = useMemo(
    () =>
      edges.map((edge) => {
        const from = boxes.get(edge.from);
        const to = boxes.get(edge.to);
        if (from === undefined || to === undefined) return null;
        return (
          <CanvasEdge
            key={edge.id}
            edge={edge}
            from={from}
            to={to}
            gutter={CANVAS_METRICS.loopGutter}
          />
        );
      }),
    [boxes, edges],
  );
  const cardNodes = useMemo(
    () =>
      cards.map((card) => (
        <CanvasCard
          key={card.id}
          card={card}
          selected={card.id === selected}
          onSelect={select}
        />
      )),
    [cards, select, selected],
  );
  const labelNodes = useMemo(
    () => labels.map((label) => <CanvasLabel key={label.id} label={label} />),
    [labels],
  );

  const worldStyle = {
    transform: `translate(${viewport.x}px, ${viewport.y}px) scale(${viewport.scale})`,
  };

  const gridOrigin = `${viewport.x}px ${viewport.y}px`;
  const gridStyle = (step: number): CSSProperties => {
    const size = `${step * viewport.scale}px`;
    return {
      backgroundSize: `${size} ${size}, ${size} ${size}`,
      backgroundPosition: `${gridOrigin}, ${gridOrigin}`,
    };
  };
  const minorGridStyle = gridStyle(CANVAS_METRICS.gridMinor);
  const majorGridStyle = gridStyle(CANVAS_METRICS.gridMajor);

  return (
    <div className={styles.viewport}>
      <div className={styles.header}>
        <div className={styles.headerContent}>{header}</div>
        <div className={styles.zoomControls}>
          <span
            className={styles.zoomLevel}
            aria-label={t("canvas.zoom_level")}
          >
            {`×${viewport.scale.toFixed(2)}`}
          </span>
          <Button
            type="text"
            size="small"
            icon={<MinusOutlined />}
            aria-label={t("canvas.zoom_out")}
            onClick={() => zoomBy(ZOOM_OUT_FACTOR)}
          />
          <Button
            type="text"
            size="small"
            icon={<PlusOutlined />}
            aria-label={t("canvas.zoom_in")}
            onClick={() => zoomBy(ZOOM_IN_FACTOR)}
          />
          <Button
            type="text"
            size="small"
            icon={<ReloadOutlined />}
            aria-label={t("canvas.zoom_reset")}
            onClick={reset}
          />
        </div>
      </div>
      <div
        ref={surfaceRef}
        className={styles.surface}
        role="group"
        aria-label={t("canvas.viewport")}
        tabIndex={0}
        data-panning={panning}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onKeyDown={handleKeyDown}
      >
        <div className={styles.grid} aria-hidden="true">
          <div className={styles.gridMinor} style={minorGridStyle} />
          <div className={styles.gridMajor} style={majorGridStyle} />
        </div>
        <div className={styles.world} style={worldStyle}>
          <div className={styles.layer}>{edgeNodes}</div>
          <div className={styles.layer}>{cardNodes}</div>
          <div className={styles.layerOverlays}>{labelNodes}</div>
        </div>
      </div>
      <CanvasLegend
        groups={legendGroups}
        showFacilityGroups={showFacilityLegend}
      />
    </div>
  );
}
