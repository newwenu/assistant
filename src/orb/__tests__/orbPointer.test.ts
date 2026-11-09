import { describe, expect, it } from "vitest";
import {
  advanceDrag,
  createDragState,
  createGestureState,
  rebaseDragState,
  reduceGesture,
  resolveContextMenu,
  type GestureEffect,
  type GestureEvent,
  type GestureState,
  type PointerKind,
} from "../pointerGesture";

interface DownOverrides {
  pointerId?: number;
  pointerType?: PointerKind;
  isPrimary?: boolean;
  button?: number;
  x?: number;
  y?: number;
  sx?: number;
  sy?: number;
}

function down(over: DownOverrides = {}): GestureEvent {
  return {
    kind: "down",
    pointerId: 1,
    pointerType: "mouse",
    isPrimary: true,
    button: 0,
    x: 100,
    y: 100,
    sx: 300,
    sy: 300,
    ...over,
  };
}

function move(over: DownOverrides = {}): GestureEvent {
  return {
    kind: "move",
    pointerId: 1,
    x: 100,
    y: 100,
    sx: 300,
    sy: 300,
    ...over,
  };
}

function up(pointerId = 1): GestureEvent {
  return { kind: "up", pointerId };
}

function cancel(pointerId = 1): GestureEvent {
  return { kind: "cancel", pointerId };
}

function lostcapture(pointerId = 1): GestureEvent {
  return { kind: "lostcapture", pointerId };
}

function longPressElapsed(now = 500): GestureEvent {
  return { kind: "longPressElapsed", now };
}

function run(
  state: GestureState,
  events: GestureEvent[],
): { state: GestureState; effects: GestureEffect[] } {
  const effects: GestureEffect[] = [];
  let current = state;
  for (const event of events) {
    const result = reduceGesture(current, event);
    current = result.state;
    effects.push(...result.effects);
  }
  return { state: current, effects };
}

function types(effects: GestureEffect[]): string[] {
  return effects.map((effect) => effect.type);
}

describe("手势状态机", () => {
  it("U1 鼠标按下 → 移动 7px → 抬起：产生 tap，无拖拽", () => {
    const { state, effects } = run(createGestureState(), [
      down({ pointerType: "mouse" }),
      move({ x: 107, y: 100, sx: 307, sy: 300 }),
      up(),
    ]);

    expect(types(effects)).not.toContain("dragStart");
    expect(types(effects)).toContain("tap");
    expect(state.phase).toBe("idle");
  });

  it("U2 触摸按下 → 移动 10px → 抬起：无拖拽（触摸阈值 12），仍算 tap", () => {
    const { state, effects } = run(createGestureState(), [
      down({ pointerType: "touch" }),
      move({ x: 110, y: 100, sx: 310, sy: 300 }),
      up(),
    ]);

    expect(types(effects)).not.toContain("dragStart");
    expect(types(effects)).toContain("tap");
    expect(state.phase).toBe("idle");
  });

  it("U3 触摸按下 → 移动 13px：进入 dragging，长按计时被取消", () => {
    const { state, effects } = run(createGestureState(), [
      down({ pointerType: "touch" }),
      move({ x: 113, y: 100, sx: 313, sy: 300 }),
    ]);

    expect(state.phase).toBe("dragging");
    expect(types(effects)).toContain("dragStart");
    expect(types(effects)).toContain("longPressDisarm");
  });

  it("U4 触摸按下 → 移动 9px → 静置 500ms：长按被取消，不弹菜单", () => {
    const { state, effects } = run(createGestureState(), [
      down({ pointerType: "touch" }),
      move({ x: 109, y: 100, sx: 309, sy: 300 }),
      longPressElapsed(),
    ]);

    expect(types(effects)).toContain("longPressDisarm");
    expect(types(effects)).not.toContain("menu");
    expect(state.phase).toBe("pressed");
  });

  it("U5 触摸长按 → 抬起：产出 menu 并记录 menuShownAt，不产出 tap", () => {
    const { state, effects } = run(createGestureState(), [
      down({ pointerType: "touch" }),
      longPressElapsed(500),
      up(),
    ]);

    const menu = effects.find((effect) => effect.type === "menu");
    expect(menu).toBeDefined();
    expect(menu && "x" in menu ? menu.x : null).toBe(100);
    expect(state.menuShownAt).toBe(500);
    expect(types(effects)).not.toContain("tap");
    expect(state.phase).toBe("idle");
  });

  it("U6 长按后 700ms 内来 contextmenu：不弹第二次；超过 700ms 才弹", () => {
    const { state } = run(createGestureState(), [
      down({ pointerType: "touch" }),
      longPressElapsed(500),
      up(),
    ]);

    expect(
      resolveContextMenu(state, { pointerType: "touch", now: 600 }).show,
    ).toBe(false);
    expect(
      resolveContextMenu(state, { pointerType: "touch", now: 1200 }).show,
    ).toBe(true);
    expect(
      resolveContextMenu(state, { pointerType: "mouse", now: 600 }).show,
    ).toBe(true);
  });

  it("U7 dragging 中 cancel：回到 idle，产出 finishDrag 且标记 interrupted", () => {
    const { state, effects } = run(createGestureState(), [
      down({ pointerType: "touch" }),
      move({ x: 113, y: 100, sx: 313, sy: 300 }),
      cancel(),
    ]);

    const finish = effects.filter((effect) => effect.type === "finishDrag");
    expect(finish).toHaveLength(1);
    expect(
      finish[0] && "interrupted" in finish[0] ? finish[0].interrupted : false,
    ).toBe(true);
    expect(state.phase).toBe("idle");
  });

  it("U10 非 isPrimary / button !== 0 的 down：状态不变", () => {
    const primary = run(createGestureState(), [down({ isPrimary: false })]);
    const rightButton = run(createGestureState(), [down({ button: 2 })]);

    expect(primary.state.phase).toBe("idle");
    expect(types(primary.effects)).toEqual([]);
    expect(rightButton.state.phase).toBe("idle");
    expect(types(rightButton.effects)).toEqual([]);
  });

  it("U11 dragging 中收到新的合法 down：先产出 finishDrag，再进入 pressed", () => {
    const { state, effects } = run(createGestureState(), [
      down({ pointerType: "touch" }),
      move({ x: 113, y: 100, sx: 313, sy: 300 }),
      down({ pointerId: 2, pointerType: "touch" }),
    ]);

    const finishIndex = types(effects).indexOf("finishDrag");
    const captureIndex = types(effects).lastIndexOf("capture");
    expect(finishIndex).toBeGreaterThanOrEqual(0);
    expect(captureIndex).toBeGreaterThan(finishIndex);
    expect(state.phase).toBe("pressed");
    expect(state.pointerId).toBe(2);
  });

  it("U12 pointerId 不一致的 move / up / cancel / lostcapture：全部丢弃", () => {
    const base = run(createGestureState(), [
      down({ pointerType: "touch" }),
      move({ x: 113, y: 100, sx: 313, sy: 300 }),
    ]);

    const mismatched = run(base.state, [
      move({ pointerId: 9, x: 200, y: 200, sx: 400, sy: 400 }),
      up(9),
      cancel(9),
      lostcapture(9),
    ]);

    expect(mismatched.state.phase).toBe("dragging");
    expect(types(mismatched.effects)).toEqual([]);
  });

  it("U13 pressed 中 cancel / lostcapture：回到 idle，不产出 tap，长按计时被取消", () => {
    const viaCancel = run(createGestureState(), [
      down({ pointerType: "touch" }),
      cancel(),
    ]);
    const viaLost = run(createGestureState(), [
      down({ pointerType: "touch" }),
      lostcapture(),
    ]);

    for (const result of [viaCancel, viaLost]) {
      expect(result.state.phase).toBe("idle");
      expect(types(result.effects)).not.toContain("tap");
      expect(types(result.effects)).toContain("longPressDisarm");
    }
  });

  it("U17 up 先到 → 随后 lostcapture 只收尾一次；lostcapture 先到则放弃最终提交", () => {
    const upFirst = run(createGestureState(), [
      down({ pointerType: "touch" }),
      move({ x: 113, y: 100, sx: 313, sy: 300 }),
      up(),
      lostcapture(),
    ]);
    const lostFirst = run(createGestureState(), [
      down({ pointerType: "touch" }),
      move({ x: 113, y: 100, sx: 313, sy: 300 }),
      lostcapture(),
      up(),
    ]);

    expect(
      upFirst.effects.filter((effect) => effect.type === "finishDrag"),
    ).toHaveLength(1);
    expect(
      lostFirst.effects.filter((effect) => effect.type === "finishDrag"),
    ).toHaveLength(1);

    const upFinish = upFirst.effects.find(
      (effect) => effect.type === "finishDrag",
    );
    const lostFinish = lostFirst.effects.find(
      (effect) => effect.type === "finishDrag",
    );
    expect(
      upFinish && "interrupted" in upFinish ? upFinish.interrupted : null,
    ).toBe(false);
    expect(
      lostFinish && "interrupted" in lostFinish ? lostFinish.interrupted : null,
    ).toBe(true);
    expect(upFirst.state.phase).toBe("idle");
    expect(lostFirst.state.phase).toBe("idle");
  });
});

describe("拖拽定位", () => {
  it("U8 定点性：窗口已跟随的输入，目标恒等于窗口位置，无提交", () => {
    const start = { x: 300, y: 300 };
    const windowPos = { x: 100, y: 100 };
    let drag = createDragState(start, windowPos, 1);

    for (let i = 0; i < 10; i += 1) {
      const pointer = { sx: start.x + i, sy: start.y + i, dpr: 1 };
      const followed = {
        x: pointer.sx - drag.grabX,
        y: pointer.sy - drag.grabY,
      };
      drag = { ...drag, submittedX: followed.x, submittedY: followed.y };

      const step = advanceDrag(drag, pointer);
      expect(step.kind).toBe("hold");
      if (step.kind === "hold") {
        expect(step.target).toEqual({ x: windowPos.x + i, y: windowPos.y + i });
      }
      drag = step.drag;
    }
  });

  it("U9 落后补齐：落后 δ 时一步补齐，后续步无位移", () => {
    const drag = createDragState({ x: 300, y: 300 }, { x: 100, y: 100 }, 1);

    const jump = advanceDrag(drag, { sx: 350, sy: 300, dpr: 1 });
    expect(jump.kind).toBe("commit");
    if (jump.kind === "commit") {
      expect(jump.physical).toEqual({ x: 150, y: 100 });
    }

    const settled = advanceDrag(jump.drag, { sx: 350, sy: 300, dpr: 1 });
    expect(settled.kind).toBe("hold");
  });

  it("U14 dragging 中 dpr 变化：返回 rebase，G 按 S(t) − W_new 重算", () => {
    const drag = createDragState({ x: 300, y: 300 }, { x: 100, y: 100 }, 1);
    const pointer = { sx: 380, sy: 340, dpr: 2 };

    const step = advanceDrag(drag, pointer);
    expect(step.kind).toBe("rebase");

    const windowNew = { x: 90, y: 70 };
    const rebased = rebaseDragState(pointer, windowNew);
    expect(rebased.grabX).toBe(380 - 90);
    expect(rebased.grabY).toBe(340 - 70);
    expect(rebased.dpr).toBe(2);

    const after = advanceDrag(rebased, pointer);
    expect(after.kind).toBe("hold");
    if (after.kind === "hold") {
      expect(after.target).toEqual(windowNew);
    }
  });

  it("U15 触摸刚好越过阈值：首步目标 = 按下时窗口位置（无跳变）", () => {
    const { effects } = run(createGestureState(), [
      down({ pointerType: "touch" }),
      move({ x: 112, y: 100, sx: 312, sy: 300 }),
    ]);
    const dragStart = effects.find((effect) => effect.type === "dragStart");
    expect(dragStart).toBeDefined();

    const sx = dragStart && "sx" in dragStart ? dragStart.sx : 0;
    const sy = dragStart && "sy" in dragStart ? dragStart.sy : 0;
    const windowPos = { x: 100, y: 100 };
    const drag = createDragState({ x: sx, y: sy }, windowPos, 1);

    const first = advanceDrag(drag, { sx, sy, dpr: 1 });
    expect(first.kind).toBe("hold");
    if (first.kind === "hold") {
      expect(first.target).toEqual(windowPos);
    }
  });

  it("U16 滞后容错：在飞提交未生效时目标仍等于 S − G，不随迭代发散", () => {
    const grabX = 200;
    const grabY = 200;
    let drag = createDragState({ x: 300, y: 300 }, { x: 100, y: 100 }, 1);

    for (let i = 1; i <= 20; i += 1) {
      const pointer = { sx: 300 + i * 5, sy: 300 + i * 5, dpr: 1 };
      const step = advanceDrag(drag, pointer);
      if (step.kind === "hold") continue;
      expect(step.kind).toBe("commit");
      if (step.kind === "commit") {
        expect(step.target).toEqual({
          x: pointer.sx - grabX,
          y: pointer.sy - grabY,
        });
        expect(step.target.x - (pointer.sx - grabX)).toBe(0);
      }
      drag = step.drag;
    }
  });
});
