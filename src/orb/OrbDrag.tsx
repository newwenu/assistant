import {
  useEffect,
  useRef,
  useState,
  type MouseEventHandler,
  type PointerEventHandler,
} from "react";
import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { orbService } from "../services/orbService";
import { createLogger } from "@/services/logger";
import { LONG_PRESS_DURATION_MS, ORB_SNAP_DELAY_MS } from "../config/constants";
import { useOrbStore } from "../stores/orbStore";
import { getConfValue } from "@/settings/core/useConfGroup";
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

function pointerScreen(e: { clientX: number; clientY: number }): GesturePoint {
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
  onPointerDown: PointerEventHandler<HTMLElement>;
  onPointerMove: PointerEventHandler<HTMLElement>;
  onPointerUp: PointerEventHandler<HTMLElement>;
  onPointerCancel: PointerEventHandler<HTMLElement>;
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
  const expandRef = useRef(expand);
  expandRef.current = expand;

  const [phase, setPhase] = useState<GesturePhase>("idle");
  const [activePointerType, setActivePointerType] =
    useState<PointerKind | null>(null);

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
      void orbService
        .snapOrbToEdge()
        .catch((e) => log.warn("snap orb to edge failed", e));
    }, ORB_SNAP_DELAY_MS);
  }

  function showMenu(x: number, y: number): void {
    void orbService
      .showOrbContextMenu(x, y)
      .catch((e) => log.warn("show orb context menu failed", e));
  }

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

  const onPointerDown: PointerEventHandler<HTMLElement> = (e) => {
    if (e.button !== 0) return;
    const screen = pointerScreen(e);
    dispatch(
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
  };

  const onPointerMove: PointerEventHandler<HTMLElement> = (e) => {
    const screen = pointerScreen(e);
    dispatch({
      kind: "move",
      pointerId: e.pointerId,
      x: e.clientX,
      y: e.clientY,
      sx: screen.x,
      sy: screen.y,
    });
  };

  const onPointerUp: PointerEventHandler<HTMLElement> = (e) => {
    dispatch({ kind: "up", pointerId: e.pointerId }, e.currentTarget);
  };

  const onPointerCancel: PointerEventHandler<HTMLElement> = (e) => {
    dispatch({ kind: "cancel", pointerId: e.pointerId }, e.currentTarget);
  };

  const onLostPointerCapture: PointerEventHandler<HTMLElement> = (e) => {
    dispatch({ kind: "lostcapture", pointerId: e.pointerId }, e.currentTarget);
  };

  const onContextMenu: MouseEventHandler<HTMLElement> = (e) => {
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
  };

  return {
    activePointerType,
    isPressed: phase === "pressed" && activePointerType !== "mouse",
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
    onLostPointerCapture,
    onContextMenu,
  };
}
