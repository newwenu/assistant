import { describe, it, expect } from "vitest";
import {
  APPROVAL_RULES_MAX,
  APPROVAL_RULE_ACTION_MAX,
  APPROVAL_RULE_RESOURCE_MAX,
  APPROVAL_RULE_SHORTCUTS,
  DEFAULT_TOOL_CONFIG,
  areApprovalRulesEqual,
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
  validateApprovalRules,
} from "../toolConfig";
import type {
  ApprovalRuleDto,
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
    approvalRules: [],
    ...partial,
  };
}

function rule(
  action: string,
  resource: string,
  effect: ApprovalRuleDto["effect"],
): ApprovalRuleDto {
  return { action, resource, effect };
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

describe("审批规则在有效配置与差异里的语义（I70 §5.4 / A10）", () => {
  it("有效配置保留规则，且会话级优先于模式级", () => {
    const resolved = resolveEffectiveToolConfig(
      config({ approvalRules: [rule("read", "docs/**", "allow")] }),
      config({ approvalRules: [rule("*", "*", "ask")] }),
    );
    expect(resolved.approvalRules).toEqual([rule("read", "docs/**", "allow")]);
  });

  it("会话级缺失时回退模式级规则", () => {
    expect(
      resolveEffectiveToolConfig(
        null,
        config({ approvalRules: [rule("execute", "git status", "allow")] }),
      ).approvalRules,
    ).toEqual([rule("execute", "git status", "allow")]);
  });

  it("两级都缺失时规则为空数组（键恒在）", () => {
    expect(resolveEffectiveToolConfig(null, undefined).approvalRules).toEqual(
      [],
    );
  });

  it("返回副本：改动解析结果不污染来源配置", () => {
    const source = config({ approvalRules: [rule("read", "*", "allow")] });
    const resolved = resolveEffectiveToolConfig(source, null);
    resolved.approvalRules.push(rule("edit", "*", "deny"));
    expect(source.approvalRules).toEqual([rule("read", "*", "allow")]);
  });

  it("规则未变时 rulesChanged 为 false", () => {
    const rules = [rule("read", "*", "allow"), rule("execute", "*", "ask")];
    expect(
      diffToolConfig(
        config({ approvalRules: rules }),
        config({ approvalRules: [...rules] }),
      ).rulesChanged,
    ).toBe(false);
  });

  it("条数相同但内容不同也算变更（改一行 effect 同样改变行为）", () => {
    expect(
      diffToolConfig(
        config({ approvalRules: [rule("read", "*", "allow")] }),
        config({ approvalRules: [rule("read", "*", "ask")] }),
      ).rulesChanged,
    ).toBe(true);
  });

  it("顺序变化算变更：顺序即生效顺序，不能用集合相等代替", () => {
    const first = rule("read", "*", "allow");
    const second = rule("read", "secret/**", "deny");
    expect(
      diffToolConfig(
        config({ approvalRules: [first, second] }),
        config({ approvalRules: [second, first] }),
      ).rulesChanged,
    ).toBe(true);
  });

  it("只改规则时差异不为空", () => {
    const diff = diffToolConfig(
      config(),
      config({ approvalRules: [rule("*", "*", "deny")] }),
    );
    expect(diff.rulesChanged).toBe(true);
    expect(isEmptyToolConfigDiff(diff)).toBe(false);
  });

  it("规则等价比较按三元组逐项、含顺序", () => {
    expect(
      areApprovalRulesEqual(
        [rule("read", "*", "allow")],
        [rule("read", "*", "allow")],
      ),
    ).toBe(true);
    expect(
      areApprovalRulesEqual(
        [rule("read", "*", "allow")],
        [rule("read", "**", "allow")],
      ),
    ).toBe(false);
    expect(areApprovalRulesEqual([], [])).toBe(true);
  });
});

describe("规则提交前预校验（I70 §6.1 / A5）", () => {
  it("合法规则通过", () => {
    expect(
      validateApprovalRules([
        rule("read", "docs/**", "allow"),
        rule("execute", "*", "ask"),
        rule("edit", ".git/**", "deny"),
      ]),
    ).toBeNull();
  });

  it("空规则集合法", () => {
    expect(validateApprovalRules([])).toBeNull();
  });

  it("allow 不得匹配 readToolOutput / mcp，也不得用通配符绕过底线", () => {
    for (const action of ["readToolOutput", "mcp", "*", "**", "read*"]) {
      const error = validateApprovalRules([rule(action, "*", "allow")]);
      expect(error?.code, `action=${action}`).toBe("allowBottomLine");
    }
  });

  it("同样动作设为 ask / deny 时允许（只禁止放宽，不禁止收紧）", () => {
    expect(
      validateApprovalRules([
        rule("readToolOutput", "*", "ask"),
        rule("mcp", "*", "deny"),
      ]),
    ).toBeNull();
  });

  it("条数超过上限时整体报错，行号为 null", () => {
    const rules = Array.from({ length: APPROVAL_RULES_MAX + 1 }, () =>
      rule("read", "*", "ask"),
    );
    const error = validateApprovalRules(rules);
    expect(error?.code).toBe("tooMany");
    expect(error?.index).toBeNull();
  });

  it("动作长度 1–32，超出或为空都报错并给出行号", () => {
    expect(validateApprovalRules([rule("", "*", "ask")])?.code).toBe(
      "actionLength",
    );
    const error = validateApprovalRules([
      rule("*", "*", "ask"),
      rule("a".repeat(APPROVAL_RULE_ACTION_MAX + 1), "*", "ask"),
    ]);
    expect(error?.code).toBe("actionLength");
    expect(error?.index).toBe(2);
  });

  it("资源长度 1–256，超出或为空都报错并给出行号", () => {
    expect(validateApprovalRules([rule("read", "", "ask")])?.code).toBe(
      "resourceLength",
    );
    const error = validateApprovalRules([
      rule("read", "a".repeat(APPROVAL_RULE_RESOURCE_MAX + 1), "ask"),
    ]);
    expect(error?.code).toBe("resourceLength");
    expect(error?.index).toBe(1);
  });

  it("资源不得以 tier: 开头（层级上限只能由上方的层级设置承担）", () => {
    const error = validateApprovalRules([
      rule("read", "*", "ask"),
      rule("read", "tier:os", "allow"),
    ]);
    expect(error?.code).toBe("reservedTier");
    expect(error?.index).toBe(2);
  });

  it("长度按字符计数，不按 UTF-16 码元", () => {
    const emoji = "😀".repeat(APPROVAL_RULE_ACTION_MAX);
    expect(validateApprovalRules([rule(emoji, "*", "ask")])).toBeNull();
    const tooLong = "😀".repeat(APPROVAL_RULE_ACTION_MAX + 1);
    expect(validateApprovalRules([rule(tooLong, "*", "ask")])?.code).toBe(
      "actionLength",
    );
  });
});

describe("快捷规则的数据定义（I70 §3.3）", () => {
  it("五条快捷规则齐备，动作与规格一致", () => {
    expect(APPROVAL_RULE_SHORTCUTS.map((item) => item.key)).toEqual([
      "execute",
      "edit",
      "read",
      "mcp",
      "readOutput",
    ]);
    expect(APPROVAL_RULE_SHORTCUTS.map((item) => item.action)).toEqual([
      "execute",
      "edit",
      "read",
      "mcp",
      "readToolOutput",
    ]);
  });

  it("mcp 与 readToolOutput 不提供 allow（底线在 UI 上可见）", () => {
    for (const key of ["mcp", "readOutput"] as const) {
      const shortcut = APPROVAL_RULE_SHORTCUTS.find((item) => item.key === key);
      expect(shortcut?.effects).toEqual(["ask", "deny"]);
    }
  });

  it("每条快捷规则追加出来的规则都能通过预校验", () => {
    for (const shortcut of APPROVAL_RULE_SHORTCUTS) {
      for (const effect of shortcut.effects) {
        expect(
          validateApprovalRules([rule(shortcut.action, "*", effect)]),
          `${shortcut.key}/${effect}`,
        ).toBeNull();
      }
    }
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
