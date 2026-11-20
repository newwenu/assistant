import { describe, expect, it } from "vitest";
import zhCN from "@/i18n/locales/zh-CN.json";
import enUS from "@/i18n/locales/en-US.json";

type LocaleTree = { [key: string]: string | LocaleTree };

function flatten(tree: LocaleTree, prefix = ""): Map<string, string> {
  const out = new Map<string, string>();
  for (const [key, value] of Object.entries(tree)) {
    const path = prefix === "" ? key : `${prefix}.${key}`;
    if (typeof value === "string") {
      out.set(path, value);
    } else {
      for (const [child, childValue] of flatten(value, path)) {
        out.set(child, childValue);
      }
    }
  }
  return out;
}

function placeholders(text: string): string[] {
  return Array.from(text.matchAll(/{{(\w+)}}/g), (match) => match[1]).sort();
}

const zh = flatten(zhCN);
const en = flatten(enUS);

const STORAGE_PREFIX = "settings.storage.";

describe("多语言键对齐", () => {
  it("zh-CN 与 en-US 的键集合完全相等", () => {
    const onlyZh = Array.from(zh.keys()).filter((key) => !en.has(key));
    const onlyEn = Array.from(en.keys()).filter((key) => !zh.has(key));
    expect(onlyZh).toEqual([]);
    expect(onlyEn).toEqual([]);
  });

  it("没有空文案", () => {
    const empty = [...zh.entries(), ...en.entries()]
      .filter(([, value]) => value.trim().length === 0)
      .map(([key]) => key);
    expect(empty).toEqual([]);
  });

  it("存储页的占位符在两份语言里一致", () => {
    const mismatched: string[] = [];
    for (const [key, value] of zh) {
      if (!key.startsWith(STORAGE_PREFIX)) continue;
      const other = en.get(key);
      if (other === undefined) continue;
      if (placeholders(value).join(",") !== placeholders(other).join(",")) {
        mismatched.push(key);
      }
    }
    expect(mismatched).toEqual([]);
  });
});
