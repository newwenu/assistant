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
import { anchorOf } from "./geometry";
import {
  anchorWithinViewport,
  gestureCenter,
  gestureDistance,
  normalizeWheelDelta,
  panBy,
  pinchTo,
  resetViewport,
  wheelZoomFactor,
  zoomAt,
  type CanvasViewportState,
} from "./viewport";
import type {
  CanvasCardView,
  CanvasEdgeView,
  CanvasLabelView,
  CanvasBox,
  CanvasPortSide,
  Point,
} from "./types";
import { CanvasCard } from "./CanvasCard";
import { CanvasEdge, type EdgeEndpointSide } from "./CanvasEdge";
import { CanvasLabel } from "./CanvasLabel";
import { CanvasLegend, type CanvasLegendGroup } from "./CanvasLegend";
import { CARD_KIND_MIME } from "./CanvasPalette";
import styles from "./canvas.module.css";

const ZOOM_IN_FACTOR = 1.1;
const ZOOM_OUT_FACTOR = 1 / 1.1;
const WHEEL_PAN_FACTOR = 0.5;
const PINCH_POINTER_COUNT = 2;
/** 拖拽与点击的分界（§7.8）：位移超过 3px 视为拖拽，未超过视为点击。 */
const DRAG_THRESHOLD = 3;
/** 拖拽结束后抑制同一次手势合成的 click，避免「拖一下就选中」。 */
const CLICK_SUPPRESS_MS = 400;
/**
 * 只有真正移动焦点的按键才算「键盘导航」：按 Shift（横向平移）、Ctrl+0（复位）、
 * Esc（清选中）时浏览器同样会把当前焦点元素判成 `:focus-visible`，在指针模态下
 * 会凭空长出一圈焦点框，故只认 `Tab`（含 Shift+Tab）。
 */
const KEYBOARD_NAV_KEYS: ReadonlySet<string> = new Set(["Tab"]);

interface CanvasPointer {
  x: number;
  y: number;
  onCard: boolean;
}

interface CardDragState {
  pointerId: number;
  id: string;
  startX: number;
  startY: number;
  box: CanvasBox;
  moved: boolean;
}

interface ReconnectState {
  pointerId: number;
  id: string;
  side: EdgeEndpointSide;
}

interface ConnectState {
  pointerId: number;
  from: string;
  side: CanvasPortSide;
  start: Point;
}

interface EdgeEndpoints {
  from: string;
  to: string;
}

/**
 * 视口状态跨实例保留（§9.1）：内容换了，视口仍然有意义；选中则随内容清空。
 * 只存一份模块级快照，不写 store、不进任何 DTO（N1 / N2）。
 */
let persistedViewport: CanvasViewportState | null = null;

export interface CanvasPlaceRequest {
  kind: string;
  /** 递增令牌：变化即请求在视口中心落一张卡片。 */
  token: number;
}

export interface CanvasEditMove {
  id: string;
  x: number;
  y: number;
}

export interface CanvasEditEdge {
  id: string;
  from: string;
  to: string;
}

export interface CanvasEditResult {
  moved: readonly CanvasEditMove[];
  removed: readonly string[];
  addedEdges: readonly CanvasEditEdge[];
  rewired: readonly CanvasEditEdge[];
}

export interface CanvasViewportProps {
  cards: readonly CanvasCardView[];
  edges?: readonly CanvasEdgeView[];
  labels?: readonly CanvasLabelView[];
  legendGroups?: readonly CanvasLegendGroup[];
  showFacilityLegend?: boolean;
  header?: ReactNode;
  /** 首次挂载时的视口；一旦用户操作过，视口按 §9.1 跨实例保留。 */
  initialViewport?: CanvasViewportState;
  selectedId?: string | null;
  onSelect?: (id: string | null) => void;
  /** 外轨连线的绕行间距；默认取观测栅格度量，画布预设传 `CARD_METRICS.loopGutter`。 */
  gutter?: number;
  /** 放置栏落下的卡片（编辑层会话状态，由调用方持有；不持久化）。 */
  placedCards?: readonly CanvasCardView[];
  /** 从放置栏拖入画布 / 放置栏回车：把类别交给调用方生成卡片。 */
  onDropCard?: (kind: string, point: Point) => void;
  /** 放置栏回车的落点请求：视口中心，由视口算好世界坐标再回调。 */
  placeRequest?: CanvasPlaceRequest | null;
  /** 放置栏拖入时的落点预览框尺寸；省略则取观测栅格单元。 */
  dropPreviewSize?: { w: number; h: number };
  onEditResult?: (result: CanvasEditResult) => void;
}

function boxesOf(cards: readonly CanvasCardView[]): Map<string, CanvasBox> {
  const boxes = new Map<string, CanvasBox>();
  for (const card of cards) boxes.set(card.id, card.box);
  return boxes;
}

function cardIdAtPoint(clientX: number, clientY: number): string | null {
  if (typeof document.elementFromPoint !== "function") return null;
  const element = document.elementFromPoint(clientX, clientY);
  if (!(element instanceof Element)) return null;
  const card = element.closest("[data-canvas-card]");
  if (!(card instanceof HTMLElement)) return null;
  return card.dataset.cardId ?? null;
}

export function CanvasViewport({
  cards,
  edges = [],
  labels = [],
  legendGroups,
  showFacilityLegend,
  header,
  initialViewport,
  selectedId,
  onSelect,
  gutter = CANVAS_METRICS.loopGutter,
  placedCards = [],
  onDropCard,
  placeRequest = null,
  dropPreviewSize = { w: CANVAS_METRICS.cellW, h: CANVAS_METRICS.cellH },
  onEditResult,
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
  const pointersRef = useRef<Map<number, CanvasPointer>>(new Map());
  const pinchRef = useRef<{
    center: Point;
    distance: number;
  } | null>(null);
  const wheelRef = useRef<{
    dx: number;
    dy: number;
    zoomFactor: number;
    anchor: Point | null;
  } | null>(null);
  const frameRef = useRef<number | null>(null);
  const [viewport, setViewport] = useState<CanvasViewportState>(
    () => persistedViewport ?? initialViewport ?? resetViewport(),
  );
  const viewportRef = useRef<CanvasViewportState>(viewport);
  const [panning, setPanning] = useState(false);
  const [internalSelected, setInternalSelected] = useState<string | null>(null);
  /**
   * 输入模态（指针 / 键盘）：只用来决定焦点框是否显示，不进任何几何与投影。
   * 指针模态下抑制 `:focus-visible`，避免「点过画布后按 Shift 横向平移」时
   * 视口或卡片上冒出伪焦点框；真正的键盘导航（Tab / 方向键等）仍保留 2px 描边。
   */
  const [inputModality, setInputModality] = useState<"pointer" | "keyboard">(
    "pointer",
  );

  /**
   * 编辑层会话状态（§5.1 / §6.1）：只存位移、删除与端点改写，不回写预设、不持久化。
   * 内容仍由 `cards` / `edges` 提供，故投影刷新不会覆盖用户的编辑结果。
   */
  const [movedBoxes, setMovedBoxes] = useState<ReadonlyMap<string, CanvasBox>>(
    new Map(),
  );
  const [removedCards, setRemovedCards] = useState<ReadonlySet<string>>(
    new Set(),
  );
  const [removedEdges, setRemovedEdges] = useState<ReadonlySet<string>>(
    new Set(),
  );
  const [edgeEndpoints, setEdgeEndpoints] = useState<
    ReadonlyMap<string, EdgeEndpoints>
  >(new Map());
  const [addedEdges, setAddedEdges] = useState<readonly CanvasEdgeView[]>([]);
  const [connectPreview, setConnectPreview] = useState<{
    start: Point;
    point: Point;
  } | null>(null);
  /** 放置栏拖入中的落点预览（世界坐标左上角；松手即清空）。 */
  const [dropPoint, setDropPoint] = useState<Point | null>(null);

  const cardDragRef = useRef<CardDragState | null>(null);
  const reconnectRef = useRef<ReconnectState | null>(null);
  const connectRef = useRef<ConnectState | null>(null);
  const suppressClickUntilRef = useRef(0);
  const cardsRef = useRef(cards);
  const boxesRef = useRef<Map<string, CanvasBox>>(new Map());
  const onDropCardRef = useRef(onDropCard);
  onDropCardRef.current = onDropCard;
  const onEditResultRef = useRef(onEditResult);
  onEditResultRef.current = onEditResult;
  const edgeEndpointsRef = useRef(edgeEndpoints);
  edgeEndpointsRef.current = edgeEndpoints;
  const edgesRef = useRef(edges);
  edgesRef.current = edges;
  const addedEdgesRef = useRef(addedEdges);
  addedEdgesRef.current = addedEdges;

  const emitEditResult = useCallback((result: CanvasEditResult) => {
    onEditResultRef.current?.(result);
  }, []);

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

  /** 拖拽结束后，同一次手势的 click 不应再改变选中（§7.8）。 */
  const selectCard = useCallback(
    (id: string) => {
      if (performance.now() < suppressClickUntilRef.current) return;
      select(id);
    },
    [select],
  );

  /** 任何指针按下都回到指针模态（含浮层控件，故挂在根节点捕获阶段）。 */
  const markPointerModality = useCallback(() => {
    setInputModality("pointer");
  }, []);

  /** 只有 `Tab` / `Shift+Tab` 才算键盘导航；修饰键与快捷键不切换模态。 */
  const markKeyboardModality = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (!KEYBOARD_NAV_KEYS.has(event.key)) return;
      setInputModality("keyboard");
    },
    [],
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

  /** 屏幕坐标 → 世界坐标（编辑层位移用；不进渲染路径）。 */
  const worldPointOf = useCallback(
    (clientX: number, clientY: number): Point | null => {
      const rect = rectRef.current ?? readRect();
      if (rect === null) return null;
      const { x, y, scale } = viewportRef.current;
      return {
        x: (clientX - rect.left - x) / scale,
        y: (clientY - rect.top - y) / scale,
      };
    },
    [readRect],
  );

  const zoomBy = useCallback(
    (factor: number) => {
      const anchor = surfaceCenter();
      if (anchor === null) return;
      commit(zoomAt(viewportRef.current, anchor, factor));
    },
    [commit, surfaceCenter],
  );

  /**
   * 复位回到调用方给的基线（`initialViewport`），而不是硬编码的世界原点：
   * 示例页把内容让开浮层后，复位若跳回 `(0, 0)` 会与首次进入的位置不一致。
   */
  const reset = useCallback(() => {
    commit(resetViewport(initialViewport));
  }, [commit, initialViewport]);

  /**
   * 单帧合帧（§12.6 允许的唯一 rAF 用法）：拖拽、双指、滚轮共用同一帧回调。
   * 三者按「双指 → 滚轮 → 单指拖拽」顺序叠加到同一份 next 状态，
   * 一帧最多一次提交，高回报率触控板不会打穿 React 渲染。
   */
  const flushGesture = useCallback(() => {
    frameRef.current = null;
    let next = viewportRef.current;
    let changed = false;

    const pinch = pinchRef.current;
    if (pinch !== null) {
      const points = Array.from(pointersRef.current.values());
      if (points.length === PINCH_POINTER_COUNT) {
        const center = gestureCenter(points[0], points[1]);
        const distance = gestureDistance(points[0], points[1]);
        pinchRef.current = { center, distance };
        next = pinchTo(next, pinch.center, pinch.distance, center, distance);
        changed = true;
      }
    }

    const wheel = wheelRef.current;
    if (wheel !== null) {
      wheelRef.current = null;
      if (wheel.anchor !== null && wheel.zoomFactor !== 1) {
        next = zoomAt(next, wheel.anchor, wheel.zoomFactor);
        changed = true;
      }
      if (wheel.dx !== 0 || wheel.dy !== 0) {
        next = panBy(next, wheel.dx, wheel.dy);
        changed = true;
      }
    }

    const drag = dragRef.current;
    if (drag !== null && (drag.dx !== 0 || drag.dy !== 0)) {
      next = panBy(next, drag.dx, drag.dy);
      drag.dx = 0;
      drag.dy = 0;
      changed = true;
    }

    if (changed) commit(next);
  }, [commit]);

  const scheduleGesture = useCallback(() => {
    if (frameRef.current !== null) return;
    frameRef.current = requestAnimationFrame(flushGesture);
  }, [flushGesture]);

  useEffect(
    () => () => {
      if (frameRef.current !== null) cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    },
    [],
  );

  useEffect(() => {
    const surface = surfaceRef.current;
    if (surface === null) return undefined;

    const accumulateWheel = (): {
      dx: number;
      dy: number;
      zoomFactor: number;
      anchor: Point | null;
    } => {
      const pending = wheelRef.current ?? {
        dx: 0,
        dy: 0,
        zoomFactor: 1,
        anchor: null,
      };
      wheelRef.current = pending;
      return pending;
    };

    const handleWheel = (event: WheelEvent) => {
      event.preventDefault();
      const delta = normalizeWheelDelta(event);

      if (event.ctrlKey || event.metaKey) {
        const rect = readRect();
        if (rect === null) return;
        const pending = accumulateWheel();
        pending.zoomFactor *= wheelZoomFactor(delta.y);
        pending.anchor = anchorWithinViewport(
          { x: event.clientX - rect.left, y: event.clientY - rect.top },
          rect.width,
          rect.height,
        );
        scheduleGesture();
        return;
      }

      const pending = accumulateWheel();
      if (event.shiftKey) {
        pending.dx -= delta.y * WHEEL_PAN_FACTOR;
      } else {
        pending.dx -= delta.x * WHEEL_PAN_FACTOR;
        pending.dy -= delta.y * WHEEL_PAN_FACTOR;
      }
      scheduleGesture();
    };

    surface.addEventListener("wheel", handleWheel, { passive: false });
    return () => surface.removeEventListener("wheel", handleWheel);
  }, [readRect, scheduleGesture]);

  const beginPan = useCallback(
    (pointerId: number, x: number, y: number) => {
      readRect();
      dragRef.current = { pointerId, x, y, dx: 0, dy: 0 };
      setPanning(true);
    },
    [readRect],
  );

  const beginPinch = useCallback(() => {
    const points = Array.from(pointersRef.current.values());
    if (points.length !== PINCH_POINTER_COUNT) return;
    dragRef.current = null;
    cardDragRef.current = null;
    pinchRef.current = {
      center: gestureCenter(points[0], points[1]),
      distance: gestureDistance(points[0], points[1]),
    };
    setPanning(true);
  }, []);

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (event.pointerType === "mouse" && event.button !== 0) return;
      if (pinchRef.current !== null) return;

      const target = event.target;
      const element = target instanceof Element ? target : null;
      const cardElement = element?.closest("[data-canvas-card]") ?? null;
      const onCard = cardElement !== null;
      pointersRef.current.set(event.pointerId, {
        x: event.clientX,
        y: event.clientY,
        onCard,
      });
      // 卡片上的指针也捕获到视口：点击仍由 click 合成事件决定（target 取
      // pointerdown/up 的共同祖先），但指针移出视口后不会留下陈旧条目。
      event.currentTarget.setPointerCapture(event.pointerId);

      if (pointersRef.current.size === PINCH_POINTER_COUNT) {
        beginPinch();
        return;
      }

      if (onCard && cardElement instanceof HTMLElement) {
        const id = cardElement.dataset.cardId;
        const box = id === undefined ? undefined : boxesRef.current.get(id);
        if (id !== undefined && box !== undefined) {
          readRect();
          cardDragRef.current = {
            pointerId: event.pointerId,
            id,
            startX: event.clientX,
            startY: event.clientY,
            box,
            moved: false,
          };
        }
        return;
      }

      // 连线命中区 / 端点手柄是编辑对象，不参与视口平移。
      if (element?.closest("[data-edge-id], [data-edge-handle]") != null) {
        return;
      }

      beginPan(event.pointerId, event.clientX, event.clientY);
    },
    [beginPan, beginPinch, readRect],
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const pointer = pointersRef.current.get(event.pointerId);

      if (
        connectRef.current !== null &&
        connectRef.current.pointerId === event.pointerId
      ) {
        const point = worldPointOf(event.clientX, event.clientY);
        if (point !== null) {
          setConnectPreview({ start: connectRef.current.start, point });
        }
        if (pointer !== undefined) {
          pointer.x = event.clientX;
          pointer.y = event.clientY;
        }
        return;
      }

      if (pointer === undefined) return;
      pointer.x = event.clientX;
      pointer.y = event.clientY;

      if (pinchRef.current !== null) {
        scheduleGesture();
        return;
      }

      const cardDrag = cardDragRef.current;
      if (cardDrag !== null && cardDrag.pointerId === event.pointerId) {
        const dx =
          (event.clientX - cardDrag.startX) / viewportRef.current.scale;
        const dy =
          (event.clientY - cardDrag.startY) / viewportRef.current.scale;
        if (
          !cardDrag.moved &&
          Math.hypot(
            event.clientX - cardDrag.startX,
            event.clientY - cardDrag.startY,
          ) > DRAG_THRESHOLD
        ) {
          cardDrag.moved = true;
        }
        if (cardDrag.moved) {
          const box: CanvasBox = {
            x: cardDrag.box.x + dx,
            y: cardDrag.box.y + dy,
            w: cardDrag.box.w,
            h: cardDrag.box.h,
          };
          setMovedBoxes((prev) => new Map(prev).set(cardDrag.id, box));
        }
        return;
      }

      const drag = dragRef.current;
      if (drag === null || drag.pointerId !== event.pointerId) return;
      drag.dx += event.clientX - drag.x;
      drag.dy += event.clientY - drag.y;
      drag.x = event.clientX;
      drag.y = event.clientY;
      scheduleGesture();
    },
    [scheduleGesture, worldPointOf],
  );

  /** 端点手柄拖拽落到另一张卡片：只改写端点，不校验类型、不重算区带（§6.4 E5）。 */
  const finishReconnect = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const reconnect = reconnectRef.current;
      if (reconnect === null) return;
      reconnectRef.current = null;
      const targetId = cardIdAtPoint(event.clientX, event.clientY);
      if (targetId !== null) {
        const current = edgeEndpointsRef.current.get(reconnect.id);
        const next: EdgeEndpoints = {
          from:
            reconnect.side === "source"
              ? targetId
              : (current?.from ?? targetId),
          to:
            reconnect.side === "target" ? targetId : (current?.to ?? targetId),
        };
        if (next.from === next.to) {
          return;
        }
        const base =
          edgesRef.current.find((edge) => edge.id === reconnect.id) ??
          addedEdgesRef.current.find((edge) => edge.id === reconnect.id);
        const before = current ?? base;
        setEdgeEndpoints((prev) => new Map(prev).set(reconnect.id, next));
        if (
          before !== undefined &&
          (next.from !== before.from || next.to !== before.to)
        ) {
          emitEditResult({
            moved: [],
            removed: [],
            addedEdges: [],
            rewired: [{ id: reconnect.id, from: next.from, to: next.to }],
          });
        }
      }
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
    },
    [emitEditResult],
  );

  /** 从占位端口圆点拖出，落在另一张卡片：新建一条示例连线（会话状态，不持久化）。 */
  const finishConnect = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const connect = connectRef.current;
      if (connect === null) return;
      connectRef.current = null;
      setConnectPreview(null);
      const targetId = cardIdAtPoint(event.clientX, event.clientY);
      if (targetId !== null && targetId !== connect.from) {
        const fromBox = boxesRef.current.get(connect.from);
        const toBox = boxesRef.current.get(targetId);
        const route =
          fromBox !== undefined &&
          toBox !== undefined &&
          Math.abs(toBox.y - fromBox.y) >= fromBox.h
            ? "vertical"
            : "horizontal";
        const id = `${connect.from}->${targetId}-${Math.round(performance.now())}`;
        setAddedEdges((prev) => [
          ...prev,
          { id, from: connect.from, to: targetId, route, phase: "idle" },
        ]);
      }
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
    },
    [],
  );

  const endPointer = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (
        reconnectRef.current !== null &&
        reconnectRef.current.pointerId === event.pointerId
      ) {
        finishReconnect(event);
        return;
      }

      if (
        connectRef.current !== null &&
        connectRef.current.pointerId === event.pointerId
      ) {
        finishConnect(event);
        return;
      }

      const pointer = pointersRef.current.get(event.pointerId);
      if (pointer === undefined) return;
      pointer.x = event.clientX;
      pointer.y = event.clientY;

      if (frameRef.current !== null) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }

      const pinch = pinchRef.current;
      pinchRef.current = null;
      if (pinch !== null && pointersRef.current.size === PINCH_POINTER_COUNT) {
        const points = Array.from(pointersRef.current.values());
        const center = gestureCenter(points[0], points[1]);
        const distance = gestureDistance(points[0], points[1]);
        commit(
          pinchTo(
            viewportRef.current,
            pinch.center,
            pinch.distance,
            center,
            distance,
          ),
        );
      } else {
        flushGesture();
      }

      const cardDrag = cardDragRef.current;
      if (cardDrag !== null && cardDrag.pointerId === event.pointerId) {
        cardDragRef.current = null;
        if (cardDrag.moved) {
          suppressClickUntilRef.current = performance.now() + CLICK_SUPPRESS_MS;
        } else {
          selectCard(cardDrag.id);
        }
      }

      pointersRef.current.delete(event.pointerId);
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }

      if (pointersRef.current.size === 0) {
        dragRef.current = null;
        rectRef.current = null;
        setPanning(false);
        return;
      }

      const [remainingId, remaining] = Array.from(
        pointersRef.current.entries(),
      )[0];
      if (remaining.onCard) {
        dragRef.current = null;
        setPanning(false);
        return;
      }
      beginPan(remainingId, remaining.x, remaining.y);
    },
    [
      beginPan,
      commit,
      finishConnect,
      finishReconnect,
      flushGesture,
      selectCard,
    ],
  );

  const handlePortDown = useCallback(
    (
      cardId: string,
      side: CanvasPortSide,
      event: React.PointerEvent<HTMLSpanElement>,
    ) => {
      if (event.pointerType === "mouse" && event.button !== 0) return;
      const box = boxesRef.current.get(cardId);
      if (box === undefined) return;
      event.preventDefault();
      event.stopPropagation();
      const start = anchorOf(box, side);
      readRect();
      connectRef.current = {
        pointerId: event.pointerId,
        from: cardId,
        side,
        start,
      };
      setConnectPreview({ start, point: start });
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [readRect],
  );

  const handleEdgeHandleDown = useCallback(
    (
      id: string,
      side: EdgeEndpointSide,
      event: React.PointerEvent<HTMLButtonElement>,
    ) => {
      event.preventDefault();
      event.stopPropagation();
      reconnectRef.current = { pointerId: event.pointerId, id, side };
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [],
  );

  const removeSelected = useCallback(() => {
    const id = selected;
    if (id === null) return;
    if (
      edges.some((edge) => edge.id === id) ||
      addedEdges.some((edge) => edge.id === id)
    ) {
      setRemovedEdges((prev) => new Set(prev).add(id));
      select(null);
      return;
    }
    setRemovedCards((prev) => new Set(prev).add(id));
    setRemovedEdges((prev) => {
      const next = new Set(prev);
      for (const edge of [...edges, ...addedEdges]) {
        if (edge.from === id || edge.to === id) next.add(edge.id);
      }
      return next;
    });
    select(null);
  }, [addedEdges, edges, select, selected]);

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape") {
        select(null);
        return;
      }
      if (event.key === "Delete" || event.key === "Backspace") {
        if (selected === null) return;
        event.preventDefault();
        removeSelected();
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
    [removeSelected, reset, select, selected, zoomBy],
  );

  const visibleCards = useMemo(() => {
    const all = [...cards, ...placedCards];
    return all
      .filter((card) => !removedCards.has(card.id))
      .map((card) => {
        const box = movedBoxes.get(card.id);
        return box === undefined ? card : { ...card, box };
      });
  }, [cards, movedBoxes, placedCards, removedCards]);

  const boxes = useMemo(() => boxesOf(visibleCards), [visibleCards]);
  boxesRef.current = boxes;

  const visibleEdges = useMemo(() => {
    const all = [...edges, ...addedEdges];
    return all
      .filter((edge) => !removedEdges.has(edge.id))
      .map((edge) => {
        const endpoints = edgeEndpoints.get(edge.id);
        return endpoints === undefined ? edge : { ...edge, ...endpoints };
      })
      .filter(
        (edge) =>
          boxes.has(edge.from) && boxes.has(edge.to) && edge.from !== edge.to,
      );
  }, [addedEdges, boxes, edgeEndpoints, edges, removedEdges]);

  const edgeNodes = useMemo(
    () =>
      visibleEdges.map((edge) => {
        const from = boxes.get(edge.from);
        const to = boxes.get(edge.to);
        if (from === undefined || to === undefined) return null;
        return (
          <CanvasEdge
            key={edge.id}
            edge={edge}
            from={from}
            to={to}
            gutter={gutter}
            selected={edge.id === selected}
            onSelect={select}
            onHandleDown={handleEdgeHandleDown}
          />
        );
      }),
    [boxes, gutter, handleEdgeHandleDown, select, selected, visibleEdges],
  );

  const cardNodes = useMemo(
    () =>
      visibleCards.map((card) => (
        <CanvasCard
          key={card.id}
          card={card}
          selected={card.id === selected}
          onSelect={selectCard}
          onPortDown={handlePortDown}
        />
      )),
    [handlePortDown, selectCard, selected, visibleCards],
  );

  const labelNodes = useMemo(
    () => labels.map((label) => <CanvasLabel key={label.id} label={label} />),
    [labels],
  );

  const previewNode = useMemo(() => {
    if (connectPreview === null) return null;
    const dx = connectPreview.point.x - connectPreview.start.x;
    const dy = connectPreview.point.y - connectPreview.start.y;
    const length = Math.hypot(dx, dy);
    const angle = (Math.atan2(dy, dx) * 180) / Math.PI;
    return (
      <div
        className={styles.connectPreview}
        style={{
          left: connectPreview.start.x,
          top: connectPreview.start.y - 1,
          width: length,
          transform: `rotate(${angle}deg)`,
        }}
      />
    );
  }, [connectPreview]);

  const dropNode = useMemo(() => {
    if (dropPoint === null) return null;
    return (
      <div
        className={styles.dropPreview}
        aria-hidden="true"
        style={{
          left: dropPoint.x,
          top: dropPoint.y,
          width: dropPreviewSize.w,
          height: dropPreviewSize.h,
        }}
      />
    );
  }, [dropPoint, dropPreviewSize.h, dropPreviewSize.w]);

  /** 放置栏回车：视口中心 → 世界坐标 → 交给调用方生成卡片。 */
  useEffect(() => {
    if (placeRequest === null) return;
    const rect = readRect();
    if (rect === null) return;
    const { x, y, scale } = viewportRef.current;
    const point = {
      x: (rect.width / 2 - x) / scale,
      y: (rect.height / 2 - y) / scale,
    };
    onDropCardRef.current?.(placeRequest.kind, point);
    // 只认令牌变化：回调与视口状态不进依赖，避免每次渲染都落卡。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [placeRequest?.token]);

  const handleDragOver = useCallback(
    (event: React.DragEvent<HTMLDivElement>) => {
      if (!event.dataTransfer.types.includes(CARD_KIND_MIME)) return;
      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
      const point = worldPointOf(event.clientX, event.clientY);
      if (point !== null) {
        setDropPoint({
          x: point.x - dropPreviewSize.w / 2,
          y: point.y - dropPreviewSize.h / 2,
        });
      }
    },
    [dropPreviewSize.h, dropPreviewSize.w, worldPointOf],
  );

  const handleDragLeave = useCallback(
    (event: React.DragEvent<HTMLDivElement>) => {
      const next = event.relatedTarget;
      if (next instanceof Node && event.currentTarget.contains(next)) return;
      setDropPoint(null);
    },
    [],
  );

  const handleDrop = useCallback(
    (event: React.DragEvent<HTMLDivElement>) => {
      const kind = event.dataTransfer.getData(CARD_KIND_MIME);
      if (kind === "") return;
      event.preventDefault();
      setDropPoint(null);
      const point = worldPointOf(event.clientX, event.clientY);
      if (point === null) return;
      onDropCardRef.current?.(kind, point);
    },
    [worldPointOf],
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
    <div
      className={styles.viewport}
      data-modality={inputModality}
      onPointerDownCapture={markPointerModality}
      onKeyDownCapture={markKeyboardModality}
    >
      <div
        ref={surfaceRef}
        className={styles.surface}
        role="group"
        aria-label={t("canvas.viewport")}
        tabIndex={0}
        data-panning={panning}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={endPointer}
        onPointerCancel={endPointer}
        onKeyDown={handleKeyDown}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        <div className={styles.grid} aria-hidden="true">
          <div className={styles.gridMinor} style={minorGridStyle} />
          <div className={styles.gridMajor} style={majorGridStyle} />
        </div>
        <div className={styles.world} style={worldStyle}>
          <div className={styles.layer}>{edgeNodes}</div>
          <div className={styles.layer}>{cardNodes}</div>
          <div className={styles.layerOverlays}>
            {labelNodes}
            {previewNode}
            {dropNode}
          </div>
        </div>
      </div>
      {header === null || header === undefined ? null : (
        <div className={styles.floatingHeader}>{header}</div>
      )}
      <div className={styles.floatingControls}>
        <span className={styles.zoomLevel} aria-label={t("canvas.zoom_level")}>
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
      <CanvasLegend
        groups={legendGroups}
        showFacilityGroups={showFacilityLegend}
      />
    </div>
  );
}
