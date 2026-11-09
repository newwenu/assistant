import {
  CONTEXT_MENU_DEDUPE_MS,
  LONG_PRESS_MOVE_TOLERANCE_PX,
  PET_DRAG_THRESHOLD_PX,
  TOUCH_DRAG_THRESHOLD_PX,
} from "../config/constants";

export type PointerKind = "mouse" | "touch" | "pen";

export type GesturePhase = "idle" | "pressed" | "dragging" | "longPressed";

export interface GesturePoint {
  x: number;
  y: number;
}

export type GestureEvent =
  | {
      kind: "down";
      pointerId: number;
      pointerType: PointerKind;
      isPrimary: boolean;
      button: number;
      x: number;
      y: number;
      sx: number;
      sy: number;
    }
  | {
      kind: "move";
      pointerId: number;
      x: number;
      y: number;
      sx: number;
      sy: number;
    }
  | { kind: "up"; pointerId: number }
  | { kind: "cancel"; pointerId: number }
  | { kind: "lostcapture"; pointerId: number }
  | { kind: "longPressElapsed"; now: number };

export interface GestureState {
  phase: GesturePhase;
  pointerId: number | null;
  pointerType: PointerKind | null;
  downClientX: number;
  downClientY: number;
  downScreenX: number;
  downScreenY: number;
  longPressEligible: boolean;
  menuShownAt: number | null;
}

export type GestureEffect =
  | { type: "capture"; pointerId: number }
  | { type: "releaseCapture"; pointerId: number }
  | { type: "longPressArm" }
  | { type: "longPressDisarm" }
  | { type: "tap" }
  | { type: "menu"; x: number; y: number }
  | {
      type: "dragStart";
      pointerId: number;
      pointerType: PointerKind;
      sx: number;
      sy: number;
    }
  | { type: "dragMove"; sx: number; sy: number }
  | { type: "finishDrag"; interrupted: boolean };

export interface GestureResult {
  state: GestureState;
  effects: GestureEffect[];
}

export function createGestureState(
  menuShownAt: number | null = null,
): GestureState {
  return {
    phase: "idle",
    pointerId: null,
    pointerType: null,
    downClientX: 0,
    downClientY: 0,
    downScreenX: 0,
    downScreenY: 0,
    longPressEligible: false,
    menuShownAt,
  };
}

function idle(state: GestureState): GestureState {
  return { ...createGestureState(state.menuShownAt) };
}

function unchanged(state: GestureState): GestureResult {
  return { state, effects: [] };
}

function releaseEffects(state: GestureState): GestureEffect[] {
  const effects: GestureEffect[] = [];
  if (state.longPressEligible) effects.push({ type: "longPressDisarm" });
  if (state.pointerId !== null) {
    effects.push({ type: "releaseCapture", pointerId: state.pointerId });
  }
  return effects;
}

function onDown(
  state: GestureState,
  event: Extract<GestureEvent, { kind: "down" }>,
): GestureResult {
  if (!event.isPrimary || event.button !== 0) return unchanged(state);

  const effects: GestureEffect[] = [];
  if (state.phase !== "idle") {
    if (state.phase === "dragging") {
      effects.push({ type: "finishDrag", interrupted: true });
    }
    effects.push(...releaseEffects(state));
  }

  const longPressEligible = event.pointerType !== "mouse";
  effects.push({ type: "capture", pointerId: event.pointerId });
  if (longPressEligible) effects.push({ type: "longPressArm" });

  return {
    state: {
      phase: "pressed",
      pointerId: event.pointerId,
      pointerType: event.pointerType,
      downClientX: event.x,
      downClientY: event.y,
      downScreenX: event.sx,
      downScreenY: event.sy,
      longPressEligible,
      menuShownAt: state.menuShownAt,
    },
    effects,
  };
}

function onMove(
  state: GestureState,
  event: Extract<GestureEvent, { kind: "move" }>,
): GestureResult {
  if (state.phase === "idle" || event.pointerId !== state.pointerId) {
    return unchanged(state);
  }

  if (state.phase === "dragging") {
    return {
      state,
      effects: [{ type: "dragMove", sx: event.sx, sy: event.sy }],
    };
  }

  if (state.phase === "longPressed") return unchanged(state);

  const dx = event.x - state.downClientX;
  const dy = event.y - state.downClientY;
  const distance = Math.sqrt(dx * dx + dy * dy);
  const threshold =
    state.pointerType === "mouse"
      ? PET_DRAG_THRESHOLD_PX
      : TOUCH_DRAG_THRESHOLD_PX;

  if (distance >= threshold) {
    const effects: GestureEffect[] = [];
    if (state.longPressEligible) effects.push({ type: "longPressDisarm" });
    effects.push({
      type: "dragStart",
      pointerId: event.pointerId,
      pointerType: state.pointerType ?? "mouse",
      sx: event.sx,
      sy: event.sy,
    });
    return {
      state: { ...state, phase: "dragging", longPressEligible: false },
      effects,
    };
  }

  if (state.longPressEligible && distance >= LONG_PRESS_MOVE_TOLERANCE_PX) {
    return {
      state: { ...state, longPressEligible: false },
      effects: [{ type: "longPressDisarm" }],
    };
  }

  return unchanged(state);
}

function onUp(
  state: GestureState,
  event: Extract<GestureEvent, { kind: "up" }>,
): GestureResult {
  if (state.phase === "idle" || event.pointerId !== state.pointerId) {
    return unchanged(state);
  }

  const effects = releaseEffects(state);
  if (state.phase === "pressed") effects.push({ type: "tap" });
  if (state.phase === "dragging") {
    effects.push({ type: "finishDrag", interrupted: false });
  }

  return { state: idle(state), effects };
}

function onAbort(
  state: GestureState,
  event: Extract<GestureEvent, { kind: "cancel" | "lostcapture" }>,
): GestureResult {
  if (state.phase === "idle" || event.pointerId !== state.pointerId) {
    return unchanged(state);
  }

  const effects = releaseEffects(state);
  if (state.phase === "dragging") {
    effects.push({ type: "finishDrag", interrupted: true });
  }

  return { state: idle(state), effects };
}

function onLongPressElapsed(
  state: GestureState,
  event: Extract<GestureEvent, { kind: "longPressElapsed" }>,
): GestureResult {
  if (state.phase !== "pressed" || !state.longPressEligible) {
    return unchanged(state);
  }
  if (state.pointerType === "mouse") return unchanged(state);

  return {
    state: {
      ...state,
      phase: "longPressed",
      longPressEligible: false,
      menuShownAt: event.now,
    },
    effects: [{ type: "menu", x: state.downClientX, y: state.downClientY }],
  };
}

export function reduceGesture(
  state: GestureState,
  event: GestureEvent,
): GestureResult {
  switch (event.kind) {
    case "down":
      return onDown(state, event);
    case "move":
      return onMove(state, event);
    case "up":
      return onUp(state, event);
    case "cancel":
    case "lostcapture":
      return onAbort(state, event);
    case "longPressElapsed":
      return onLongPressElapsed(state, event);
    default:
      return unchanged(state);
  }
}

export function resolveContextMenu(
  state: GestureState,
  event: { pointerType: PointerKind; now: number },
): { show: boolean } {
  if (event.pointerType === "mouse") return { show: true };
  if (state.menuShownAt === null) return { show: true };
  return { show: event.now - state.menuShownAt >= CONTEXT_MENU_DEDUPE_MS };
}

export interface DragState {
  grabX: number;
  grabY: number;
  dpr: number;
  submittedX: number | null;
  submittedY: number | null;
}

export type DragStep =
  | { kind: "hold"; drag: DragState; target: GesturePoint }
  | {
      kind: "commit";
      drag: DragState;
      target: GesturePoint;
      physical: GesturePoint;
    }
  | { kind: "rebase"; drag: DragState };

export interface DragPointer {
  sx: number;
  sy: number;
  dpr: number;
}

export function createDragState(
  start: GesturePoint,
  windowLogical: GesturePoint,
  dpr: number,
): DragState {
  return {
    grabX: start.x - windowLogical.x,
    grabY: start.y - windowLogical.y,
    dpr,
    submittedX: Math.round(windowLogical.x * dpr),
    submittedY: Math.round(windowLogical.y * dpr),
  };
}

export function rebaseDragState(
  pointer: DragPointer,
  windowLogical: GesturePoint,
): DragState {
  return createDragState(
    { x: pointer.sx, y: pointer.sy },
    windowLogical,
    pointer.dpr,
  );
}

export function advanceDrag(drag: DragState, pointer: DragPointer): DragStep {
  if (pointer.dpr !== drag.dpr) return { kind: "rebase", drag };

  const target: GesturePoint = {
    x: pointer.sx - drag.grabX,
    y: pointer.sy - drag.grabY,
  };
  const physical: GesturePoint = {
    x: Math.round(target.x * pointer.dpr),
    y: Math.round(target.y * pointer.dpr),
  };

  if (
    drag.submittedX !== null &&
    drag.submittedY !== null &&
    Math.abs(physical.x - drag.submittedX) < 1 &&
    Math.abs(physical.y - drag.submittedY) < 1
  ) {
    return { kind: "hold", drag, target };
  }

  return {
    kind: "commit",
    drag: { ...drag, submittedX: physical.x, submittedY: physical.y },
    target,
    physical,
  };
}
