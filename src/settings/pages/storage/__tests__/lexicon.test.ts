import { afterEach, describe, expect, it, vi } from "vitest";
import zhCN from "@/i18n/locales/zh-CN.json";
import enUS from "@/i18n/locales/en-US.json";
import {
  ENTRY_KEYS,
  actionText,
  availabilityText,
  eligibilityText,
  entryIdText,
  entryNameText,
  inventoryClassText,
  layerText,
  policyChips,
  policySourceText,
  reasonText,
  reclaimErrorCodeText,
} from "../lexicon";
import { makePolicy } from "./fixtures";

function localeHas(locale: unknown, key: string): boolean {
  let node: unknown = locale;
  for (const segment of key.split(".")) {
    if (typeof node !== "object" || node === null) return false;
    node = (node as Record<string, unknown>)[segment];
  }
  return typeof node === "string" && node.length > 0;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("存储词表", () => {
  it("13 个条目 key 全部有 id 与名称", () => {
    const ids = new Set<string>();
    for (const key of ENTRY_KEYS) {
      const id = entryIdText(key);
      expect(id).not.toBeNull();
      ids.add(id as string);

      const name = entryNameText(key);
      expect(name.length).toBeGreaterThan(0);
      expect(name).not.toBe(key);
      expect(name).not.toBe(`settings.storage.entries.names.${id}`);
    }
    expect(ids.size).toBe(13);
    expect(ids).toEqual(
      new Set(Array.from({ length: 13 }, (_, i) => `E${i + 1}`)),
    );
  });

  it("条目名称键在 zh-CN 与 en-US 都存在", () => {
    const missing: string[] = [];
    for (const key of ENTRY_KEYS) {
      const id = entryIdText(key);
      const path = `settings.storage.entries.names.${(id as string).toLowerCase()}`;
      if (!localeHas(zhCN, path)) missing.push(`zh-CN:${path}`);
      if (!localeHas(enUS, path)) missing.push(`en-US:${path}`);
    }
    expect(missing).toEqual([]);
  });

  it("未知条目 key 回退原文并告警", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(entryIdText("future-root")).toBeNull();
    expect(entryNameText("future-root")).toBe("future-root");
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it("未知枚举值回退原文，不显示空白也不猜文案", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(availabilityText("futureValue")).toBe("futureValue");
    expect(eligibilityText("futureValue")).toBe("futureValue");
    expect(reasonText("futureValue")).toBe("futureValue");
    expect(actionText("futureValue")).toBe("futureValue");
    expect(policySourceText("futureValue")).toBe("futureValue");
    expect(inventoryClassText("futureValue")).toBe("futureValue");
    expect(layerText("futureValue")).toBe("futureValue");
    expect(reclaimErrorCodeText("futureValue")).toBe("futureValue");
    expect(warn).toHaveBeenCalledTimes(8);
  });

  it("原型键不误命中词表", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    for (const prototypeKey of ["constructor", "toString", "__proto__"]) {
      expect(entryIdText(prototypeKey)).toBeNull();
      expect(availabilityText(prototypeKey)).toBe(prototypeKey);
      expect(reasonText(prototypeKey)).toBe(prototypeKey);
      expect(reclaimErrorCodeText(prototypeKey)).toBe(prototypeKey);
    }
    expect(warn).toHaveBeenCalled();
  });

  it("已知值返回文案而不是键名或原串", () => {
    expect(availabilityText("available")).not.toBe("available");
    expect(eligibilityText("eligible")).not.toBe("eligible");
    expect(reasonText("orphan")).not.toBe("orphan");
    expect(actionText("deleteOrphans")).not.toBe("deleteOrphans");
    expect(policySourceText("config")).not.toBe("config");
    expect(inventoryClassText("unregistered")).not.toBe("unregistered");
    expect(layerText("local")).not.toBe("local");
    expect(reclaimErrorCodeText("io")).not.toBe("io");
  });

  it("策略 chip 顺序为来源在前、自动执行在末，保留天数 0 表示禁用", () => {
    expect(policyChips(makePolicy(), false)).toHaveLength(1);
    expect(policyChips(makePolicy({ retentionDays: 0 }), false).length).toBe(2);
    expect(policyChips(makePolicy({ retentionDays: 30 }), false).length).toBe(
      2,
    );
    expect(
      policyChips(
        makePolicy({ retentionDays: 30, keepCount: 5, maxBytes: 1024 }),
        false,
      ).length,
    ).toBe(4);
    expect(policyChips(makePolicy({ retentionDays: 30 }), true).length).toBe(3);
    expect(policyChips(makePolicy({ retentionDays: 0 }), false)[1]).not.toBe(
      policyChips(makePolicy({ retentionDays: 30 }), false)[1],
    );
  });
});
