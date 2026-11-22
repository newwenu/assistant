import { afterEach, describe, expect, it, vi } from "vitest";
import zhCN from "@/i18n/locales/zh-CN.json";
import enUS from "@/i18n/locales/en-US.json";
import {
  actionText,
  availabilityText,
  cleanupErrorCodeText,
  eligibilityText,
  entryNameText,
  groupNameText,
  inventoryClassText,
  layerText,
  policyChips,
  policySourceText,
  reasonText,
} from "../lexicon";
import { makePolicy } from "./fixtures";

const ENTRY_KEYS = [
  "attachments",
  "attachments/derived",
  "tool-results",
  "tool-index",
  "mascot-assets",
  "assistant.db",
  "logs",
  "crashes",
  "EBWebView",
  "config.json",
  "orb-config.json",
  "mascot-config.json",
] as const;

const ENTRY_NAME_PATHS = [
  "attachments",
  "attachments_derived",
  "tool_results",
  "tool_index",
  "mascot_assets",
  "assistant_db",
  "security_security_db",
  "security_backups",
  "security_vault",
  "security_recovery",
  "security_audit",
  "logs",
  "crashes",
  "EBWebView",
  "config_json",
  "orb_config_json",
  "mascot_config_json",
] as const;

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
  it("12 个核心条目 key 全部有本地化名称", () => {
    for (const key of ENTRY_KEYS) {
      const name = entryNameText(key);
      expect(name.length).toBeGreaterThan(0);
      expect(name).not.toBe(key);
    }
  });

  it("条目名称键在 zh-CN 与 en-US 都存在", () => {
    const missing: string[] = [];
    for (const name of ENTRY_NAME_PATHS) {
      const path = `settings.storage.entries.names.${name}`;
      if (!localeHas(zhCN, path)) missing.push(`zh-CN:${path}`);
      if (!localeHas(enUS, path)) missing.push(`en-US:${path}`);
    }
    expect(missing).toEqual([]);
  });

  it("分组名命中覆盖表时本地化，未命中静默回退原文", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(groupNameText("security")).not.toBe("security");
    expect(groupNameText("future-group")).toBe("future-group");
    expect(warn).not.toHaveBeenCalled();
  });

  it("未知条目 key 静默回退原文", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(entryNameText("future-root")).toBe("future-root");
    expect(warn).not.toHaveBeenCalled();
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
    expect(cleanupErrorCodeText("futureValue")).toBe("futureValue");
    expect(warn).toHaveBeenCalledTimes(8);
  });

  it("原型键不误命中词表", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    for (const prototypeKey of ["constructor", "toString", "__proto__"]) {
      expect(entryNameText(prototypeKey)).toBe(prototypeKey);
      expect(availabilityText(prototypeKey)).toBe(prototypeKey);
      expect(reasonText(prototypeKey)).toBe(prototypeKey);
      expect(cleanupErrorCodeText(prototypeKey)).toBe(prototypeKey);
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
    expect(cleanupErrorCodeText("io")).not.toBe("io");
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
