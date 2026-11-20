import { afterEach, describe, expect, it, vi } from "vitest";
import {
  INITIAL_RECLAIM_STATE,
  canExecute,
  dialogBody,
  dialogOpen,
  dialogTitle,
  reclaimErrorView,
  reclaimReducer,
  reclaimResultNotice,
  type ReclaimState,
} from "../reclaimFlow";
import { actionText, reclaimErrorCodeText } from "../lexicon";
import {
  makePlanEntry,
  makeReclaimEntryResult,
  makeReclaimResult,
} from "./fixtures";

afterEach(() => {
  vi.restoreAllMocks();
});

function request(key = "attachments"): ReclaimState {
  return reclaimReducer(INITIAL_RECLAIM_STATE, {
    type: "request",
    key,
    action: "deleteOrphans",
  });
}

describe("回收状态机", () => {
  it("idle 发起回收进入 planning，并清空上一次残留", () => {
    const state = request();
    expect(state.phase).toBe("planning");
    expect(state.key).toBe("attachments");
    expect(state.action).toBe("deleteOrphans");
    expect(state.plan).toBeNull();
    expect(state.typedKey).toBe("");
  });

  it("非 idle 时重复发起被忽略，执行中不可重入", () => {
    const planning = request();
    expect(
      reclaimReducer(planning, {
        type: "request",
        key: "logs",
        action: "sweepExpired",
      }),
    ).toBe(planning);

    const executing = reclaimReducer(planning, {
      type: "planned",
      entry: makePlanEntry({ confirmation: "none" }),
    });
    expect(executing.phase).toBe("executing");
    expect(
      reclaimReducer(executing, {
        type: "request",
        key: "logs",
        action: "sweepExpired",
      }),
    ).toBe(executing);
  });

  it("confirmation 为 none 直接执行，不弹确认框", () => {
    const state = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "none" }),
    });
    expect(state.phase).toBe("executing");
    expect(dialogOpen(state)).toBe(false);
    expect(canExecute(state)).toBe(false);
  });

  it("confirmation 需要确认时进入 confirming，普通档无需键入即可执行", () => {
    const state = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "confirm" }),
    });
    expect(state.phase).toBe("confirming");
    expect(dialogOpen(state)).toBe(true);
    expect(canExecute(state)).toBe(true);
  });

  it("强确认必须键入条目 key 才可执行", () => {
    const state = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "strongConfirm" }),
    });
    expect(canExecute(state)).toBe(false);

    const partial = reclaimReducer(state, { type: "typed", value: "attach" });
    expect(canExecute(partial)).toBe(false);

    const exact = reclaimReducer(partial, {
      type: "typed",
      value: "attachments",
    });
    expect(canExecute(exact)).toBe(true);
  });

  it("确认后由 execute 事件进入执行，弹窗保持打开显示进度", () => {
    const confirming = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "confirm" }),
    });
    const executing = reclaimReducer(confirming, { type: "execute" });
    expect(executing.phase).toBe("executing");
    expect(dialogOpen(executing)).toBe(true);
    expect(canExecute(executing)).toBe(false);
  });

  it("未满足强确认时 execute 事件被忽略", () => {
    const confirming = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "strongConfirm" }),
    });
    expect(reclaimReducer(confirming, { type: "execute" })).toBe(confirming);
  });

  it("idle 或 planning 阶段的 execute 事件被忽略", () => {
    expect(reclaimReducer(INITIAL_RECLAIM_STATE, { type: "execute" })).toBe(
      INITIAL_RECLAIM_STATE,
    );
    const planning = request();
    expect(reclaimReducer(planning, { type: "execute" })).toBe(planning);
  });

  it("planning 阶段收到 typed 被忽略", () => {
    const state = request();
    expect(reclaimReducer(state, { type: "typed", value: "x" })).toBe(state);
  });

  it("取消在确认阶段生效，在执行阶段被忽略", () => {
    const confirming = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "confirm" }),
    });
    expect(reclaimReducer(confirming, { type: "cancel" })).toEqual(
      INITIAL_RECLAIM_STATE,
    );

    const executing = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "none" }),
    });
    expect(reclaimReducer(executing, { type: "cancel" })).toBe(executing);
  });

  it("失败按 keepDialogOpen 决定回 idle 还是留在确认态", () => {
    const confirming = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "strongConfirm" }),
    });

    const closed = reclaimReducer(confirming, {
      type: "failed",
      keepDialogOpen: false,
    });
    expect(closed).toEqual(INITIAL_RECLAIM_STATE);

    const kept = reclaimReducer(
      reclaimReducer(confirming, { type: "typed", value: "attachments" }),
      { type: "failed", keepDialogOpen: true },
    );
    expect(kept.phase).toBe("confirming");
    expect(kept.plan).not.toBeNull();
    expect(kept.typedKey).toBe("");
  });

  it("成功后回到初始态", () => {
    const executing = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "none" }),
    });
    expect(reclaimReducer(executing, { type: "succeeded" })).toEqual(
      INITIAL_RECLAIM_STATE,
    );
  });

  it("idle 收到失败不产生状态变化，也不留下重试标记", () => {
    expect(
      reclaimReducer(INITIAL_RECLAIM_STATE, {
        type: "failed",
        keepDialogOpen: false,
      }),
    ).toBe(INITIAL_RECLAIM_STATE);
  });
});

describe("回收错误映射", () => {
  it("busy 映射到忙碌文案并关闭弹窗", () => {
    const view = reclaimErrorView("plan", "busy");
    expect(view.messageKey).toBe("settings.storage.reclaim.error.busy");
    expect(view.keepDialogOpen).toBe(false);
  });

  it("计划过期与计划丢失分别映射", () => {
    expect(reclaimErrorView("plan", "planExpired").messageKey).toBe(
      "settings.storage.reclaim.error.plan_expired",
    );
    expect(reclaimErrorView("execute", "planNotFound").messageKey).toBe(
      "settings.storage.reclaim.error.plan_not_found",
    );
  });

  it("执行阶段 confirmationRequired 保留弹窗并带原文", () => {
    const view = reclaimErrorView("execute", "confirmationRequired");
    expect(view.messageKey).toBe(
      "settings.storage.reclaim.error.confirmation_required",
    );
    expect(view.keepDialogOpen).toBe(true);
    expect(view.detail).toBe("confirmationRequired");
  });

  it("Error 对象取 message 作为错误码", () => {
    const view = reclaimErrorView("plan", new Error("planExpired"));
    expect(view.messageKey).toBe("settings.storage.reclaim.error.plan_expired");
    expect(view.detail).toBeNull();
  });

  it("未知错误按阶段回退到通用失败文案并带原文", () => {
    const plan = reclaimErrorView("plan", "boom");
    expect(plan.messageKey).toBe("settings.storage.reclaim.error.plan_failed");
    expect(plan.detail).toBe("boom");
    expect(plan.keepDialogOpen).toBe(false);

    const execute = reclaimErrorView("execute", { code: 500 });
    expect(execute.messageKey).toBe(
      "settings.storage.reclaim.error.execute_failed",
    );
    expect(execute.detail).toContain("500");
  });
});

describe("回收结果提示", () => {
  it("done 报成功并带数量与字节", () => {
    const notice = reclaimResultNotice(
      makeReclaimResult([
        makeReclaimEntryResult({
          status: "done",
          removedCount: 3,
          removedBytes: 4096,
        }),
      ]),
    );
    expect(notice.kind).toBe("success");
    expect(notice.text).toContain("3");
    expect(notice.text).toContain("4.0 KB");
    expect(notice.detail).toBeNull();
  });

  it("partial 带失败计数与首个错误码文案", () => {
    const notice = reclaimResultNotice(
      makeReclaimResult([
        makeReclaimEntryResult({
          status: "partial",
          removedCount: 2,
          removedBytes: 2048,
          failedCount: 1,
          errors: [{ code: "permissionDenied", message: "EACCES" }],
        }),
      ]),
    );
    expect(notice.kind).toBe("warning");
    expect(notice.text).toContain("1");
    expect(notice.detail).toBe(reclaimErrorCodeText("permissionDenied"));
  });

  it("failed 报失败并保留错误码文案", () => {
    const notice = reclaimResultNotice(
      makeReclaimResult([
        makeReclaimEntryResult({
          status: "failed",
          failedCount: 2,
          errors: [{ code: "io", message: "disk error" }],
        }),
      ]),
    );
    expect(notice.kind).toBe("error");
    expect(notice.detail).toBe(reclaimErrorCodeText("io"));
  });

  it("skipped 报跳过而不是失败", () => {
    const notice = reclaimResultNotice(
      makeReclaimResult([
        makeReclaimEntryResult({ status: "skipped", skippedCount: 2 }),
      ]),
    );
    expect(notice.kind).toBe("info");
  });

  it("notEligible 或空结果按失败处理并告警", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(
      reclaimResultNotice(
        makeReclaimResult([makeReclaimEntryResult({ status: "notEligible" })]),
      ).kind,
    ).toBe("error");
    expect(reclaimResultNotice(makeReclaimResult([])).kind).toBe("error");
    expect(warn).toHaveBeenCalledTimes(2);
  });
});

describe("确认弹窗文案", () => {
  it("普通确认标题带条目名，强确认标题带动作", () => {
    const confirm = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "confirm" }),
    });
    const strong = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "strongConfirm" }),
    });

    expect(dialogTitle(confirm).length).toBeGreaterThan(0);
    expect(dialogTitle(strong)).not.toBe(dialogTitle(confirm));
    expect(dialogBody(strong)).toContain(actionText("deleteOrphans"));
    expect(dialogBody(strong)).toContain("attachments");
  });

  it("计划条数或字节不可得时显示占位符而不是 0", () => {
    const state = reclaimReducer(request(), {
      type: "planned",
      entry: makePlanEntry({
        confirmation: "confirm",
        expectedCount: null,
        expectedBytes: null,
      }),
    });
    expect(dialogBody(state)).toContain("—");
    expect(dialogBody(state)).not.toContain("0 B");
  });

  it("无计划时标题与正文为空", () => {
    expect(dialogTitle(INITIAL_RECLAIM_STATE)).toBe("");
    expect(dialogBody(INITIAL_RECLAIM_STATE)).toBe("");
  });
});
