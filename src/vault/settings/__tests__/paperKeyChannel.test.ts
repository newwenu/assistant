import { describe, expect, it } from "vitest";
import { readRepoFile } from "@/test-utils/paths";
import { buildPaperKeyPrintHtml } from "../recovery/printPaperKey";

const groups = ["K7M2QXF9", "3TBAWZE5", "RNCJ4YD6", "PHK2SVG9", "MTB7XQFD"];

describe("T-P9 恢复码展示通道", () => {
  it("打印内容按 5 组分组展示并包含警告", () => {
    const html = buildPaperKeyPrintHtml({
      title: "密码箱恢复密钥",
      warning: "这张纸就是全部数据。",
      groups,
      meta: "数据密钥版本：1",
      footer: "请离线保存。",
    });

    for (const group of groups) {
      expect(html).toContain(group);
    }
    expect(html).toContain("这张纸就是全部数据。");
    expect(html).toContain("数据密钥版本：1");
  });

  it("打印内容对注入字符做转义", () => {
    const html = buildPaperKeyPrintHtml({
      title: "<script>alert(1)</script>",
      warning: 'a & b " c',
      groups,
      meta: "",
      footer: "",
    });

    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
    expect(html).toContain("a &amp; b &quot; c");
  });

  it("恢复码展示组件不写入剪贴板", () => {
    for (const file of [
      "src/vault/settings/recovery/RecoveryKeyDialog.tsx",
      "src/vault/settings/recovery/RotateDekDialog.tsx",
    ]) {
      const source = readRepoFile(file);
      expect(source).not.toContain("navigator.clipboard");
      expect(source).not.toContain("writeText");
    }
  });
});
