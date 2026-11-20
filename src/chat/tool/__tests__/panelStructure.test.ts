import { describe, expect, it } from "vitest";
import { readRepoFile } from "@/test-utils/paths";

const PANEL_FILE = "src/chat/tool/ToolConfigPanel.tsx";
const PANEL_CSS = "src/chat/tool/ToolConfigPanel.module.css";
const ASSEMBLY_FILE = "src/settings/shared/sections/ToolAssemblySection.tsx";
const ASSEMBLY_CSS =
  "src/settings/shared/sections/ToolAssemblySection.module.css";
const LOCALES = ["src/i18n/locales/zh-CN.json", "src/i18n/locales/en-US.json"];

/** 取 `from` 起点到 `to` 标记之间的源码片段，用来把单个分支体单独拿出来断言。 */
function slice(text: string, from: string, to: string): string {
  const start = text.indexOf(from);
  expect(start, `缺少片段起点：${from}`).toBeGreaterThanOrEqual(0);
  const end = text.indexOf(to, start);
  expect(end, `缺少片段终点：${to}`).toBeGreaterThan(start);
  return text.slice(start, end);
}

/** 取 CSS 里单条选择器的声明块（本文件的选择器都不嵌套）。 */
function cssBlock(text: string, selector: string): string {
  const start = text.indexOf(`${selector} {`);
  expect(start, `缺少选择器：${selector}`).toBeGreaterThanOrEqual(0);
  const end = text.indexOf("}", start);
  expect(end, `选择器 ${selector} 没有闭合`).toBeGreaterThan(start);
  return text.slice(start, end);
}

function pick(root: unknown, keys: string[]): unknown {
  let node = root;
  for (const key of keys) {
    if (typeof node !== "object" || node === null) return undefined;
    node = (node as Record<string, unknown>)[key];
  }
  return node;
}

/**
 * 面板的伴随展示与元工具勾选（I58 §3.6 / §10 G20）。
 *
 * 仓库测试环境是 node + 空 `window`（`vitest.config.ts` 无 `environment`、`src/testSetup.ts`
 * 只补空对象），没有 jsdom 与 `@testing-library/react`，且 I48 §9 已明确「不为此引入组件测试基建」。
 * 因此渲染层门禁落在**源码结构 + CSS 声明**上：断言的是「渲染单元怎么折」「哪条分支有勾选框」
 * 这类结构性事实，不断言像素与运行时布局。
 */
describe("聊天面板：伴随条目与元工具（G20）", () => {
  const source = readRepoFile(PANEL_FILE);

  it("伴随条目并入基工具的渲染单元，不会在横向流里被拆成独立条目", () => {
    expect(source).toMatch(
      /groupToolUnits\(items\)\.map\(\(unit\) => \(\s*<div key=\{unit\.head\.id\} className=\{css\.toolUnit\}>[\s\S]*?unit\.companions\.map[\s\S]*?<\/div>/,
    );
  });

  it("伴随条目没有勾选框，也不写 enabledTools / disabledTools", () => {
    const branch = slice(
      source,
      "const baseId = tool.companionOf;",
      "if (isMetaTool(tool)) {",
    );
    expect(branch).not.toContain("Checkbox");
    expect(branch).not.toContain("onChange");
    expect(branch).not.toContain("setTierSelection");
    expect(branch).toContain("css.companion");
    expect(branch).toContain("chat.tool.panel.companion_of");
    expect(branch).toContain("chat.tool.panel.companion_blocked");
  });

  it("元工具是可勾选条目，带「申请入口」标签并写回勾选集合", () => {
    const branch = slice(
      source,
      "if (isMetaTool(tool)) {",
      "\n    }\n    return (",
    );
    expect(branch).toContain("<Checkbox");
    expect(branch).toContain("css.metaBadge");
    expect(branch).toContain("chat.tool.panel.meta_badge");
    expect(branch).toContain("setTierSelection");
  });

  it("已启用计数走可勾选口径，伴随项不撑大分子分母", () => {
    expect(source).toContain(
      "const enabledCount = countEnabledSelectableTools(config, tools);",
    );
    expect(source).toContain(
      't("chat.tool.panel.enabled_count", { count: enabledCount })',
    );
    expect(source).not.toContain("config.enabledTools.length");
    expect(source).not.toContain("tools.length");
  });

  it("CSS：列表换行、单元不可断行、伴随条目只占一个名字宽度", () => {
    const list = cssBlock(readRepoFile(PANEL_CSS), ".toolList");
    expect(list).toContain("flex-wrap: wrap");
    const unit = cssBlock(readRepoFile(PANEL_CSS), ".toolUnit");
    expect(unit).toContain("display: inline-flex");
    const companion = cssBlock(readRepoFile(PANEL_CSS), ".companion");
    expect(companion).toContain("white-space: nowrap");
    expect(companion).toContain("--color-text-tertiary");
  });
});

/**
 * 设置页的模式级面板是纵向列表，没有横向换行的拆行问题，因此布局刻意不同
 * （缩进 + 只读状态点），但**语义同口径**：无勾选框、不写名单、计数不计。
 */
describe("设置页面板：语义同口径（G20）", () => {
  const source = readRepoFile(ASSEMBLY_FILE);

  it("伴随条目是缩进的只读从属行，没有勾选框", () => {
    const branch = slice(
      source,
      "const baseId = tool.companionOf;",
      "if (isMetaTool(tool)) {",
    );
    expect(branch).not.toContain("Checkbox");
    expect(branch).not.toContain("onChange");
    expect(branch).not.toContain("setTierSelection");
    expect(branch).toContain("css.companionRow");
    expect(branch).toContain("css.statusDot");
    expect(branch).toContain("settings.agent_mode.tool_assembly.companion_of");
    expect(branch).toContain(
      "settings.agent_mode.tool_assembly.companion_blocked",
    );
  });

  it("CSS：从属行相对基工具缩进一级", () => {
    const row = cssBlock(readRepoFile(ASSEMBLY_CSS), ".companionRow");
    expect(row).toContain("margin-inline-start: 22px");
  });

  it("元工具是可勾选条目，带「申请入口」标签与一句话说明", () => {
    const branch = slice(
      source,
      "if (isMetaTool(tool)) {",
      "\n                      }\n                      return (",
    );
    expect(branch).toContain("<Checkbox");
    expect(branch).toContain("css.metaBadge");
    expect(branch).toContain("settings.agent_mode.tool_assembly.meta_badge");
    expect(branch).toContain("settings.agent_mode.tool_assembly.meta_hint");
    expect(branch).toContain("setTierSelection");
  });

  it("已启用计数同口径", () => {
    expect(source).toContain(
      "count: countEnabledSelectableTools(config, tools),",
    );
    expect(source).not.toContain("config.enabledTools.length");
  });
});

describe("伴随与元工具文案（中英一致）", () => {
  /** 聊天面板只用到前三个键（元工具说明走 `TOOL_DESC_KEYS`），设置页多一个一句话说明。 */
  const namespaces = [
    {
      path: ["chat", "tool", "panel"],
      keys: ["companion_of", "companion_blocked", "meta_badge"],
    },
    {
      path: ["settings", "agent_mode", "tool_assembly"],
      keys: ["companion_of", "companion_blocked", "meta_badge", "meta_hint"],
    },
  ];

  it("两个命名空间在两个语种里都具备各自的键", () => {
    for (const locale of LOCALES) {
      const raw = JSON.parse(readRepoFile(locale)) as unknown;
      for (const namespace of namespaces) {
        for (const name of namespace.keys) {
          const value = pick(raw, [...namespace.path, name]);
          expect(
            typeof value === "string" && value.length > 0,
            `${locale} 缺少 ${namespace.path.join(".")}.${name}`,
          ).toBe(true);
        }
      }
    }
  });

  it("从属状态文案都带基工具占位符", () => {
    for (const locale of LOCALES) {
      const raw = JSON.parse(readRepoFile(locale)) as unknown;
      for (const namespace of namespaces) {
        for (const name of ["companion_of", "companion_blocked"]) {
          expect(String(pick(raw, [...namespace.path, name]))).toContain(
            "{{base}}",
          );
        }
      }
    }
  });
});

/**
 * A33（用户侧）：`shell` 面板短句在 zh / en 两份里都存在，且面板确实用它。
 *
 * 与 Rust 侧 `a33_description_covers_the_six_contracts` 是**两个不同断言对象**：
 * 那边是模型读的 `description()`（6 项契约，长文本），这里是悬停一句话。
 * 若面板不登记这个 id，会回落到 `description()`——十几行提示词塞进小浮层。
 */
describe("shell 面板说明（A33）", () => {
  const source = readRepoFile(PANEL_FILE);

  it("面板登记了 shell 的短说明键", () => {
    expect(source).toContain('shell: "chat.tool.panel.tool_desc.shell",');
  });

  it("两个语种都有一句话说明，且不重复模型的整段描述", () => {
    for (const locale of LOCALES) {
      const raw = JSON.parse(readRepoFile(locale)) as unknown;
      const value = pick(raw, ["chat", "tool", "panel", "tool_desc", "shell"]);
      expect(
        typeof value === "string" && value.length > 0,
        `${locale} 缺少 chat.tool.panel.tool_desc.shell`,
      ).toBe(true);
      const text = String(value);
      expect(text.length).toBeLessThan(120);
      expect(text).not.toContain("\n");
    }
  });
});
