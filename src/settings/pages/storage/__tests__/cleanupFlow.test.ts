import { afterEach, describe, expect, it, vi } from "vitest";
import {
  INITIAL_CLEANUP_STATE,
  canExecute,
  dialogBody,
  dialogOpen,
  dialogTitle,
  cleanupErrorView,
  cleanupReducer,
  cleanupResultNotice,
  type CleanupState,
} from "../cleanupFlow";
import { actionText, cleanupErrorCodeText } from "../lexicon";
import {
  makePlanEntry,
  makeReclaimEntryResult,
  makeReclaimResult,
} from "./fixtures";

afterEach(() => {
  vi.restoreAllMocks();
});

function request(key = "attachments"): CleanupState {
  return cleanupReducer(INITIAL_CLEANUP_STATE, {
    type: "request",
    key,
    action: "deleteOrphans",
  });
}

describe("清理状态机", () => {
  it("idle 发起清理进入 planning，并清空上一次残留", () => {
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
      cleanupReducer(planning, {
        type: "request",
        key: "logs",
        action: "sweepExpired",
      }),
    ).toBe(planning);

    const executing = cleanupReducer(planning, {
      type: "planned",
      entry: makePlanEntry({ confirmation: "none" }),
    });
    expect(executing.phase).toBe("executing");
    expect(
      cleanupReducer(executing, {
        type: "request",
        key: "logs",
        action: "sweepExpired",
      }),
    ).toBe(executing);
  });

  it("confirmation 为 none 直接执行，不弹确认框", () => {
    const state = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "none" }),
    });
    expect(state.phase).toBe("executing");
    expect(dialogOpen(state)).toBe(false);
    expect(canExecute(state)).toBe(false);
  });

  it("confirmation 需要确认时进入 confirming，普通档无需键入即可执行", () => {
    const state = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "confirm" }),
    });
    expect(state.phase).toBe("confirming");
    expect(dialogOpen(state)).toBe(true);
    expect(canExecute(state)).toBe(true);
  });

  it("强确认必须键入条目 key 才可执行", () => {
    const state = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "strongConfirm" }),
    });
    expect(canExecute(state)).toBe(false);

    const partial = cleanupReducer(state, { type: "typed", value: "attach" });
    expect(canExecute(partial)).toBe(false);

    const exact = cleanupReducer(partial, {
      type: "typed",
      value: "attachments",
    });
    expect(canExecute(exact)).toBe(true);
  });

  it("确认后由 execute 事件进入执行，弹窗保持打开显示进度", () => {
    const confirming = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "confirm" }),
    });
    const executing = cleanupReducer(confirming, { type: "execute" });
    expect(executing.phase).toBe("executing");
    expect(dialogOpen(executing)).toBe(true);
    expect(canExecute(executing)).toBe(false);
  });

  it("未满足强确认时 execute 事件被忽略", () => {
    const confirming = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "strongConfirm" }),
    });
    expect(cleanupReducer(confirming, { type: "execute" })).toBe(confirming);
  });

  it("idle 或 planning 阶段的 execute 事件被忽略", () => {
    expect(cleanupReducer(INITIAL_CLEANUP_STATE, { type: "execute" })).toBe(
      INITIAL_CLEANUP_STATE,
    );
    const planning = request();
    expect(cleanupReducer(planning, { type: "execute" })).toBe(planning);
  });

  it("planning 阶段收到 typed 被忽略", () => {
    const state = request();
    expect(cleanupReducer(state, { type: "typed", value: "x" })).toBe(state);
  });

  it("取消在确认阶段生效，在执行阶段被忽略", () => {
    const confirming = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "confirm" }),
    });
    expect(cleanupReducer(confirming, { type: "cancel" })).toEqual(
      INITIAL_CLEANUP_STATE,
    );

    const executing = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "none" }),
    });
    expect(cleanupReducer(executing, { type: "cancel" })).toBe(executing);
  });

  it("失败按 keepDialogOpen 决定回 idle 还是留在确认态", () => {
    const confirming = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "strongConfirm" }),
    });

    const closed = cleanupReducer(confirming, {
      type: "failed",
      keepDialogOpen: false,
    });
    expect(closed).toEqual(INITIAL_CLEANUP_STATE);

    const kept = cleanupReducer(
      cleanupReducer(confirming, { type: "typed", value: "attachments" }),
      { type: "failed", keepDialogOpen: true },
    );
    expect(kept.phase).toBe("confirming");
    expect(kept.plan).not.toBeNull();
    expect(kept.typedKey).toBe("");
  });

  it("成功后回到初始态", () => {
    const executing = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "none" }),
    });
    expect(cleanupReducer(executing, { type: "succeeded" })).toEqual(
      INITIAL_CLEANUP_STATE,
    );
  });

  it("idle 收到失败不产生状态变化，也不留下重试标记", () => {
    expect(
      cleanupReducer(INITIAL_CLEANUP_STATE, {
        type: "failed",
        keepDialogOpen: false,
      }),
    ).toBe(INITIAL_CLEANUP_STATE);
  });
});

describe("清理错误映射", () => {
  it("busy 映射到忙碌文案并关闭弹窗", () => {
    const view = cleanupErrorView("plan", "busy");
    expect(view.messageKey).toBe("settings.storage.cleanup.error.busy");
    expect(view.keepDialogOpen).toBe(false);
  });

  it("计划过期与计划丢失分别映射", () => {
    expect(cleanupErrorView("plan", "planExpired").messageKey).toBe(
      "settings.storage.cleanup.error.plan_expired",
    );
    expect(cleanupErrorView("execute", "planNotFound").messageKey).toBe(
      "settings.storage.cleanup.error.plan_not_found",
    );
  });

  it("执行阶段 confirmationRequired 保留弹窗并带原文", () => {
    const view = cleanupErrorView("execute", "confirmationRequired");
    expect(view.messageKey).toBe(
      "settings.storage.cleanup.error.confirmation_required",
    );
    expect(view.keepDialogOpen).toBe(true);
    expect(view.detail).toBe("confirmationRequired");
  });

  it("Error 对象取 message 作为错误码", () => {
    const view = cleanupErrorView("plan", new Error("planExpired"));
    expect(view.messageKey).toBe("settings.storage.cleanup.error.plan_expired");
    expect(view.detail).toBeNull();
  });

  it("未知错误按阶段回退到通用失败文案并带原文", () => {
    const plan = cleanupErrorView("plan", "boom");
    expect(plan.messageKey).toBe("settings.storage.cleanup.error.plan_failed");
    expect(plan.detail).toBe("boom");
    expect(plan.keepDialogOpen).toBe(false);

    const execute = cleanupErrorView("execute", { code: 500 });
    expect(execute.messageKey).toBe(
      "settings.storage.cleanup.error.execute_failed",
    );
    expect(execute.detail).toContain("500");
  });
});

describe("清理结果提示", () => {
  it("done 报成功并带数量与字节", () => {
    const notice = cleanupResultNotice(
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
    const notice = cleanupResultNotice(
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
    expect(notice.detail).toBe(cleanupErrorCodeText("permissionDenied"));
  });

  it("failed 报失败并保留错误码文案", () => {
    const notice = cleanupResultNotice(
      makeReclaimResult([
        makeReclaimEntryResult({
          status: "failed",
          failedCount: 2,
          errors: [{ code: "io", message: "disk error" }],
        }),
      ]),
    );
    expect(notice.kind).toBe("error");
    expect(notice.detail).toBe(cleanupErrorCodeText("io"));
  });

  it("skipped 报跳过而不是失败", () => {
    const notice = cleanupResultNotice(
      makeReclaimResult([
        makeReclaimEntryResult({ status: "skipped", skippedCount: 2 }),
      ]),
    );
    expect(notice.kind).toBe("info");
  });

  it("notEligible 或空结果按失败处理并告警", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(
      cleanupResultNotice(
        makeReclaimResult([makeReclaimEntryResult({ status: "notEligible" })]),
      ).kind,
    ).toBe("error");
    expect(cleanupResultNotice(makeReclaimResult([])).kind).toBe("error");
    expect(warn).toHaveBeenCalledTimes(2);
  });
});

describe("确认弹窗文案", () => {
  it("普通确认标题带条目名，强确认标题带动作", () => {
    const confirm = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "confirm" }),
    });
    const strong = cleanupReducer(request(), {
      type: "planned",
      entry: makePlanEntry({ confirmation: "strongConfirm" }),
    });

    expect(dialogTitle(confirm).length).toBeGreaterThan(0);
    expect(dialogTitle(strong)).not.toBe(dialogTitle(confirm));
    expect(dialogBody(strong)).toContain(actionText("deleteOrphans"));
    expect(dialogBody(strong)).toContain("attachments");
  });

  it("计划条数或字节不可得时显示占位符而不是 0", () => {
    const state = cleanupReducer(request(), {
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
    expect(dialogTitle(INITIAL_CLEANUP_STATE)).toBe("");
    expect(dialogBody(INITIAL_CLEANUP_STATE)).toBe("");
  });
});
