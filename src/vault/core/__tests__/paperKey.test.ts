import { describe, expect, it } from "vitest";
import {
  PAPER_CHARS,
  PAPER_GROUP_LEN,
  formatPaperKey,
  groupPaperKey,
  normalizePaperInput,
  paperInputError,
} from "../display/paperKey";

const SAMPLE = "0123456789ABCDEFGHJKMNPQRSTVWXYZ01234567";

describe("纸面密钥归一化", () => {
  it("小写、连字符、空格被剥离并转大写", () => {
    const raw = "0123-4567 89ab cdef ghjk mnpq rstv wxyz 0123 4567";
    expect(normalizePaperInput(raw)).toBe(SAMPLE);
  });

  it("全角数字与全角字母映射为半角", () => {
    const raw =
      "０１２３４５６７８９ＡＢＣＤＥＦＧＨＪＫＭＮＰＱＲＳＴＶＷＸＹＺ０１２３４５６７";
    expect(normalizePaperInput(raw)).toBe(SAMPLE);
  });

  it("O/I/L/U 混淆字符按 Crockford 规则映射", () => {
    expect(normalizePaperInput("OILU")).toBe("011V");
  });

  it("全角空格与制表符被剥离", () => {
    expect(normalizePaperInput("0123\u30004567\t89AB")).toBe("0123456789AB");
  });
});

describe("纸面密钥分组", () => {
  it("按 8 字符分 5 组", () => {
    const groups = groupPaperKey(SAMPLE);
    expect(groups).toHaveLength(5);
    expect(groups[0]).toBe("01234567");
    expect(groups[4]).toBe("01234567");
  });

  it("格式化后为 5 段连字符连接", () => {
    expect(formatPaperKey(SAMPLE)).toBe(
      "01234567-89ABCDEF-GHJKMNPQ-RSTVWXYZ-01234567",
    );
  });

  it("分组长度恒为 8，末组不补位", () => {
    const groups = groupPaperKey("0123456789");
    expect(groups).toEqual(["01234567", "89"]);
    expect(PAPER_GROUP_LEN).toBe(8);
    expect(PAPER_CHARS).toBe(40);
  });
});

describe("纸面密钥输入校验", () => {
  it("长度不足报 short", () => {
    expect(paperInputError(SAMPLE.slice(0, 39))).toBe("short");
  });

  it("长度超出报 long", () => {
    expect(paperInputError(SAMPLE + "0")).toBe("long");
  });

  it("含字母表外字符报 char", () => {
    expect(paperInputError(SAMPLE.slice(0, 39) + "!")).toBe("char");
  });

  it("合法输入返回 null", () => {
    expect(paperInputError(SAMPLE)).toBeNull();
  });
});
