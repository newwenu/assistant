import { describe, expect, it } from "vitest";
import zhCN from "@/i18n/locales/zh-CN.json";
import enUS from "@/i18n/locales/en-US.json";
import { readRepoFile } from "@/test-utils/paths";

const CARD_FILE = "src/chat/tool/ApprovalCard.tsx";
const EDITOR_FILE = "src/chat/tool/ApprovalRulesEditor.tsx";
const PANEL_FILE = "src/chat/tool/ToolConfigPanel.tsx";
const ASSEMBLY_FILE = "src/settings/shared/sections/ToolAssemblySection.tsx";

const LEVELS = ["none", "workspace_write", "dangerous_only", "all_os_tools"];

function pick(root: unknown, keys: string[]): unknown {
  let node = root;
  for (const key of keys) {
    if (typeof node !== "object" || node === null) return undefined;
    node = (node as Record<string, unknown>)[key];
  }
  return node;
}

/** 该层级下的直接字符串子键。 */
function stringKeys(root: unknown, keys: string[]): string[] {
  const node = pick(root, keys);
  if (typeof node !== "object" || node === null) return [];
  return Object.entries(node as Record<string, unknown>)
    .filter(([, value]) => typeof value === "string")
    .map(([key]) => key)
    .sort();
}

function text(root: unknown, keys: string[]): string {
  const value = pick(root, keys);
  expect(typeof value, `缺少文案：${keys.join(".")}`).toBe("string");
  return value as string;
}

function placeholders(value: string): string[] {
  return Array.from(value.matchAll(/{{(\w+)}}/g), (match) => match[1]).sort();
}

describe("审批原因分支（I70 §7.1 / A8）", () => {
  const source = readRepoFile(CARD_FILE);

  it("reasonText 覆盖全部六种原因且没有 default 兜底", () => {
    const start = source.indexOf("const reasonText = ");
    expect(start).toBeGreaterThanOrEqual(0);
    const body = source.slice(start, source.indexOf("\n  };", start));
    for (const kind of [
      "defaultPolicy",
      "approvalLevel",
      "projectRule",
      "userRule",
      "tierCeiling",
      "advisor",
    ]) {
      expect(body, `缺少分支：${kind}`).toContain(`case "${kind}"`);
    }
    expect(body).not.toContain("default:");
  });

  it("用户规则走独立的 user_rule 文案，不伪装成项目规则", () => {
    const start = source.indexOf('case "userRule"');
    expect(start).toBeGreaterThanOrEqual(0);
    const branch = source.slice(
      start,
      source.indexOf('case "tierCeiling"', start),
    );
    expect(branch).toContain("chat.tool.approval.reason.user_rule");
    expect(branch).not.toContain("project_rule");
  });

  it("两份语言的 user_rule 文案都带 action 与 resource 两个参数", () => {
    for (const [name, locale] of [
      ["zh-CN", zhCN],
      ["en-US", enUS],
    ] as const) {
      const value = text(locale, [
        "chat",
        "tool",
        "approval",
        "reason",
        "user_rule",
      ]);
      expect(placeholders(value), name).toEqual(["action", "resource"]);
    }
  });
});

describe("四档文案（I70 §7.4 / A16）", () => {
  it("设置页模式编辑与工具装配两处选项名齐备", () => {
    for (const [name, locale] of [
      ["zh-CN", zhCN],
      ["en-US", enUS],
    ] as const) {
      expect(
        stringKeys(locale, [
          "settings",
          "agent_mode",
          "field",
          "approval_level",
          "options",
        ]),
        name,
      ).toEqual([...LEVELS].sort());
    }
  });

  it("会话内审批级别与说明两处齐备", () => {
    for (const [name, locale] of [
      ["zh-CN", zhCN],
      ["en-US", enUS],
    ] as const) {
      expect(
        stringKeys(locale, ["chat", "tool", "approval", "level"]),
        name,
      ).toEqual([...LEVELS].sort());
      expect(
        stringKeys(locale, ["chat", "tool", "approval", "level_hint"]),
        name,
      ).toEqual([...LEVELS].sort());
    }
  });

  it("dangerous_only 不再叫「默认 / Default」", () => {
    expect(
      text(zhCN, ["chat", "tool", "approval", "level", "dangerous_only"]),
    ).toBe("谨慎");
    expect(
      text(enUS, ["chat", "tool", "approval", "level", "dangerous_only"]),
    ).toBe("Cautious");
    for (const [name, locale] of [
      ["zh-CN", zhCN],
      ["en-US", enUS],
    ] as const) {
      for (const key of [
        "dangerous_only",
        "none",
        "workspace_write",
        "all_os_tools",
      ]) {
        const option = text(locale, ["chat", "tool", "approval", "level", key]);
        expect(option, `${name}.${key}`).not.toMatch(/默认|Default/i);
      }
    }
  });

  it("workspace_write 说明写出敏感文件例外", () => {
    const zh = text(zhCN, [
      "chat",
      "tool",
      "approval",
      "level_hint",
      "workspace_write",
    ]);
    expect(zh).toContain(".env");
    expect(zh).toContain(".git");
    const en = text(enUS, [
      "chat",
      "tool",
      "approval",
      "level_hint",
      "workspace_write",
    ]);
    expect(en).toContain(".env");
    expect(en).toContain(".git");
  });

  it("none 说明保留「工具输出回读除外」这一例外", () => {
    expect(
      text(zhCN, ["chat", "tool", "approval", "level_hint", "none"]),
    ).toContain("工具输出回读");
    expect(
      text(enUS, ["chat", "tool", "approval", "level_hint", "none"]),
    ).toContain("tool output");
  });

  it("说明文案不使用「危险操作 / 开放式操作」这类工具 flag 措辞", () => {
    for (const locale of [zhCN, enUS]) {
      const hints = pick(locale, ["chat", "tool", "approval", "level_hint"]);
      const joined = Object.values(hints as Record<string, string>).join(" ");
      expect(joined).not.toMatch(/危险操作|开放式操作|destructive|open_world/);
    }
  });
});

describe("规则编辑器复用与语义（I70 §7.3）", () => {
  it("会话级只读展示，写入只在设置页的编辑器里", () => {
    const panel = readRepoFile(PANEL_FILE);
    expect(panel).toContain("ApprovalRulesSummary");
    expect(panel).not.toContain("ApprovalRulesEditor");
    expect(readRepoFile(ASSEMBLY_FILE)).toContain("ApprovalRulesEditor");
  });

  it("只读展示带「去设置编辑」入口，不内嵌输入控件", () => {
    const source = readRepoFile("src/chat/tool/ApprovalRulesSummary.tsx");
    expect(source).toContain("common.approval_rules.edit_in_settings");
    expect(source).not.toMatch(/<Input\b|<Select\b|<Dropdown\b/);
  });

  it("编辑器提供序号与「后写覆盖先写」提示，并区分 deny 行", () => {
    const source = readRepoFile(EDITOR_FILE);
    expect(source).toContain("common.approval_rules.row_index");
    expect(source).toContain("common.approval_rules.order_hint");
    expect(source).toMatch(/rule\.effect === "deny"\s*\?\s*css\.rowDeny/);
  });

  it("效果文案不叫「允许 / 拒绝」", () => {
    for (const [name, locale] of [
      ["zh-CN", zhCN],
      ["en-US", enUS],
    ] as const) {
      const labels = stringKeys(locale, ["common", "approval_rules", "effect"]);
      expect(labels, name).toEqual(["allow", "ask", "deny"]);
      for (const key of labels) {
        const label = text(locale, ["common", "approval_rules", "effect", key]);
        expect(label, `${name}.${key}`).not.toMatch(
          /^允许$|^拒绝$|^Allow$|^Deny$/,
        );
      }
    }
  });

  it("编辑器错误文案与后端校验码一一对应", () => {
    const source = readRepoFile(EDITOR_FILE);
    for (const code of [
      "tooMany",
      "actionLength",
      "resourceLength",
      "allowBottomLine",
      "reservedTier",
    ]) {
      expect(source, code).toContain(`case "${code}"`);
    }
  });

  it("编辑器只在合法状态上报，避免中间态把整条配置打回", () => {
    const source = readRepoFile(EDITOR_FILE);
    expect(source).toMatch(
      /if \(validateApprovalRules\(next\) === null\) onChange\(next\)/,
    );
  });

  it("快捷规则按钮组复用同一份数据，不保存开关状态", () => {
    const source = readRepoFile(EDITOR_FILE);
    expect(source).toContain("APPROVAL_RULE_SHORTCUTS");
    expect(source).toContain("addShortcut");
    expect(source).not.toMatch(/<Switch\b|checked=\{/);
  });

  it("动作下拉只列生产集，不把通配符当动作", () => {
    const source = readRepoFile("src/chat/core/toolConfig.ts");
    const start = source.indexOf("APPROVAL_ACTION_OPTIONS: string[] = [");
    const body = source.slice(start, source.indexOf("];", start));
    expect(body).not.toContain('"*"');
    for (const action of [
      "read",
      "edit",
      "delete",
      "purge",
      "execute",
      "mcp",
      "readToolOutput",
    ]) {
      expect(body, action).toContain(`"${action}"`);
    }
  });

  it("只写 delete 时提示 purge 不覆盖，且不自动合并", () => {
    const source = readRepoFile(EDITOR_FILE);
    expect(source).toContain("delete_purge_hint");
    expect(source).toMatch(
      /draft\.some\(\(rule\) => rule\.action === "delete"\)/,
    );
  });
});
