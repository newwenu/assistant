import { describe, it, expect } from "vitest";
import {
  DEFAULT_TOOL_CONFIG,
  countEnabledSelectableTools,
  countSelectableTools,
  diffToolConfig,
  groupToolsByTier,
  groupToolUnits,
  isEmptyToolConfigDiff,
  isCompanionTool,
  isMetaTool,
  isSelectableTool,
  resolveEffectiveToolConfig,
  selectedIdsInTier,
  stripCompanionIds,
} from "../toolConfig";
import type {
  ConversationToolConfigDto,
  ToolDescriptorDto,
} from "../../../types/protocol.contract";

function config(
  partial: Partial<ConversationToolConfigDto> = {},
): ConversationToolConfigDto {
  return {
    maxToolTier: "sandbox",
    approvalLevel: "dangerous_only",
    enabledTools: [],
    disabledTools: [],
    ...partial,
  };
}

function descriptor(
  id: string,
  partial: Partial<ToolDescriptorDto> = {},
): ToolDescriptorDto {
  return {
    id,
    name: id,
    description: `${id} description`,
    tier: "os",
    companionOf: null,
    ...partial,
  };
}

describe("resolveEffectiveToolConfig", () => {
  it("会话级优先于模式级", () => {
    const resolved = resolveEffectiveToolConfig(
      config({ enabledTools: ["shell"] }),
      config({ enabledTools: ["read"] }),
    );
    expect(resolved.enabledTools).toEqual(["shell"]);
  });

  it("会话级缺失时回退模式级", () => {
    expect(
      resolveEffectiveToolConfig(null, config({ enabledTools: ["read"] }))
        .enabledTools,
    ).toEqual(["read"]);
  });

  it("两级都缺失时回退缺省值", () => {
    expect(resolveEffectiveToolConfig(null, undefined)).toEqual(
      DEFAULT_TOOL_CONFIG,
    );
  });

  it("返回副本，调用方改动不污染来源", () => {
    const source = config({ enabledTools: ["read"] });
    const resolved = resolveEffectiveToolConfig(source, null);
    resolved.enabledTools.push("shell");
    expect(source.enabledTools).toEqual(["read"]);
  });
});

describe("diffToolConfig", () => {
  it("只报告新增与移除的工具", () => {
    const diff = diffToolConfig(
      config({ enabledTools: ["read", "shell"] }),
      config({ enabledTools: ["read", "write"] }),
    );
    expect(diff.addedTools).toEqual(["write"]);
    expect(diff.removedTools).toEqual(["shell"]);
    expect(diff.tierChanged).toBeNull();
    expect(diff.levelChanged).toBeNull();
  });

  it("层级上限与审批级别变更时给出新值", () => {
    const diff = diffToolConfig(
      config({ approvalLevel: "none" }),
      config({ maxToolTier: "os", approvalLevel: "all_os_tools" }),
    );
    expect(diff.tierChanged).toBe("os");
    expect(diff.levelChanged).toBe("all_os_tools");
  });

  it("无变化时差异为空", () => {
    expect(isEmptyToolConfigDiff(diffToolConfig(config(), config()))).toBe(
      true,
    );
  });
});

describe("伴随与元工具的面板语义（I58 §3.6）", () => {
  it("分类：只有伴随项不是可勾选条目，元工具照常可勾选", () => {
    expect(
      isCompanionTool(descriptor("super_read", { companionOf: "read" })),
    ).toBe(true);
    expect(isMetaTool(descriptor("request_tool"))).toBe(true);
    expect(isSelectableTool(descriptor("request_tool"))).toBe(true);
    expect(
      isSelectableTool(descriptor("super_read", { companionOf: "read" })),
    ).toBe(false);
    expect(isSelectableTool(descriptor("read"))).toBe(true);
  });

  it("分组：伴随条目紧跟基工具之后，不新建分组", () => {
    const grouped = groupToolsByTier([
      descriptor("read"),
      descriptor("edit"),
      descriptor("super_read", { companionOf: "read" }),
    ]);
    expect(grouped.get("os")?.map((tool) => tool.id)).toEqual([
      "read",
      "super_read",
      "edit",
    ]);
  });

  it("渲染单元：伴随条目并入基工具单元，横向流里不会被换行拆开", () => {
    const units = groupToolUnits([
      descriptor("read"),
      descriptor("super_read", { companionOf: "read" }),
      descriptor("edit"),
      descriptor("super_edit", { companionOf: "edit" }),
    ]);
    expect(units.map((unit) => unit.head.id)).toEqual(["read", "edit"]);
    expect(units[0].companions.map((tool) => tool.id)).toEqual(["super_read"]);
    expect(units[1].companions.map((tool) => tool.id)).toEqual(["super_edit"]);
  });

  it("渲染单元：基工具不在本 tier 的伴随条目自成一个单元", () => {
    const units = groupToolUnits([
      descriptor("super_read", { companionOf: "read" }),
      descriptor("edit"),
    ]);
    expect(units.map((unit) => unit.head.id)).toEqual(["super_read", "edit"]);
    expect(units[0].companions).toEqual([]);
  });

  it("分组：元工具排在组内末尾，与普通能力区分开", () => {
    const grouped = groupToolsByTier([
      descriptor("request_tool"),
      descriptor("read"),
      descriptor("write"),
    ]);
    expect(grouped.get("os")?.map((tool) => tool.id)).toEqual([
      "read",
      "write",
      "request_tool",
    ]);
  });

  it("分组：基工具不在本 tier 时伴随条目并列，不跨 tier 移动", () => {
    const grouped = groupToolsByTier([
      descriptor("read", { tier: "sandbox" }),
      descriptor("super_read", { companionOf: "read", tier: "os" }),
    ]);
    expect(grouped.get("os")?.map((tool) => tool.id)).toEqual(["super_read"]);
    expect(grouped.get("sandbox")?.map((tool) => tool.id)).toEqual(["read"]);
  });

  it("勾选集合：历史残留的伴随 id 不作为已勾选项出现", () => {
    const tools = [
      descriptor("read"),
      descriptor("super_read", { companionOf: "read" }),
    ];
    expect(
      selectedIdsInTier(
        config({ enabledTools: ["read", "super_read"] }),
        tools,
        "os",
      ),
    ).toEqual(["read"]);
  });

  it("计数：伴随项不计入，元工具计入", () => {
    const tools = [
      descriptor("read"),
      descriptor("super_read", { companionOf: "read" }),
      descriptor("request_tool"),
    ];
    expect(
      countEnabledSelectableTools(
        config({ enabledTools: ["read", "super_read", "request_tool"] }),
        tools,
      ),
    ).toBe(2);
  });

  it("计数分母：只数可勾选条目，伴随项不撑大总数", () => {
    const tools = [
      descriptor("read"),
      descriptor("super_read", { companionOf: "read" }),
      descriptor("edit"),
      descriptor("super_edit", { companionOf: "edit" }),
      descriptor("request_tool"),
    ];
    expect(countSelectableTools(tools)).toBe(3);
  });

  it("计数分子：配置里的未知 id 不计入已启用数", () => {
    expect(
      countEnabledSelectableTools(
        config({ enabledTools: ["read", "ghost_tool"] }),
        [descriptor("read")],
      ),
    ).toBe(1);
  });

  it("剔除：名单里的伴随 id 被移除，普通 id 与未知 id 保留", () => {
    const tools = [
      descriptor("read"),
      descriptor("super_read", { companionOf: "read" }),
    ];
    const next = stripCompanionIds(
      config({ enabledTools: ["read", "super_read", "ghost_tool"] }),
      tools,
    );
    expect(next.enabledTools).toEqual(["read", "ghost_tool"]);
  });

  it("剔除：无残留时返回同一对象，避免无谓重渲染", () => {
    const base = config({ enabledTools: ["read"] });
    expect(stripCompanionIds(base, [descriptor("read")])).toBe(base);
  });
});
