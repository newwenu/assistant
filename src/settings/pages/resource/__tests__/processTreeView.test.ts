import { afterEach, describe, expect, it, vi } from "vitest";
import { translateKey } from "@/i18n";
import type { ProcessSnapshotDto } from "@/types/protocol.contract";
import { formatBytes } from "@/utils/formatBytes";
import {
  cpuText,
  entryRowView,
  memoryColumnText,
  summaryView,
} from "../processTreeView";

vi.mock("@/utils/formatBytes", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/utils/formatBytes")>();
  return { formatBytes: vi.fn(actual.formatBytes) };
});

afterEach(() => {
  vi.mocked(formatBytes).mockClear();
});

function makeSnapshot(
  overrides: Partial<ProcessSnapshotDto> = {},
): ProcessSnapshotDto {
  return {
    availability: "available",
    reason: null,
    rootPid: 4242,
    hostThreadCount: 12,
    memoryBasis: "privateWorkingSet",
    memoryAdditive: true,
    memoryEqualsFootprint: false,
    webviewDetached: false,
    truncated: false,
    sampledAt: 1_700_000_000_000,
    cpuBasis: "singleCorePercent",
    coverage: "subtreeOnly",
    entries: [
      {
        pid: 4242,
        parentPid: 0,
        startTime: 100,
        exeName: "assistant.exe",
        role: "host",
        cpuPercent: 12.5,
        memoryBytes: 2048,
      },
      {
        pid: 4243,
        parentPid: 4242,
        startTime: 101,
        exeName: "msedgewebview2.exe",
        role: "webviewRenderer",
        cpuPercent: null,
        memoryBytes: 1024,
      },
    ],
    selfGroup: {
      pidCount: 1,
      cpuPercent: 12.5,
      memoryBytes: 2048,
      memoryAvailability: "available",
      memoryReason: null,
    },
    subtreeGroup: {
      pidCount: 2,
      cpuPercent: 12.5,
      memoryBytes: 3072,
      memoryAvailability: "available",
      memoryReason: null,
    },
    ...overrides,
  };
}

describe("进程树视图模型", () => {
  it("内存列名由 memoryBasis 驱动", () => {
    expect(memoryColumnText("privateWorkingSet")).toBe(
      translateKey("settings.resource.caliber.winPrivate"),
    );
    expect(memoryColumnText("workingSet")).toBe(
      translateKey("settings.resource.caliber.linuxWorkingSet"),
    );
    expect(memoryColumnText("physFootprint")).toBe(
      translateKey("settings.resource.caliber.macFootprint"),
    );
  });

  it("CPU 无基准时显示占位文案，有值时保留一位小数", () => {
    expect(cpuText(null)).toBe(
      translateKey("settings.resource.status.cpuFirstSample"),
    );
    expect(cpuText(12.5)).toBe("12.5%");
    expect(cpuText(0)).toBe("0.0%");
  });

  it("内存为 null 显示占位符且不调用 formatBytes", () => {
    const view = entryRowView({
      pid: 4242,
      parentPid: 0,
      startTime: 100,
      exeName: "assistant.exe",
      role: "host",
      cpuPercent: null,
      memoryBytes: null,
    });
    expect(view.memoryText).toBe("—");
    expect(formatBytes).not.toHaveBeenCalled();
  });

  it("行 key 由 pid 与启动时间组成，可区分 PID 复用", () => {
    const base = {
      parentPid: 4242,
      exeName: "child.exe",
      role: "otherChild" as const,
      cpuPercent: 1,
      memoryBytes: 1024,
    };
    const first = entryRowView({ ...base, pid: 4242, startTime: 100 });
    const reused = entryRowView({ ...base, pid: 4242, startTime: 200 });
    expect(first.key).not.toBe(reused.key);
    expect(first.pidText).toBe(reused.pidText);
  });

  it("两个合计同时给出且不相加", () => {
    const summary = summaryView(makeSnapshot());
    expect(summary.self.memoryText).toBe(formatBytes(2048));
    expect(summary.subtree.memoryText).toBe(formatBytes(3072));
    expect(summary.relationText).toBe(
      translateKey("settings.resource.summary.relation"),
    );
    expect(summary.cpuNote).toBe(
      translateKey("settings.resource.summary.cpuNote"),
    );
  });

  it("宿主线程数为 null 时显示占位符而非 0", () => {
    const summary = summaryView(makeSnapshot({ hostThreadCount: null }));
    expect(summary.hostText).toContain("—");
    expect(summary.hostText).not.toContain("0");
  });

  it("采样时刻按 epoch 毫秒换算，不按秒", () => {
    const sampledAt = 1_700_000_000_123;
    const summary = summaryView(makeSnapshot({ sampledAt }));
    const expected = new Date(sampledAt).toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
    expect(summary.sampledAtText).toBe(
      translateKey("settings.resource.summary.sampledAt", { time: expected }),
    );

    const wrongScale = new Date(sampledAt * 1000).toLocaleTimeString(
      undefined,
      { hour: "2-digit", minute: "2-digit", second: "2-digit" },
    );
    expect(summary.sampledAtText).not.toBe(
      translateKey("settings.resource.summary.sampledAt", {
        time: wrongScale,
      }),
    );
  });

  it("采样时刻为 null 时不产生该行", () => {
    const summary = summaryView(makeSnapshot({ sampledAt: null }));
    expect(summary.sampledAtText).toBeNull();
  });

  it("口径分支由 memoryAdditive 驱动", () => {
    const additive = summaryView(
      makeSnapshot({ memoryAdditive: true, memoryEqualsFootprint: false }),
    );
    expect(additive.memoryCaliberNote).toBe(
      translateKey("settings.resource.summary.memoryNotDoubleCounted"),
    );

    const footprint = summaryView(
      makeSnapshot({ memoryAdditive: true, memoryEqualsFootprint: true }),
    );
    expect(footprint.memoryCaliberNote).toBe(
      translateKey("settings.resource.summary.memoryApprox"),
    );

    const nonAdditive = summaryView(makeSnapshot({ memoryAdditive: false }));
    expect(nonAdditive.memoryCaliberNote).toBeNull();
  });

  it("合计内存不可用时带原因，可用时无附注", () => {
    const unavailable = summaryView(
      makeSnapshot({
        memoryAdditive: false,
        selfGroup: {
          pidCount: 1,
          cpuPercent: 12.5,
          memoryBytes: null,
          memoryAvailability: "unavailable",
          memoryReason: "workingSetNotAdditive",
        },
      }),
    );
    expect(unavailable.self.memoryNote).toBe("workingSetNotAdditive");
    expect(unavailable.self.memoryText).toBe("—");

    const fallback = summaryView(
      makeSnapshot({
        selfGroup: {
          pidCount: 1,
          cpuPercent: 12.5,
          memoryBytes: null,
          memoryAvailability: "unavailable",
          memoryReason: null,
        },
      }),
    );
    expect(fallback.self.memoryNote).toBe(
      translateKey("settings.resource.status.memoryUnavailable"),
    );

    const available = summaryView(makeSnapshot());
    expect(available.self.memoryNote).toBeNull();
  });
});
