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

  it("清理按钮：不可清理时禁用且带原因文案", () => {
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
      expect(view.cleanup.kind).toBe("button");
      if (view.cleanup.kind !== "button") throw new Error("unreachable");
      expect(view.cleanup.disabled).toBe(true);
      expect(view.cleanup.tooltip).toContain(reasonText(item.reason));
    }

    const notImplemented = entryRowView(
      makeEntry({
        reclaim: makeReclaim({
          action: "sweepExpired",
          eligibility: "notImplemented",
          reason: "noReclaimApi",
          note: "暂未提供清理入口",
        }),
      }),
    );
    expect(notImplemented.cleanup.kind).toBe("button");
    if (notImplemented.cleanup.kind !== "button")
      throw new Error("unreachable");
    expect(notImplemented.cleanup.disabled).toBe(true);
    expect(notImplemented.cleanup.tooltip).toContain("暂未提供清理入口");
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
    expect(view.cleanup.kind).toBe("text");
    if (view.cleanup.kind !== "text") throw new Error("unreachable");
    expect(view.cleanup.text.length).toBeGreaterThan(0);
  });

  it("action 为 none 时清理列没有入口", () => {
    const view = entryRowView(
      makeEntry({
        reclaim: makeReclaim({ action: "none", eligibility: "notEligible" }),
      }),
    );
    expect(view.cleanup.kind).toBe("none");
  });

  it("eligible 但 action 为 none 时降级为无入口并告警", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const view = entryRowView(
      makeEntry({
        reclaim: makeReclaim({ action: "none", eligibility: "eligible" }),
      }),
    );
    expect(view.cleanup.kind).toBe("none");
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
    expect(view.cleanup.kind).toBe("button");
    if (view.cleanup.kind !== "button") throw new Error("unreachable");
    expect(view.cleanup.disabled).toBe(false);
    expect(view.cleanup.strongConfirm).toBe(true);
  });

  it("可用性只在异常时出现角标，missing 与 available 不加角标", () => {
    expect(
      entryRowView(makeEntry({ availability: "available" })).availabilityBadge,
    ).toBeNull();
    expect(
      entryRowView(makeEntry({ availability: "missing" })).availabilityBadge,
    ).toBeNull();

    const locked = entryRowView(
      makeEntry({ availability: "locked", availabilityReason: "EACCES" }),
    );
    expect(locked.availabilityBadge?.tone).toBe("warning");
    expect(locked.availabilityBadge?.reason).toBe("EACCES");

    const unreadable = entryRowView(makeEntry({ availability: "unreadable" }));
    expect(unreadable.availabilityBadge?.tone).toBe("error");

    const notImplemented = entryRowView(
      makeEntry({ availability: "notImplemented" }),
    );
    expect(notImplemented.availabilityBadge?.tone).toBe("neutral");
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
  it("详情固定六项，首项为 key 原文", () => {
    const detail = entryDetailView(makeEntry({ key: "tool-results" }));
    expect(detail.items).toHaveLength(6);
    expect(detail.items[0].value).toBe("tool-results");
    expect(detail.items[0].mono).toBe(true);
  });

  it("时间与可清理为空时显示占位符", () => {
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

  it("failClosed 不参与详情与策略 chip", () => {
    const detail = entryDetailView(
      makeEntry({
        reclaim: makeReclaim({ policy: makePolicy({ failClosed: true }) }),
      }),
    );
    expect(detail.chips).toEqual([]);
    const texts = detail.items.map((item) => item.value);
    expect(texts.join("")).not.toContain("failClosed");
  });
});
