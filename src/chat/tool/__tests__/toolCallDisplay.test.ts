import { describe, it, expect } from "vitest";
import {
  formatToolDuration,
  parsePersistedOutput,
  resultStats,
  shouldCollapseResult,
  summarizeArguments,
} from "../toolCallDisplay";

describe("summarizeArguments", () => {
  it("command 数组按 argv 拼接", () => {
    expect(summarizeArguments({ command: ["ls", "-la", "/tmp"] })).toBe(
      "ls -la /tmp",
    );
  });

  it("无 command 时优先取 path 类字段", () => {
    expect(summarizeArguments({ path: "src/main.ts", depth: 2 })).toBe(
      "src/main.ts",
    );
  });

  it("没有可读标量时返回 null", () => {
    expect(summarizeArguments({ nested: { a: 1 } })).toBeNull();
    expect(summarizeArguments({})).toBeNull();
    expect(summarizeArguments(undefined)).toBeNull();
  });

  it("超长摘要截断并加省略号", () => {
    const long = "a".repeat(300);
    const summary = summarizeArguments({ command: long })!;
    expect(summary.endsWith("…")).toBe(true);
    expect(summary.length).toBeLessThanOrEqual(241);
  });
});

describe("parsePersistedOutput", () => {
  it("解析落盘引用与前缀预览", () => {
    const parsed = parsePersistedOutput(
      '<persisted-output path="C:/tmp/tool.txt">\nfirst line',
    );
    expect(parsed).toEqual({ path: "C:/tmp/tool.txt", preview: "first line" });
  });

  it("非落盘格式返回 null", () => {
    expect(parsePersistedOutput("plain text")).toBeNull();
    expect(parsePersistedOutput("")).toBeNull();
    expect(parsePersistedOutput(null)).toBeNull();
  });
});

describe("shouldCollapseResult", () => {
  it("行数或字节数超阈值时折叠", () => {
    expect(shouldCollapseResult("short")).toBe(false);
    const manyLines = Array.from({ length: 20 }, (_, i) => `line ${i}`).join(
      "\n",
    );
    expect(shouldCollapseResult(manyLines)).toBe(true);
    expect(shouldCollapseResult("x".repeat(1100))).toBe(true);
  });

  it("统计行数与字节数", () => {
    expect(resultStats("a\nb")).toEqual({ lines: 2, bytes: 3 });
    expect(resultStats("")).toEqual({ lines: 0, bytes: 0 });
  });
});

describe("formatToolDuration", () => {
  it("按量级切换单位", () => {
    expect(formatToolDuration(0, 250)).toBe("250ms");
    expect(formatToolDuration(0, 1500)).toBe("1.5s");
    expect(formatToolDuration(0, 65_000)).toBe("1m5s");
  });

  it("缺少任一端点返回 null", () => {
    expect(formatToolDuration(null, 100)).toBeNull();
    expect(formatToolDuration(100, null)).toBeNull();
  });
});
