import { afterEach, describe, expect, it, vi } from "vitest";
import { formatBytes } from "@/utils/formatBytes";
import { entryDetailView, entryRowView } from "../entryView";
import { reasonText } from "../lexicon";
import { makeEntry, makePolicy, makeReclaim } from "./fixtures";

vi.mock("@/utils/formatBytes", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/utils/formatBytes")>();
  return { formatBytes: vi.fn(actual.formatBytes) };
});

afterEach(() => {
  vi.mocked(formatBytes).mockClear();
});

describe("条目行视图模型", () => {
  it("bytes 为 null 显示占位符且不调用 formatBytes", () => {
    const view = entryRowView(
      makeEntry({ bytes: null, count: null, availability: "unreadable" }),
    );
    expect(view.sizeText).toBe("—");
    expect(view.countText).toBeNull();
    expect(formatBytes).not.toHaveBeenCalled();
  });

  it("目录不存在且计 0 时显示 0 B，不显示未知", () => {
    const view = entryRowView(
      makeEntry({ bytes: 0, count: 0, availability: "missing" }),
    );
    expect(view.sizeText).toBe("0 B");
    expect(view.countText).toBeNull();
  });

  it("条数为 null 时不产生次行", () => {
    const view = entryRowView(makeEntry({ count: null }));
    expect(view.sizeText).toBe("1.0 KB");
    expect(view.countText).toBeNull();
  });

  it("条数非 null 时次行为「N 项」", () => {
    const view = entryRowView(makeEntry({ count: 3 }));
    expect(view.countText).not.toBeNull();
    expect(view.countText ?? "").toContain("3");
  });

  it("不可回收四态：按钮禁用且带原因文案", () => {
    const cases = [
      { eligibility: "notEligible", reason: "orphan" },
      { eligibility: "locked", reason: "locked" },
    ] as const;

    for (const item of cases) {
      const view = entryRowView(
        makeEntry({
          reclaim: makeReclaim({
            action: "deleteOrphans",
            eligibility: item.eligibility,
            reason: item.reason,
          }),
        }),
      );
      expect(view.action.kind).toBe("button");
      if (view.action.kind !== "button") throw new Error("unreachable");
      expect(view.action.disabled).toBe(true);
      expect(view.action.tooltip).toContain(reasonText(item.reason));
      expect(view.eligibility.secondLine).toBe(reasonText(item.reason));
    }

    const notImplemented = entryRowView(
      makeEntry({
        reclaim: makeReclaim({
          action: "sweepExpired",
          eligibility: "notImplemented",
          reason: "noReclaimApi",
          note: "清理入口在下一版提供",
        }),
      }),
    );
    expect(notImplemented.action.kind).toBe("button");
    if (notImplemented.action.kind !== "button") throw new Error("unreachable");
    expect(notImplemented.action.disabled).toBe(true);
    expect(notImplemented.eligibility.secondLine).toBe("清理入口在下一版提供");
    expect(notImplemented.action.tooltip).toContain("清理入口在下一版提供");
  });

  it("ownerManaged 显示纯文本，不出现按钮", () => {
    const view = entryRowView(
      makeEntry({
        reclaim: makeReclaim({
          action: "rotate",
          eligibility: "ownerManaged",
          reason: "externalOwner",
        }),
      }),
    );
    expect(view.action.kind).toBe("text");
    if (view.action.kind !== "text") throw new Error("unreachable");
    expect(view.action.text.length).toBeGreaterThan(0);
  });

  it("action 为 none 时动作列没有入口", () => {
    const view = entryRowView(
      makeEntry({
        reclaim: makeReclaim({ action: "none", eligibility: "notEligible" }),
      }),
    );
    expect(view.action.kind).toBe("none");
  });

  it("eligible 但 action 为 none 时降级为不可点并告警", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const view = entryRowView(
      makeEntry({
        reclaim: makeReclaim({ action: "none", eligibility: "eligible" }),
      }),
    );
    expect(view.action.kind).toBe("none");
    expect(warn).toHaveBeenCalledTimes(1);
    warn.mockRestore();
  });

  it("strongConfirm 只标记强确认，不影响可点性", () => {
    const view = entryRowView(
      makeEntry({
        reclaim: makeReclaim({
          action: "deleteOrphans",
          eligibility: "eligible",
          confirmation: "strongConfirm",
        }),
      }),
    );
    expect(view.action.kind).toBe("button");
    if (view.action.kind !== "button") throw new Error("unreachable");
    expect(view.action.disabled).toBe(false);
    expect(view.action.strongConfirm).toBe(true);
  });

  it("可用性徽标按枚举着色", () => {
    expect(
      entryRowView(makeEntry({ availability: "available" })).availability.tone,
    ).toBe("neutral");
    expect(
      entryRowView(makeEntry({ availability: "locked" })).availability.tone,
    ).toBe("warning");
    expect(
      entryRowView(makeEntry({ availability: "unreadable" })).availability.tone,
    ).toBe("error");
    expect(
      entryRowView(makeEntry({ availability: "notImplemented" })).availability
        .tone,
    ).toBe("info");
  });

  it("策略 chip 不因保留策略禁用而误显示天数", () => {
    const view = entryRowView(
      makeEntry({
        reclaim: makeReclaim({
          policy: makePolicy({ source: "config", retentionDays: 0 }),
        }),
      }),
    );
    expect(view.policyChips.length).toBe(2);
    expect(view.policyChips[1]).not.toContain("0");
  });
});

describe("条目详情视图模型", () => {
  it("详情固定六项，id 与 key 同列", () => {
    const detail = entryDetailView(makeEntry({ key: "tool-results" }));
    expect(detail.items).toHaveLength(6);
    expect(detail.items[0].value).toBe("E3 / tool-results");
    expect(detail.items[0].mono).toBe(true);
  });

  it("时间与可回收为空时显示占位符", () => {
    const detail = entryDetailView(
      makeEntry({ oldestAt: null, newestAt: null }),
    );
    expect(detail.items[3].value).toBe("—");
    expect(detail.items[4].value).toBe("—");
  });

  it("口径标记只在真值时出现", () => {
    expect(entryDetailView(makeEntry()).chips).toEqual([]);
    expect(
      entryDetailView(
        makeEntry({ includesDerived: true, countsSelfLogs: true }),
      ).chips,
    ).toHaveLength(2);
  });

  it("诊断原文原样透传，不做翻译", () => {
    const detail = entryDetailView(
      makeEntry({
        availabilityReason: "EACCES",
        reclaim: makeReclaim({ note: "owner offline" }),
      }),
    );
    expect(detail.availabilityReason).toBe("EACCES");
    expect(detail.note).toBe("owner offline");
  });
});
