import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import zhCN from "@/i18n/locales/zh-CN.json";
import { repoPath } from "@/test-utils/paths";
import type { ToolApprovalRequest } from "@/chat/core/chatTypes";
import {
  buildApprovalView,
  TOOL_PROFILES,
  TOOLBOX_ONLY_IDS,
} from "@/chat/tool/approvalPresentation";

type PreviewDto = NonNullable<ToolApprovalRequest["context"]["preview"]>;
type ShellDto = NonNullable<PreviewDto["shell"]>;

const ABS = "G:\\work\\docs\\report.md";
const ABS_OUT = "G:\\work\\out\\result.txt";

const PREVIEW_BASE: PreviewDto = {
  summary: "",
  truncated: false,
  estimated: false,
  entries: null,
  bytes: null,
  targetKind: null,
  exists: null,
  symlink: null,
  verbCode: null,
  shell: null,
};

function pick(root: unknown, keys: string[]): unknown {
  let node = root;
  for (const key of keys) {
    if (typeof node !== "object" || node === null) return undefined;
    node = (node as Record<string, unknown>)[key];
  }
  return node;
}

function translate(key: string, options?: Record<string, unknown>): string {
  const value = pick(zhCN, key.split("."));
  expect(typeof value, `缺少文案：${key}`).toBe("string");
  return (value as string).replace(/{{(\w+)}}/g, (_, name: string) => {
    const replacement = options?.[name];
    if (typeof replacement === "number") return `${replacement}`;
    return typeof replacement === "string" ? replacement : "";
  });
}

interface ApprovalInput {
  toolName: string;
  args?: Record<string, unknown>;
  intents?: ToolApprovalRequest["intents"];
  preview?: Partial<PreviewDto> | null;
  extensions?: ToolApprovalRequest["context"]["extensions"];
  reasons?: ToolApprovalRequest["context"]["reasons"];
  behavior?: Partial<ToolApprovalRequest["context"]["behavior"]>;
}

function makeApproval(input: ApprovalInput): ToolApprovalRequest {
  return {
    conversationId: "conv-1",
    turnId: "turn-1",
    callId: "call-1",
    toolName: input.toolName,
    arguments: input.args ?? {},
    intents: input.intents ?? [],
    timeoutMs: 60_000,
    context: {
      tier: "os",
      behavior: {
        readOnly: false,
        destructive: false,
        openWorld: false,
        ...input.behavior,
      },
      reasons: input.reasons ?? [],
      preview:
        input.preview == null ? null : { ...PREVIEW_BASE, ...input.preview },
      extensions: input.extensions ?? [],
    },
  };
}

function view(input: ApprovalInput) {
  return buildApprovalView(makeApproval(input), translate);
}

function shellPreview(partial: Partial<ShellDto>): ShellDto {
  return {
    statements: 1,
    chains: 0,
    class: "unknown",
    redirects: [],
    payload: null,
    targetsScope: null,
    confidence: "certain",
    ...partial,
  };
}

function deleteView(
  args: Record<string, unknown>,
  preview: Partial<PreviewDto> | null,
) {
  return view({
    toolName: "delete",
    args,
    intents: [{ action: "delete", resources: [ABS] }],
    preview,
  });
}

function rustToolIds(): string[] {
  const root = repoPath("src-tauri/crates/assistant-tools/src/os/tools");
  const files = fs
    .readdirSync(root, { recursive: true })
    .map((entry) => String(entry))
    .filter((entry) => entry.endsWith(".rs"));
  const ids: string[] = [];
  for (const file of files) {
    const text = fs.readFileSync(path.join(root, file), "utf8");
    for (const match of text.matchAll(/const TOOL_ID: &str = "([^"]+)";/g)) {
      ids.push(match[1]);
    }
  }
  return ids;
}

describe("审批视图模型：决策要素（I72 A1–A10f）", () => {
  it("A1 永久删除：动词「永久删除」且后果含「无法恢复」", () => {
    const result = deleteView(
      { path: ABS, permanent: true },
      { bytes: 3072, targetKind: "file", exists: true, symlink: false },
    );
    expect(result.verb).toBe(
      translate("chat.tool.approval.verb.delete_permanent"),
    );
    expect(result.consequences.join(" · ")).toContain("无法恢复");
  });

  it("A2 普通删除：动词「删除」且后果含「回收站」「可恢复」", () => {
    const result = deleteView(
      { path: ABS, permanent: false },
      { bytes: 3072, targetKind: "file", exists: true, symlink: false },
    );
    expect(result.verb).toBe(translate("chat.tool.approval.verb.delete"));
    const consequence = result.consequences.join(" · ");
    expect(consequence).toContain("回收站");
    expect(consequence).toContain("可恢复");
  });

  it("A3 删除目录：动词「删除目录」，规模段取 preview.entries 总数（wire 只有总条目数，不分文件 / 子目录）", () => {
    const result = deleteView(
      { path: ABS, recursive: true },
      {
        entries: 3,
        bytes: 4_194_304,
        targetKind: "directory",
        exists: true,
        symlink: false,
      },
    );
    expect(result.verb).toBe(translate("chat.tool.approval.verb.delete_dir"));
    expect(result.scale).toEqual([
      translate("chat.tool.approval.scale.entries", { count: 3 }),
      translate("chat.tool.approval.scale.bytes", { bytes: "4.0 MB" }),
    ]);
  });

  it("A3b 永久删除目录走组合查表，不拼成「永久删除删除目录」", () => {
    const result = deleteView(
      { path: ABS, permanent: true, recursive: true },
      { entries: 3, bytes: 1024, targetKind: "directory", exists: true },
    );
    expect(result.verb).toBe(
      translate("chat.tool.approval.verb.delete_dir_permanent"),
    );
    expect(result.verb).not.toContain("删除删除");
  });

  it("A4 write 三种形态：追加到 / 新建 / 覆盖，不出现含糊的「写入」", () => {
    const append = view({
      toolName: "write",
      args: { path: ABS, content: "a\nb\n", append: true },
      intents: [{ action: "edit", resources: [ABS] }],
      preview: { exists: true },
    });
    expect(append.verb).toBe(translate("chat.tool.approval.verb.append"));
    expect(append.consequences.join("")).toContain("已有内容保留");

    const create = view({
      toolName: "write",
      args: { path: ABS, content: "a\n", append: false },
      intents: [{ action: "edit", resources: [ABS] }],
      preview: { exists: false },
    });
    expect(create.verb).toBe(translate("chat.tool.approval.verb.create"));

    const overwrite = view({
      toolName: "write",
      args: { path: ABS, content: "a\n", append: false },
      intents: [{ action: "edit", resources: [ABS] }],
      preview: { exists: true },
    });
    expect(overwrite.verb).toBe(translate("chat.tool.approval.verb.overwrite"));
    expect(overwrite.consequences.join("")).toContain("原内容被替换");

    const degraded = view({
      toolName: "write",
      args: { path: ABS, content: "a\n", append: false },
      intents: [{ action: "edit", resources: [ABS] }],
      preview: { exists: null },
    });
    expect(degraded.verb).toBe(
      translate("chat.tool.approval.verb.create_or_overwrite"),
    );

    for (const result of [append, create, overwrite, degraded]) {
      expect(result.verb).not.toBe("写入");
    }
  });

  it("A5 move 覆盖语义为条件句，默认不覆盖", () => {
    const overwrite = view({
      toolName: "move",
      args: { source: ABS, target: ABS_OUT, overwrite: true },
      intents: [
        { action: "edit", resources: [ABS] },
        { action: "edit", resources: [ABS_OUT] },
      ],
    });
    expect(overwrite.consequences).toEqual([
      translate("chat.tool.approval.consequence.overwrite_if_exists"),
    ]);
    expect(overwrite.consequences[0]).not.toBe("目标已存在");
    const keep = view({
      toolName: "move",
      args: { source: ABS, target: ABS_OUT, overwrite: false },
      intents: [{ action: "edit", resources: [ABS] }],
    });
    expect(keep.consequences).toEqual([
      translate("chat.tool.approval.consequence.no_overwrite"),
    ]);
  });

  it("A6 edit 规模段为行数变化，diff 进折叠区且保留换行", () => {
    const result = view({
      toolName: "edit",
      args: { path: ABS, old_string: "a\nb\n", new_string: "a\nc\nd\n" },
      intents: [{ action: "edit", resources: [ABS] }],
    });
    expect(result.scale).toEqual([
      translate("chat.tool.approval.scale.lines_transition", {
        from: 2,
        to: 3,
      }),
    ]);
    expect(result.detail.evidence?.kind).toBe("diff");
    expect(result.detail.evidence?.text).toContain("- a\n- b\n+ a\n+ c\n+ d");

    const superEdit = view({
      toolName: "super_edit",
      args: { path: ABS, startLine: 10, endLine: 12, replacement: "x\ny\n" },
      intents: [{ action: "edit", resources: [ABS] }],
    });
    expect(superEdit.scale).toEqual([
      translate("chat.tool.approval.scale.range_transition", {
        from: 10,
        to: 12,
        lines: 2,
      }),
    ]);
  });

  it("A7 shell 短命令即对象；超宽或含控制操作符则全文进行 2 命令块且不截断", () => {
    const short = view({
      toolName: "shell",
      args: { command: "git status" },
      intents: [{ action: "execute", resources: ["git status"] }],
      preview: { shell: shellPreview({ class: "readOnly" }) },
    });
    expect(short.object).toBe("git status");
    expect(short.command).toBeNull();

    const longCommand = `echo ${"a".repeat(130)}`;
    const wide = view({
      toolName: "shell",
      args: { command: longCommand },
      intents: [{ action: "execute", resources: [longCommand] }],
      preview: { shell: shellPreview({ class: "readOnly" }) },
    });
    expect(wide.command).toBe(longCommand);
    expect(wide.object).toBeNull();

    const multiline = "echo a\necho b";
    const control = view({
      toolName: "shell",
      args: { command: multiline },
      intents: [{ action: "execute", resources: [multiline] }],
      preview: { shell: shellPreview({ class: "unknown" }) },
    });
    expect(control.command).toBe(multiline);
    expect(control.command).toContain("\n");

    // 子命令（反引号 / `$(`）同属结构层控制操作符（§6.3），短命令也要进命令块。
    for (const command of ["echo `date`", "echo $(date)", "echo a\recho b"]) {
      const nested = view({
        toolName: "shell",
        args: { command },
        intents: [{ action: "execute", resources: [command] }],
        preview: { shell: shellPreview({ class: "unknown" }) },
      });
      expect(nested.command, command).toBe(command);
      expect(nested.object, command).toBeNull();
    }
  });

  it("A8 shell 语句数 / 重定向 / 边界；含控制操作符时不给「只读」结论", () => {
    const chained = view({
      toolName: "shell",
      args: { command: "rm -rf tmp && echo done" },
      intents: [{ action: "execute", resources: ["rm -rf tmp && echo done"] }],
      preview: { shell: shellPreview({ statements: 2, class: "destructive" }) },
    });
    const chainedText = chained.segments.join(" · ");
    expect(chainedText).toContain(
      translate("chat.tool.approval.scale.statements", { count: 2 }),
    );
    expect(chainedText).toContain(
      translate("chat.tool.approval.consequence.shell_destructive"),
    );

    const redirected = view({
      toolName: "shell",
      args: { command: `echo hi > ${ABS_OUT}` },
      intents: [{ action: "execute", resources: [`echo hi > ${ABS_OUT}`] }],
      preview: {
        shell: shellPreview({
          class: "destructive",
          redirects: [ABS_OUT],
          targetsScope: "inside",
        }),
      },
    });
    const redirectedText = redirected.segments.join(" · ");
    expect(redirectedText).toContain(
      translate("chat.tool.approval.scale.redirect", { target: "result.txt" }),
    );
    expect(redirectedText).not.toContain("1 条");

    const readOnlyWithControl = view({
      toolName: "shell",
      args: { command: "cat a && cat b" },
      intents: [{ action: "execute", resources: ["cat a && cat b"] }],
      preview: { shell: shellPreview({ statements: 2, class: "readOnly" }) },
    });
    expect(readOnlyWithControl.consequences).toEqual([
      translate("chat.tool.approval.consequence.shell_unknown"),
    ]);
    expect(readOnlyWithControl.segments.join("")).not.toContain("只读");

    const inside = view({
      toolName: "shell",
      args: { command: "rm tmp/x" },
      intents: [{ action: "execute", resources: ["rm tmp/x"] }],
      preview: {
        shell: shellPreview({ class: "destructive", targetsScope: "inside" }),
      },
    });
    expect(inside.boundary?.text).toBe(
      translate("chat.tool.approval.boundary.shell_inside"),
    );
    expect(inside.segments.join("")).not.toContain(
      translate("chat.tool.approval.boundary.shell_inside"),
    );

    for (const scope of ["unknown", null]) {
      const result = view({
        toolName: "shell",
        args: { command: "rm tmp/x" },
        intents: [{ action: "execute", resources: ["rm tmp/x"] }],
        preview: {
          shell: shellPreview({ class: "destructive", targetsScope: scope }),
        },
      });
      expect(result.boundary).toBeNull();
    }
  });

  it("A9 preview 缺失则无规模段；截断 / 估算措辞不进首屏", () => {
    const noPreview = deleteView({ path: ABS }, null);
    expect(noPreview.scale).toEqual([]);

    const degraded = deleteView(
      { path: ABS, recursive: true },
      {
        summary: "",
        truncated: true,
        estimated: true,
        entries: 5,
        bytes: 1024,
      },
    );
    const text = degraded.segments.join(" · ");
    expect(degraded.detail.diagnostics.previewSummary).toBeNull();
    expect(text).not.toContain(
      translate("chat.tool.approval.diagnostic.preview_truncated"),
    );
    expect(text).not.toContain(
      translate("chat.tool.approval.diagnostic.preview_estimated"),
    );
    expect(text).not.toContain("≥");
  });

  it("A9b 目标为链接：后果「只删链接，不碰目标」且无规模段", () => {
    const result = deleteView(
      { path: ABS },
      {
        entries: 1,
        bytes: null,
        targetKind: "link",
        exists: true,
        symlink: true,
      },
    );
    expect(result.scale).toEqual([]);
    expect(result.consequences.join("")).toContain("只删链接");
  });

  it("A10 未知工具只回显标识与参数摘要，不抛异常、不猜字段", () => {
    const result = view({ toolName: "frobnicate", args: { foo: "bar" } });
    expect(result.verb).toBe(translate("chat.tool.approval.verb.call"));
    expect(result.segments).toHaveLength(2);
    expect(result.segments[1]).toContain("frobnicate");
    expect(result.segments[1]).toContain("bar");
    expect(result.consequences).toEqual([]);
    expect(result.scale).toEqual([]);
  });

  it("A10b 要素缺失不留空槽与多余分隔符", () => {
    const read = view({
      toolName: "read",
      args: { path: ABS },
      intents: [{ action: "read", resources: [ABS] }],
      preview: null,
    });
    expect(read.segments).toHaveLength(2);
    expect(read.verb).toBe(translate("chat.tool.approval.intent.action.read"));
    expect(read.object).toBe("report.md");

    const remove = deleteView(
      { path: ABS, permanent: false },
      { bytes: 3072, targetKind: "file", exists: true },
    );
    expect(remove.segments).toHaveLength(4);

    for (const result of [read, remove]) {
      expect(result.segments.join(" · ")).not.toContain("· ·");
      expect(result.segments.every((segment) => segment !== "")).toBe(true);
    }
  });

  it("A10c source + target 只在无判别参数时为「移动」，判别取值未知时不产出动词", () => {
    const move = view({
      toolName: "move",
      args: { source: ABS, target: ABS_OUT },
      intents: [{ action: "edit", resources: [ABS] }],
    });
    expect(move.verb).toBe(translate("chat.tool.approval.verb.move"));

    const ambiguous = view({
      toolName: "future_tool",
      args: { source: ABS, target: ABS_OUT, permanent: "unknown" },
      intents: [{ action: "edit", resources: [ABS] }],
    });
    expect(ambiguous.verb).toBeNull();
    expect(ambiguous.segments.join("")).not.toContain(
      translate("chat.tool.approval.verb.move"),
    );
  });

  it("A10d 不可证事实不出现：无「自动备份」「无备份」", () => {
    const samples = [
      deleteView(
        { path: ABS, permanent: true },
        { bytes: 1024, targetKind: "file" },
      ),
      view({
        toolName: "shell",
        args: { command: "rm tmp/x" },
        intents: [{ action: "execute", resources: ["rm tmp/x"] }],
        preview: { shell: shellPreview({ class: "destructive" }) },
      }),
      view({
        toolName: "write",
        args: { path: ABS, content: "a\n", append: false },
        intents: [{ action: "edit", resources: [ABS] }],
        preview: { exists: true },
      }),
    ];
    for (const result of samples) {
      const text = result.consequences.join(" · ");
      expect(text).not.toContain("自动备份");
      expect(text).not.toContain("无备份");
    }
  });

  it("A10e 有效性措辞不进卡片：无「会失败」「可能失败」", () => {
    const samples = [
      deleteView({ path: ABS }, { bytes: 1024, targetKind: "file" }),
      view({
        toolName: "move",
        args: { source: ABS, target: ABS_OUT, overwrite: false },
        intents: [{ action: "edit", resources: [ABS] }],
      }),
    ];
    for (const result of samples) {
      const text = result.consequences.join(" · ");
      expect(text).not.toContain("会失败");
      expect(text).not.toContain("可能失败");
    }
  });

  it("A10f MCP 桥接工具：动词「调用外部工具」，对象不含 hash 末段", () => {
    const result = view({
      toolName: "mcp__github__search",
      args: { q: "issue" },
      intents: [{ action: "mcp", resources: ["mcp:github:abc123"] }],
    });
    expect(result.verb).toBe(translate("chat.tool.approval.intent.action.mcp"));
    expect(result.object).toContain("mcp__github__search");
    expect(result.object).toContain("issue");
    expect(result.segments.join("")).not.toContain("mcp:github");
    expect(result.consequences).toEqual([]);
    expect(result.detail.intents[0].raw).toEqual(["mcp:github:abc123"]);
  });
});

describe("审批视图模型：折叠区与字典（I72 A12–A13f）", () => {
  it("A12 多原因取优先级最高者为主句，其余全部进 rest", () => {
    const result = view({
      toolName: "delete",
      args: { path: ABS },
      intents: [{ action: "delete", resources: [ABS] }],
      reasons: [
        { kind: "defaultPolicy", rule: "* *" },
        { kind: "userRule", action: "delete", resource: ABS },
        { kind: "tierCeiling", tier: "os" },
      ],
    });
    expect(result.detail.reasons.primary?.kind).toBe("userRule");
    expect(result.detail.reasons.rest.map((reason) => reason.kind)).toEqual([
      "tierCeiling",
      "defaultPolicy",
    ]);
  });

  it("A13 destructive / openWorld 对恒定工具不产生首屏文案", () => {
    const destructiveText = translate("chat.tool.approval.destructive");
    const openWorldText = translate("chat.tool.approval.open_world");

    const remove = view({
      toolName: "delete",
      args: { path: ABS, permanent: true },
      intents: [{ action: "delete", resources: [ABS] }],
      preview: { bytes: 1024, targetKind: "file" },
      behavior: { destructive: true },
    });
    expect(remove.segments.join("")).not.toContain(destructiveText);

    const shell = view({
      toolName: "shell",
      args: { command: "rm tmp/x" },
      intents: [{ action: "execute", resources: ["rm tmp/x"] }],
      preview: { shell: shellPreview({ class: "destructive" }) },
      behavior: { destructive: true, openWorld: true },
    });
    const shellText = shell.segments.join("");
    expect(shellText).not.toContain(destructiveText);
    expect(shellText).not.toContain(openWorldText);

    const superEdit = view({
      toolName: "super_edit",
      args: { path: ABS, startLine: 1, endLine: 2, replacement: "a\n" },
      intents: [{ action: "edit", resources: [ABS] }],
      behavior: { destructive: true },
    });
    expect(superEdit.segments.join("")).not.toContain(destructiveText);
  });

  it("A13b TOOL_PROFILES 键集合 = Rust TOOL_ID 扣除 TOOLBOX_ONLY_IDS", () => {
    expect(new Set(TOOLBOX_ONLY_IDS)).toEqual(new Set(["fs_link"]));
    const agentTools = new Set(
      rustToolIds().filter((id) => !TOOLBOX_ONLY_IDS.has(id)),
    );
    expect(agentTools.size).toBe(14);
    expect(new Set(Object.keys(TOOL_PROFILES))).toEqual(agentTools);
  });

  it("A13c extensions 用 kind.code 字典文案，未知组合折叠区保留原文", () => {
    const known = view({
      toolName: "shell",
      args: { command: "rm tmp/x" },
      intents: [{ action: "execute", resources: ["rm tmp/x"] }],
      preview: { shell: shellPreview({ class: "destructive" }) },
      extensions: [
        { kind: "sandbox.enforcement", code: "partial", severity: "caution" },
      ],
    });
    expect(known.detail.diagnostics.extensions[0].label).toBe(
      translate("chat.tool.approval.extension.sandbox.enforcement.partial"),
    );
    expect(known.detail.diagnostics.extensions[0].label).not.toMatch(
      /sandbox|enforcement/i,
    );

    const unknown = view({
      toolName: "read",
      args: { path: ABS },
      intents: [{ action: "read", resources: [ABS] }],
      extensions: [{ kind: "future", code: "weird", severity: "danger" }],
    });
    expect(unknown.detail.diagnostics.extensions[0].label).toBe("future.weird");
  });

  it("A13d intents 首屏为 action 人话 + 路径末段，折叠区保留原文", () => {
    const result = view({
      toolName: "delete",
      args: { path: ABS, permanent: true },
      intents: [
        { action: "delete", resources: [ABS] },
        { action: "purge", resources: [ABS] },
      ],
      preview: { bytes: 1024, targetKind: "file", exists: true },
    });
    expect(result.detail.intents.map((intent) => intent.label)).toEqual([
      translate("chat.tool.approval.intent.action.delete"),
      translate("chat.tool.approval.intent.action.purge"),
    ]);
    for (const intent of result.detail.intents) {
      expect(intent.head).toBe("report.md");
      expect(intent.raw).toEqual([ABS]);
    }
    const text = result.segments.join(" · ");
    expect(text).not.toContain("purge");
    expect(text).not.toContain(ABS);
  });

  it("A13e 路径显示单点：verbatim 前缀、双分隔符、尾分隔符归一为末段", () => {
    const messy = "\\\\?\\G:\\work\\\\docs\\a.md\\\\";
    const result = view({
      toolName: "read",
      args: { path: messy },
      intents: [{ action: "read", resources: [messy] }],
      preview: null,
    });
    expect(result.object).toBe("a.md");
    expect(result.segments.join("")).not.toContain("\\\\");
    expect(result.segments.join("")).not.toContain("G:");
  });

  it("A13f mkdir 动词取 preview.verbCode，不回退成「修改」", () => {
    const result = view({
      toolName: "mkdir",
      args: { path: "G:\\work\\newdir" },
      intents: [{ action: "edit", resources: ["G:\\work\\newdir"] }],
      preview: { verbCode: "mkdir", exists: false },
    });
    expect(result.verb).toBe(translate("chat.tool.approval.verb.mkdir"));
    expect(result.verb).not.toBe(
      translate("chat.tool.approval.intent.action.edit"),
    );
    expect(result.object).toBe("newdir");
  });

  it("A10g 未知工具不借数据参数冒充已知：仍只回显标识（§5.4）", () => {
    for (const args of [
      { path: ABS },
      { content: "hello" },
      { command: "rm -rf build" },
      { source: ABS, target: ABS_OUT },
    ]) {
      const result = view({ toolName: "mystery", args });
      expect(result.verb, JSON.stringify(args)).toBe(
        translate("chat.tool.approval.verb.call"),
      );
      expect(result.scale, JSON.stringify(args)).toEqual([]);
      expect(result.consequences, JSON.stringify(args)).toEqual([]);
      expect(result.segments, JSON.stringify(args)).toHaveLength(2);
      expect(result.segments[1], JSON.stringify(args)).toContain("mystery");
    }
  });

  it("A13g 授权范围串单点拼接：二次确认复用 scope.alwaysIntents（§8.2）", () => {
    const result = view({
      toolName: "delete",
      args: { path: ABS, permanent: true },
      intents: [
        { action: "delete", resources: [ABS] },
        { action: "purge", resources: [ABS] },
      ],
      preview: { bytes: 1024, targetKind: "file", exists: true },
    });
    const separator = translate("chat.tool.approval.scope.intent_separator");
    const expected = [
      `${translate("chat.tool.approval.intent.action.delete")} report.md`,
      `${translate("chat.tool.approval.intent.action.purge")} report.md`,
    ].join(separator);
    expect(result.scope.alwaysIntents).toBe(expected);
    expect(result.scope.always).toContain(result.scope.alwaysIntents);
  });
});
