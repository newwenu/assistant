import { describe, expect, it } from "vitest";
import type { ClockSnapshotDto } from "@/types/protocol.contract";
import { computeClockView, formatClockTime } from "../clockTime";

function snapshot(overrides: Partial<ClockSnapshotDto> = {}): ClockSnapshotDto {
  return {
    mode: "countdown",
    status: "idle",
    durationMs: 60_000,
    remainingMs: 60_000,
    elapsedMs: 0,
    startedAtMs: null,
    elapsedBeforeMs: 0,
    ...overrides,
  };
}

describe("formatClockTime", () => {
  it("倒计时按剩余秒向上取整，普通计时按已用秒向下取整", () => {
    expect(formatClockTime(59_500, "countdown")).toBe("01:00");
    expect(formatClockTime(59_500, "stopwatch")).toBe("00:59");
  });

  it("达到一小时使用 h:mm:ss", () => {
    expect(formatClockTime(3_600_000, "countdown")).toBe("1:00:00");
    expect(formatClockTime(3_600_000, "stopwatch")).toBe("1:00:00");
    expect(formatClockTime(3_599_500, "countdown")).toBe("1:00:00");
  });

  it("零值显示 00:00", () => {
    expect(formatClockTime(0, "countdown")).toBe("00:00");
    expect(formatClockTime(0, "stopwatch")).toBe("00:00");
  });
});

describe("computeClockView", () => {
  it("倒计时运行中按墙钟续算剩余与进度", () => {
    const view = computeClockView(
      snapshot({ status: "running", startedAtMs: 1_000 }),
      31_000,
    );
    expect(view.elapsedMs).toBe(30_000);
    expect(view.remainingMs).toBe(30_000);
    expect(view.progress).toBe(0.5);
  });

  it("倒计时暂停时剩余冻结", () => {
    const view = computeClockView(
      snapshot({ status: "paused", elapsedBeforeMs: 30_000 }),
      91_000,
    );
    expect(view.elapsedMs).toBe(30_000);
    expect(view.remainingMs).toBe(30_000);
    expect(view.progress).toBe(0.5);
  });

  it("倒计时完成时剩余为零", () => {
    const view = computeClockView(
      snapshot({ status: "finished", elapsedBeforeMs: 60_000 }),
      91_000,
    );
    expect(view.elapsedMs).toBe(60_000);
    expect(view.remainingMs).toBe(0);
    expect(view.progress).toBe(0);
  });

  it("普通计时运行中累加已用时间且没有剩余/进度", () => {
    const view = computeClockView(
      snapshot({
        mode: "stopwatch",
        status: "running",
        startedAtMs: 1_000,
        remainingMs: null,
      }),
      31_000,
    );
    expect(view.elapsedMs).toBe(30_000);
    expect(view.remainingMs).toBeNull();
    expect(view.progress).toBeNull();
  });

  it("普通计时超过倒计时时长后不被裁剪", () => {
    const view = computeClockView(
      snapshot({
        mode: "stopwatch",
        status: "paused",
        elapsedBeforeMs: 120_000,
        remainingMs: null,
      }),
      200_000,
    );
    expect(view.elapsedMs).toBe(120_000);
    expect(view.remainingMs).toBeNull();
  });
});
