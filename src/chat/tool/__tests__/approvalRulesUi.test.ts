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

const CARD_CSS_FILE = "src/chat/tool/ApprovalCard.module.css";
const PRESENTATION_DIR = "src/chat/tool/approvalPresentation";
const PRESENTATION_FILES = [
  "index.ts",
  "types.ts",
  "tables.ts",
  "text.ts",
  "elements.ts",
  "evidence.ts",
  "diagnostics.ts",
  "detail.ts",
  "view.ts",
];

/** 递归收集某节点下全部字符串叶子，键为相对路径。 */
function stringLeaves(root: unknown, keys: string[]): Record<string, string> {
  const out: Record<string, string> = {};
  const walk = (value: unknown, prefix: string) => {
    if (typeof value === "string") {
      out[prefix] = value;
      return;
    }
    if (typeof value !== "object" || value === null) return;
    for (const [key, child] of Object.entries(value)) {
      walk(child, prefix === "" ? key : `${prefix}.${key}`);
    }
  };
  walk(pick(root, keys), "");
  return out;
}

function cssRuleBody(source: string, className: string): string {
  const start = source.indexOf(`.${className} {`);
  expect(start, `缺少样式：.${className}`).toBeGreaterThanOrEqual(0);
  return source.slice(start, source.indexOf("}", start));
}

describe("卡片四区块结构（I72 §4 / B14–B17）", () => {
  const card = readRepoFile(CARD_FILE);
  const cardCss = readRepoFile(CARD_CSS_FILE);

  it("只有决策句 / 命令块 / 按钮 / 折叠区四个区块，无 row 平铺（B14）", () => {
    for (const block of [
      "css.decision",
      "css.commandBlock",
      "css.actions",
      "css.fold",
    ]) {
      expect(card, block).toContain(block);
    }
    for (const stale of [
      "css.row",
      "css.preview",
      "css.intents",
      "css.impact",
      "css.extensions",
      "css.reasons",
    ]) {
      expect(card, stale).not.toContain(stale);
    }
    for (const staleRule of [
      "row",
      "label",
      "intent",
      "extension",
      "alwaysHint",
    ]) {
      expect(cardCss, staleRule).not.toMatch(
        new RegExp(`\\.${staleRule}\\s*{`),
      );
    }
  });

  it("折叠区按 §4.5 固定顺序渲染，入口文案按实际区块拼接（B14）", () => {
    const detail = readRepoFile(`${PRESENTATION_DIR}/detail.ts`);
    const start = detail.indexOf("export function foldSections");
    expect(start).toBeGreaterThanOrEqual(0);
    const body = detail.slice(start);
    let cursor = 0;
    for (const section of [
      "why",
      "scope",
      "evidence",
      "arguments",
      "diagnostics",
      "denyConsequence",
    ]) {
      const index = body.indexOf(`"${section}"`, cursor);
      expect(index, `区块顺序：${section}`).toBeGreaterThanOrEqual(cursor);
      cursor = index;
    }
    expect(card).toContain("foldSections.map(renderFoldSection)");
    expect(card).toContain("translateKey(foldLabelKey(section))");
    expect(card).toContain('.join(" · ")');
  });

  it("证据块保留换行且等宽（B15）", () => {
    for (const className of ["commandBlock", "evidence"]) {
      const body = cssRuleBody(cardCss, className);
      expect(body, className).toContain("white-space: pre-wrap");
      expect(body, className).toContain("var(--font-mono)");
    }
  });

  it("决策按钮仍为四个，文案键不变（B16）", () => {
    for (const key of ["approve", "deny", "session_allow", "always"]) {
      expect(card, key).toContain(`chat.tool.approval.${key}`);
    }
    for (const kind of ["approve", "allowSession", "allowAlways"]) {
      expect(card, kind).toContain(`decideOther("${kind}")`);
    }
    expect(card).toContain("onClick={decideDeny}");
    expect(card).toContain('void decide("deny"');
  });

  it("always_hint 固定文案不再被引用，改为按需范围说明（B17）", () => {
    expect(card).not.toContain("always_hint");
    expect(card).toContain("view.scope.session");
    expect(card).toContain("view.scope.always");
    for (const [name, locale] of [
      ["zh-CN", zhCN],
      ["en-US", enUS],
    ] as const) {
      expect(
        pick(locale, ["chat", "tool", "approval", "always_hint"]),
        name,
      ).toBeUndefined();
    }
  });

  it("等待审批态隐藏上方 name + argsSummary 行", () => {
    const bubble = readRepoFile("src/chat/tool/ToolCallBubble.tsx");
    expect(bubble).toMatch(/!awaitingApproval && argsSummary/);
    expect(bubble).toMatch(
      /!awaitingApproval && <span className=\{css\.name\}/,
    );
  });
});

describe("视图模型纯度（I72 §10.1 / B18）", () => {
  it("整组模块不 import React、不引用 store 与工作区根", () => {
    for (const file of PRESENTATION_FILES) {
      const source = readRepoFile(`${PRESENTATION_DIR}/${file}`);
      expect(source, file).not.toMatch(
        /from "react"|from "react-dom"|from "react-i18next"/,
      );
      expect(source, file).not.toContain("useChatStore");
      expect(source, file).not.toContain("workspaceRoot");
    }
  });

  it("不按 toolName 分支，签名不接收工作区根", () => {
    for (const file of PRESENTATION_FILES) {
      const source = readRepoFile(`${PRESENTATION_DIR}/${file}`);
      expect(source, file).not.toMatch(/toolName\s*===\s*"/);
      expect(source, file).not.toMatch(/toolName\s*===\s*'|toolName\s*!==\s*"/);
    }
    const view = readRepoFile(`${PRESENTATION_DIR}/view.ts`);
    expect(view).toContain("approval: ToolApprovalRequest");
    expect(view).toContain("t: TranslateFn");
    expect(view).not.toMatch(/root|Root/);
  });
});

describe("审批文案双语对齐（I72 §10.1 / C19）", () => {
  const GROUPS: string[][] = [
    ["chat", "tool", "approval", "verb"],
    ["chat", "tool", "approval", "consequence"],
    ["chat", "tool", "approval", "scale"],
    ["chat", "tool", "approval", "boundary"],
    ["chat", "tool", "approval", "object"],
    ["chat", "tool", "approval", "intent"],
    ["chat", "tool", "approval", "extension"],
    ["chat", "tool", "approval", "fold"],
    ["chat", "tool", "approval", "scope"],
    ["chat", "tool", "approval", "diagnostic"],
    ["chat", "tool", "approval", "evidence"],
  ];

  const DIRECT_KEYS = [
    "always_confirm",
    "expired_decision",
    "deny_reason_open",
    "deny_reason_close",
    "deny_reason_placeholder",
    "deny_reason_hint",
  ];

  it("新增键集合完全对齐，占位符集合一致", () => {
    for (const keys of GROUPS) {
      const zh = stringLeaves(zhCN, keys);
      const en = stringLeaves(enUS, keys);
      const label = keys.join(".");
      expect(Object.keys(zh).sort(), label).toEqual(Object.keys(en).sort());
      for (const key of Object.keys(zh)) {
        expect(placeholders(en[key]), `${label}.${key}`).toEqual(
          placeholders(zh[key]),
        );
      }
    }
  });

  it("散键（二次确认 / 超时 / 拒绝理由）双语齐备且占位符一致", () => {
    for (const key of DIRECT_KEYS) {
      const zh = text(zhCN, ["chat", "tool", "approval", key]);
      const en = text(enUS, ["chat", "tool", "approval", key]);
      expect(placeholders(en), key).toEqual(placeholders(zh));
    }
    expect(
      placeholders(text(zhCN, ["chat", "tool", "approval", "always_confirm"])),
    ).toEqual(["intents"]);
  });
});

describe("拒绝理由（I72 §8.5 / E22–E28）", () => {
  const card = readRepoFile(CARD_FILE);

  const functionBody = (signature: string): string => {
    const start = card.indexOf(signature);
    expect(start, signature).toBeGreaterThanOrEqual(0);
    return card.slice(start, card.indexOf("\n  };", start));
  };

  it("默认路径零成本：拒绝单击即提交，无输入框、无二次确认（E22）", () => {
    expect(card).toContain("onClick={decideDeny}");
    const deny = functionBody("const decideDeny = () => {");
    expect(deny).toContain('void decide("deny",');
    const decide = functionBody("const decide = async");
    expect(decide).toContain('if (kind === "deny" && reason !== undefined)');
    expect(decide).toContain("payload.reason = reason");
    expect((card.match(/<Popconfirm/g) ?? []).length).toBe(1);
    expect(card).toContain('onConfirm={() => decideOther("allowAlways")}');
  });

  it("空白理由按未填写处理，不传空串（E23）", () => {
    const deny = functionBody("const decideDeny = () => {");
    expect(deny).toContain("const trimmed = denyReason.trim();");
    expect(deny).toContain('trimmed === "" ? undefined : trimmed');
  });

  it("理由为 trim 后原文，且拒绝按钮不带二次确认（E24）", () => {
    expect(card).toContain(
      'void decide("deny", trimmed === "" ? undefined : trimmed)',
    );
    const denyStart = card.indexOf('loading={submitting === "deny"}');
    expect(denyStart).toBeGreaterThanOrEqual(0);
    const denyButton = card.slice(
      denyStart,
      card.indexOf("</Button>", denyStart),
    );
    expect(denyButton).not.toContain("Popconfirm");
  });

  it("放行类动作不携带理由，展开的理由框自动收起（E25）", () => {
    const other = functionBody("const decideOther = (kind: DecisionKind) => {");
    expect(other).toContain("setDenyReasonOpen(false);");
    expect(other).toContain('setDenyReason("");');
    expect(other).toContain("void decide(kind);");
    for (const kind of ["approve", "allowSession", "allowAlways"]) {
      expect(card, kind).toContain(`decideOther("${kind}")`);
    }
    expect(card).not.toContain('decideOther("deny")');
  });

  it("单行输入限长 200、自动聚焦、回车不提交（E26）", () => {
    expect(card).toContain("maxLength={200}");
    expect(card).toContain("autoFocus");
    expect(card).toContain("onPressEnter={(event) => event.preventDefault()}");
    expect(card).toContain("chat.tool.approval.deny_reason_placeholder");
  });

  it("提交失败保留展开与已输入内容（E27）", () => {
    const decide = functionBody("const decide = async");
    const catchStart = decide.indexOf("catch (err)");
    expect(catchStart).toBeGreaterThanOrEqual(0);
    const catchBody = decide.slice(catchStart);
    expect(catchBody).toContain("setError(errorText(err));");
    expect(catchBody).not.toContain("setDenyReasonOpen");
    expect(catchBody).not.toContain("setDenyReason(");
  });

  it("超时态理由入口与按钮一起隐藏（E28）", () => {
    const entryStart = card.indexOf("className={css.denyReasonEntry}");
    expect(entryStart).toBeGreaterThanOrEqual(0);
    const before = card.slice(0, entryStart);
    const guard = before.lastIndexOf("{!expired && (");
    expect(guard).toBeGreaterThanOrEqual(0);
    expect(entryStart - guard).toBeLessThan(400);
    expect(card).toContain("disabled={submitting !== null}");
    expect(card).toContain("{denyReasonOpen && !expired && (");
  });
});
