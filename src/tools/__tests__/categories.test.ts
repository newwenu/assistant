import { describe, expect, it } from "vitest";
import { classifyKeywords } from "@/tools/framework/categories";
import { tools } from "@/tools/framework/registry";

const EXPECTED_CATEGORIES: Record<string, string> = {
  "hash-text": "security",
  "uuid-generator": "dev",
  "base64-string-converter": "converter",
  "color-converter": "converter",
  "date-time-converter": "converter",
  "integer-base-converter": "converter",
  "url-encoder": "converter",
  "json-viewer": "dev",
  "regex-tester": "dev",
  "jwt-parser": "security",
  "sql-formatter": "dev",
  "cron-parser": "dev",
  "text-diff": "dev",
  "text-statistics": "dev",
  "qrcode-generator": "media",
  calculator: "dev",
  "unit-converter": "converter",
  "password-generator": "security",
  "ip-info": "dev",
  "image-compressor": "media",
  "svg-preview": "media",
  "fs-link": "dev",
  timekeeper: "productivity",
};

describe("工具分类回归", () => {
  it("既有工具的归类结果不因新增 productivity 而改变", () => {
    for (const tool of tools) {
      if (tool.id === "timekeeper") continue;
      expect(tool.category, tool.id).toBe(EXPECTED_CATEGORIES[tool.id]);
    }
  });

  it("timekeeper 的 keywords 归入 productivity", () => {
    expect(
      classifyKeywords([
        "pomodoro",
        "focus",
        "countdown",
        "stopwatch",
        "session",
        "break",
      ]),
    ).toBe("productivity");
  });
});
