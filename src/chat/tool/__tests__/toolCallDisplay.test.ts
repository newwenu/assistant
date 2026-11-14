import { describe, it, expect } from "vitest";
import type { ToolResultCompactDto } from "../../../types/protocol.contract";
import {
  formatToolDuration,
  parsePersistedOutput,
  resolveResultDisplay,
  resultStats,
  shouldCollapseResult,
  shouldRenderDocStub,
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

describe("resolveResultDisplay", () => {
  const compact = (
    patch: Partial<ToolResultCompactDto> = {},
  ): ToolResultCompactDto => ({
    head: "ok read a.txt · 3/3 lines",
    truncated: null,
    persisted: null,
    errorKind: null,
    anchor: null,
    ...patch,
  });

  it("有 compact 时 head 作标题且默认收起", () => {
    const display = resolveResultDisplay("file body", compact());
    expect(display.source).toBe("compact");
    expect(display.head).toBe("ok read a.txt · 3/3 lines");
    expect(display.body).toBe("file body");
    expect(display.defaultCollapsed).toBe(true);
  });

  it("有 compact 时透传 truncated 与 errorKind", () => {
    const display = resolveResultDisplay(
      "boom",
      compact({
        head: "err denied shell",
        truncated: { unit: "lines", returned: 20, total: 900 },
        errorKind: "denied",
      }),
    );
    expect(display.truncated).toEqual({
      unit: "lines",
      returned: 20,
      total: 900,
    });
    expect(display.errorKind).toBe("denied");
  });

  it("persisted 取自信封并按已知前缀裁剪标签", () => {
    const text = '<persisted-output path="store/c1.txt">\npreview';
    const display = resolveResultDisplay(
      text,
      compact({ persisted: { path: "store/c1.txt", bytes: 4096 } }),
    );
    expect(display.persisted).toEqual({ path: "store/c1.txt", bytes: 4096 });
    expect(display.body).toBe("preview");
  });

  it("信封无 persisted 时不解析文本标签（门禁 7）", () => {
    const text = '<persisted-output path="store/c1.txt">\npreview';
    const display = resolveResultDisplay(text, compact());
    expect(display.persisted).toBeNull();
    expect(display.body).toBe(text);
  });

  it("compact 为 null 时回落到文本解析与阈值折叠", () => {
    const display = resolveResultDisplay("plain text", null);
    expect(display.source).toBe("text");
    expect(display.head).toBeNull();
    expect(display.persisted).toBeNull();
    expect(display.body).toBe("plain text");
    expect(display.defaultCollapsed).toBe(false);
  });

  it("compact 为 null 时落盘标签仍可兜底解析", () => {
    const display = resolveResultDisplay(
      '<persisted-output path="store/c1.txt">\npreview',
      undefined,
    );
    expect(display.source).toBe("text");
    expect(display.persisted).toEqual({ path: "store/c1.txt", bytes: null });
    expect(display.body).toBe("preview");
  });
});

describe("shouldRenderDocStub", () => {
  const anchor = (path: string): ToolResultCompactDto => ({
    head: "ok edit a.ts · 2 replacements",
    truncated: null,
    persisted: null,
    errorKind: null,
    anchor: { path, ranges: [{ start: 12, end: 20 }] },
  });

  it("锚点命中当前打开的文件才走批注引用", () => {
    expect(shouldRenderDocStub({ compact: anchor("a.ts") }, "a.ts")).toBe(true);
  });

  it("面板未打开或打开别的文件时走卡片", () => {
    expect(shouldRenderDocStub({ compact: anchor("a.ts") }, null)).toBe(false);
    expect(shouldRenderDocStub({ compact: anchor("a.ts") }, undefined)).toBe(
      false,
    );
    expect(shouldRenderDocStub({ compact: anchor("a.ts") }, "b.ts")).toBe(
      false,
    );
  });

  it("无锚点或 compact 缺失时走卡片", () => {
    expect(
      shouldRenderDocStub(
        { compact: { ...anchor("a.ts"), anchor: null } },
        "a.ts",
      ),
    ).toBe(false);
    expect(shouldRenderDocStub({ compact: null }, "a.ts")).toBe(false);
    expect(shouldRenderDocStub({}, "a.ts")).toBe(false);
  });

  it("同一消息内按各自锚点独立选路", () => {
    const withAnchor = { compact: anchor("a.ts") };
    const withoutAnchor = { compact: { ...anchor("a.ts"), anchor: null } };
    expect(shouldRenderDocStub(withAnchor, "a.ts")).toBe(true);
    expect(shouldRenderDocStub(withoutAnchor, "a.ts")).toBe(false);
  });
});
