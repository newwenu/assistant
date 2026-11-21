import { describe, expect, it } from "vitest";
import zhCN from "@/i18n/locales/zh-CN.json";
import enUS from "@/i18n/locales/en-US.json";
import { readRepoFile } from "@/test-utils/paths";
import {
  VAULT_AUDIT_EVENT_TYPES,
  vaultAuditEventText,
} from "../audit/vaultEvents";

function displayStrings(): string[] {
  const source = readRepoFile(
    "src-tauri/crates/assistant-vault/src/audit/mod.rs",
  );
  const start = source.indexOf("impl fmt::Display for AuditEvent");
  const end = source.indexOf("pub fn log(", start);
  const block = source.slice(start, end);
  return Array.from(block.matchAll(/write!\(f, "([^"]+)"\)/g), (m) => m[1]);
}

function localeHas(locale: unknown, key: string): boolean {
  let node: unknown = locale;
  for (const segment of key.split(".")) {
    if (typeof node !== "object" || node === null) return false;
    node = (node as Record<string, unknown>)[segment];
  }
  return typeof node === "string" && node.length > 0;
}

describe("vault 审计事件词表", () => {
  it("事件字典与后端 AuditEvent 的 Display 串全量对齐", () => {
    const backend = displayStrings();
    expect(backend.length).toBeGreaterThan(0);
    expect(new Set(backend).size).toBe(backend.length);
    expect(new Set(backend)).toEqual(new Set(VAULT_AUDIT_EVENT_TYPES));
  });

  it("每个事件在 zh-CN 与 en-US 都有文案", () => {
    const missing: string[] = [];
    for (const event of VAULT_AUDIT_EVENT_TYPES) {
      const key = `vault.security_log.event.${event}`;
      if (!localeHas(zhCN, key)) missing.push(`zh-CN:${key}`);
      if (!localeHas(enUS, key)) missing.push(`en-US:${key}`);
    }
    expect(missing).toEqual([]);
  });

  it("已知值返回翻译文案，不回显键名或原串", () => {
    for (const event of VAULT_AUDIT_EVENT_TYPES) {
      const text = vaultAuditEventText(event);
      expect(text).not.toBe(`vault.security_log.event.${event}`);
      expect(text).not.toBe(event);
    }
  });

  it("未知值原样兜底，不过滤也不留空", () => {
    expect(vaultAuditEventText("vault_future_event")).toBe(
      "vault_future_event",
    );
    expect(vaultAuditEventText("")).toBe("");
  });
});
