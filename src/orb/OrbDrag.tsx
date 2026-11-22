import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type MouseEventHandler,
  type PointerEventHandler,
} from "react";
import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { orbService } from "../services/desktop/orbService";
import { createLogger } from "@/services/system/logger";
import { LONG_PRESS_DURATION_MS, ORB_SNAP_DELAY_MS } from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { getConfValue } from "@/settings/core/runtime/useConfGroup";
import { useOrbExpand } from "./OrbExpand";
import {
  advanceDrag,
  createDragState,
  createGestureState,
  rebaseDragState,
  reduceGesture,
  resolveContextMenu,
  type DragPointer,
  type DragState,
  type DragStep,
  type GestureEffect,
  type GestureEvent,
  type GesturePhase,
  type GesturePoint,
  type GestureState,
  type PointerKind,
} from "./pointerGesture";

const IS_WINDOWS = /\bWindows\b/i.test(navigator.userAgent);
const log = createLogger("OrbDrag");

const currentDpr = (): number => window.devicePixelRatio || 1;
const isValidScale = (scale: number): boolean =>
  Number.isFinite(scale) && scale > 0;

function pointerScreen(e: {
  clientX: number;
  clientY: number;
  screenX?: number;
  screenY?: number;
}): GesturePoint {
  // 优先用事件自带的绝对屏幕坐标：`window.screenX + clientX` 里的 screenX 是处理器
  // 执行时刻的窗口位置，比事件生成时刻晚一帧；拖拽中窗口正被 pump 以 60fps 移动，
  // 快速拖动时这一帧的滞后可达数十像素，会让窗口落点系统性偏移、把光标甩出命中区。
  if (typeof e.screenX === "number" && typeof e.screenY === "number") {
    return { x: e.screenX, y: e.screenY };
  }
  return { x: window.screenX + e.clientX, y: window.screenY + e.clientY };
}

function toPointerKind(pointerType: string): PointerKind {
  return pointerType === "touch" || pointerType === "pen"
    ? pointerType
    : "mouse";
}

function isPointerKind(value: string): value is PointerKind {
  return value === "mouse" || value === "touch" || value === "pen";
}

interface DragSession {
  pointerType: PointerKind;
  drag: DragState | null;
  pending: GesturePoint | null;
  pumping: boolean;
  accepting: boolean;
  rebase: boolean;
  lastPointer: DragPointer | null;
  finished: boolean;
  cancelled: boolean;
}

export interface OrbDragApi {
  activePointerType: PointerKind | null;
  isPressed: boolean;
  isMenuOpen: boolean;
  onPointerDown: PointerEventHandler<HTMLElement>;
  onPointerMove: PointerEventHandler<HTMLElement>;
  onPointerUp: PointerEventHandler<HTMLElement>;
  onPointerCancel: PointerEventHandler<HTMLElement>;
  onPointerLeave: PointerEventHandler<HTMLElement>;
  onLostPointerCapture: PointerEventHandler<HTMLElement>;
  onContextMenu: MouseEventHandler<HTMLElement>;
}

export function useOrbDrag(): OrbDragApi {
  const setDragging = useOrbStore((s) => s.setDragging);
  const { expand } = useOrbExpand();

  const gestureRef = useRef<GestureState>(createGestureState());
  const sessionRef = useRef<DragSession | null>(null);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const snapTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastPointerTypeRef = useRef<PointerKind>("mouse");
  /** 左键是否仍按住：拖拽被中断（cancel / 丢失捕获）时用它决定能不能立刻贴边。 */
  const pointerHeldRef = useRef(false);
  const expandRef = useRef(expand);
  expandRef.current = expand;

  const [phase, setPhase] = useState<GesturePhase>("idle");
  const [activePointerType, setActivePointerType] =
    useState<PointerKind | null>(null);
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const menuOpenRef = useRef(false);

  const setMenuOpen = useCallback((open: boolean): void => {
    if (menuOpenRef.current === open) return;
    menuOpenRef.current = open;
    setIsMenuOpen(open);
  }, []);

  function clearLongPress(): void {
    if (longPressTimerRef.current !== null) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  }

  function clearSnap(): void {
    if (snapTimerRef.current !== null) {
      clearTimeout(snapTimerRef.current);
      snapTimerRef.current = null;
    }
  }

  function scheduleSnap(): void {
    clearSnap();
    snapTimerRef.current = setTimeout(() => {
      snapTimerRef.current = null;
      // 到点再校验一次：松手后 1s 内用户可能已经重新按住小球（此时 phase 为 pressed
      // 或正在拖拽，还没跨过 8px 阈值），贴边会把球从用户手底下拽走。
      if (useOrbStore.getState().isDragging) return;
      if (gestureRef.current.phase !== "idle") return;
      // 指针还按着：多半是拖拽被系统收回捕获（`lostcapture`）后立即排队的贴边，
      // 此刻用户并没有松手，贴边会把球从手底下拽走。
      if (pointerHeldRef.current) return;
      void orbService
        .snapOrbToEdge()
        .catch((e) => log.warn("snap orb to edge failed", e));
    }, ORB_SNAP_DELAY_MS);
  }

  const clearSnapRef = useRef(clearSnap);
  clearSnapRef.current = clearSnap;

  const showMenu = useCallback(
    (x: number, y: number): void => {
      setMenuOpen(true);
      void orbService
        .showOrbContextMenu(x, y)
        .catch((e) => log.warn("show orb context menu failed", e))
        .finally(() => setMenuOpen(false));
    },
    [setMenuOpen],
  );

  function applyStep(session: DragSession, step: DragStep): void {
    if (step.kind === "hold") {
      session.drag = step.drag;
      return;
    }
    if (step.kind === "rebase") {
      session.rebase = true;
      return;
    }
    session.drag = step.drag;
    session.pending = step.physical;
  }

  async function rebase(
    session: DragSession,
    pointer: DragPointer,
  ): Promise<boolean> {
    if (!session.drag) return false;
    try {
      const position = await getCurrentWindow().outerPosition();
      const dpr = pointer.dpr;
      if (!isValidScale(dpr)) return false;
      const logical = { x: position.x / dpr, y: position.y / dpr };
      session.drag = rebaseDragState(pointer, logical);
      applyStep(session, advanceDrag(session.drag, pointer));
      return true;
    } catch (e) {
      log.warn("drag rebase failed", e);
      return false;
    }
  }

  async function pump(session: DragSession): Promise<void> {
    if (session.pumping) return;
    session.pumping = true;
    const appWindow = getCurrentWindow();
    try {
      while (!session.cancelled) {
        if (session.rebase) {
          session.rebase = false;
          const pointer = session.lastPointer;
          if (pointer && session.drag && pointer.dpr !== session.drag.dpr) {
            const ok = await rebase(session, pointer);
            if (!ok) {
              endDrag(true);
              break;
            }
          }
          continue;
        }
        if (session.pending === null) break;
        const target = session.pending;
        session.pending = null;
        try {
          await appWindow.setPosition(new PhysicalPosition(target.x, target.y));
        } catch (e) {
          log.warn("set orb position failed", e);
          endDrag(true);
          break;
        }
      }
    } finally {
      session.pumping = false;
    }
  }

  function endDrag(interrupted: boolean): void {
    const session = sessionRef.current;
    if (!session || session.finished) return;
    session.finished = true;
    session.accepting = false;
    if (interrupted) {
      session.pending = null;
      session.cancelled = true;
    }
    setDragging(false);
    if (!useOrbStore.getState().isExpanded) {
      const orbConf = getConfValue("orb");
      if (orbConf?.clickThrough) {
        void orbService
          .setOrbClickThrough(true)
          .catch((e) => log.warn("restore click through failed", e));
      }
    }
    scheduleSnap();
  }

  function startDrag(
    effect: Extract<GestureEffect, { type: "dragStart" }>,
  ): void {
    const previous = sessionRef.current;
    if (previous) previous.cancelled = true;

    const session: DragSession = {
      pointerType: effect.pointerType,
      drag: null,
      pending: null,
      pumping: false,
      accepting: true,
      rebase: false,
      lastPointer: { sx: effect.sx, sy: effect.sy, dpr: currentDpr() },
      finished: false,
      cancelled: false,
    };
    sessionRef.current = session;

    setDragging(true);
    clearSnap();
    void orbService
      .cancelOrbSnap()
      .catch((e) => log.warn("cancel orb snap failed", e));

    const orbConf = getConfValue("orb");
    if (orbConf?.clickThrough) {
      void orbService
        .setOrbClickThrough(false)
        .catch((e) => log.warn("disable click through failed", e));
    }

    if (effect.pointerType === "mouse" && !IS_WINDOWS) {
      void getCurrentWindow()
        .startDragging()
        .catch((e) => log.warn("startDragging failed", e))
        .finally(() => endDrag(false));
      return;
    }

    const dpr = currentDpr();
    if (!isValidScale(dpr)) {
      endDrag(true);
      return;
    }

    void getCurrentWindow()
      .outerPosition()
      .then((position) => {
        if (session.cancelled || session.finished) return;
        const logical = { x: position.x / dpr, y: position.y / dpr };
        session.drag = createDragState(
          { x: effect.sx, y: effect.sy },
          logical,
          dpr,
        );
        if (session.lastPointer) {
          applyStep(session, advanceDrag(session.drag, session.lastPointer));
        }
        void pump(session);
      })
      .catch((e) => {
        log.warn("drag baseline failed", e);
        endDrag(true);
      });
  }

  function moveDrag(sx: number, sy: number): void {
    const session = sessionRef.current;
    if (!session || !session.accepting) return;
    const pointer: DragPointer = { sx, sy, dpr: currentDpr() };
    session.lastPointer = pointer;
    if (!session.drag) return;
    applyStep(session, advanceDrag(session.drag, pointer));
    void pump(session);
  }

  function applyEffects(
    effects: GestureEffect[],
    target: HTMLElement | null,
  ): void {
    for (const effect of effects) {
      switch (effect.type) {
        case "capture":
          if (target) {
            try {
              target.setPointerCapture(effect.pointerId);
            } catch {
              log.debug("setPointerCapture failed");
            }
          }
          break;
        case "releaseCapture":
          if (target) {
            try {
              target.releasePointerCapture(effect.pointerId);
            } catch {
              log.debug("releasePointerCapture failed");
            }
          }
          break;
        case "longPressArm":
          clearLongPress();
          longPressTimerRef.current = setTimeout(() => {
            longPressTimerRef.current = null;
            dispatchRef.current({ kind: "longPressElapsed", now: Date.now() });
          }, LONG_PRESS_DURATION_MS);
          break;
        case "longPressDisarm":
          clearLongPress();
          break;
        case "tap":
          if (!useOrbStore.getState().isExpanded) {
            void expandRef.current();
          }
          break;
        case "menu":
          showMenu(effect.x, effect.y);
          break;
        case "dragStart":
          startDrag(effect);
          break;
        case "dragMove":
          moveDrag(effect.sx, effect.sy);
          break;
        case "finishDrag":
          endDrag(effect.interrupted);
          break;
        default:
          break;
      }
    }
  }

  function dispatch(
    event: GestureEvent,
    target: HTMLElement | null = null,
  ): void {
    const result = reduceGesture(gestureRef.current, event);
    gestureRef.current = result.state;
    setPhase(result.state.phase);
    const nextType =
      result.state.pointerId === null ? null : result.state.pointerType;
    setActivePointerType(nextType);
    if (nextType) lastPointerTypeRef.current = nextType;
    applyEffects(result.effects, target);
  }

  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  useEffect(() => {
    return () => {
      clearLongPress();
      clearSnap();
      const session = sessionRef.current;
      if (session) session.cancelled = true;
    };
  }, []);

  const onPointerDown = useCallback<PointerEventHandler<HTMLElement>>(
    (e) => {
      // 按下即撤销上一次松手排队的贴边：从「按下」到「位移跨过 8px 进入 dragging」之间
      // 可能停顿很久（尤其贴边抓取时），不撤销会让 1s 定时器在拖拽途中准点触发。
      clearSnapRef.current();
      // 右键按下先切到箭头光标：原生菜单弹起后会阻塞渲染线程，此刻再改 CSS 已来不及。
      if (e.button === 2) {
        setMenuOpen(true);
        return;
      }
      setMenuOpen(false);
      if (e.button !== 0) return;
      pointerHeldRef.current = true;
      const screen = pointerScreen(e);
      dispatchRef.current(
        {
          kind: "down",
          pointerId: e.pointerId,
          pointerType: toPointerKind(e.pointerType),
          isPrimary: e.isPrimary,
          button: e.button,
          x: e.clientX,
          y: e.clientY,
          sx: screen.x,
          sy: screen.y,
        },
        e.currentTarget,
      );
    },
    [setMenuOpen],
  );

  const onPointerMove = useCallback<PointerEventHandler<HTMLElement>>((e) => {
    const screen = pointerScreen(e);
    dispatchRef.current({
      kind: "move",
      pointerId: e.pointerId,
      x: e.clientX,
      y: e.clientY,
      sx: screen.x,
      sy: screen.y,
    });
  }, []);

  const onPointerUp = useCallback<PointerEventHandler<HTMLElement>>(
    (e) => {
      // 先清「按住」标记再派发：finishDrag 会在派发内同步 scheduleSnap，
      // 定时器到点时要能看到「已经松手」。
      if (e.button !== 2) pointerHeldRef.current = false;
      // 右键抬起后 contextmenu 会紧接着补一次 showMenu（Windows 顺序是 up → contextmenu），
      // 这里先复位，避免没弹菜单的右键把箭头光标留在小球上。
      if (e.button === 2) setMenuOpen(false);
      dispatchRef.current(
        { kind: "up", pointerId: e.pointerId },
        e.currentTarget,
      );
    },
    [setMenuOpen],
  );

  const onPointerCancel = useCallback<PointerEventHandler<HTMLElement>>(
    (e) => {
      pointerHeldRef.current = false;
      setMenuOpen(false);
      dispatchRef.current(
        { kind: "cancel", pointerId: e.pointerId },
        e.currentTarget,
      );
    },
    [setMenuOpen],
  );

  // 指针离开命中区就复位菜单光标：右键按下后移到窗口外松开时既不会收到 pointerup，
  // 也不会在命中区触发 contextmenu，否则箭头会一直留在小球上。
  const onPointerLeave = useCallback<PointerEventHandler<HTMLElement>>(() => {
    setMenuOpen(false);
  }, [setMenuOpen]);

  const onLostPointerCapture = useCallback<PointerEventHandler<HTMLElement>>(
    (e) => {
      dispatchRef.current(
        { kind: "lostcapture", pointerId: e.pointerId },
        e.currentTarget,
      );
    },
    [],
  );

  const onContextMenu = useCallback<MouseEventHandler<HTMLElement>>(
    (e) => {
      e.preventDefault();
      const nativeType = (e.nativeEvent as PointerEvent).pointerType;
      const pointerType = isPointerKind(nativeType)
        ? nativeType
        : lastPointerTypeRef.current;
      const now = Date.now();
      const { show } = resolveContextMenu(gestureRef.current, {
        pointerType,
        now,
      });
      if (!show) return;
      gestureRef.current = { ...gestureRef.current, menuShownAt: now };
      showMenu(e.clientX, e.clientY);
    },
    [showMenu],
  );

  return {
    activePointerType,
    isPressed: phase === "pressed" && activePointerType !== "mouse",
    isMenuOpen,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    onPointerLeave,
    onLostPointerCapture,
    onContextMenu,
  };
}
