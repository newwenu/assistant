import type {
  ApprovalLevelDto,
  ApprovalRuleDto,
  ApprovalRuleEffectDto,
  ConversationToolConfigDto,
  ToolDescriptorDto,
  ToolTierDto,
} from "../../types/protocol.contract";

/** 工具层级的完整取值集合（顺序即 UI 展示顺序）。 */
export const TOOL_TIERS: ToolTierDto[] = ["sandbox", "os", "external"];

/** 层级高低权重：数值越大越靠外，用于与 `maxToolTier` 比较。 */
export const TIER_RANK: Record<ToolTierDto, number> = {
  sandbox: 0,
  os: 1,
  external: 2,
};

/** 审批级别的完整取值集合（顺序即 UI 展示顺序，按由宽到严排列）。 */
export const APPROVAL_LEVELS: ApprovalLevelDto[] = [
  "none",
  "workspace_write",
  "dangerous_only",
  "all_os_tools",
];

/**
 * 前端「未设置」兜底：与 Rust `ConversationToolConfigDto::default()` 同义。
 *
 * 权威来源是 Rust 协议层；前端只在后端值缺失（模式未找到）时使用，不得作为新值来源。
 */
export const DEFAULT_TOOL_CONFIG: ConversationToolConfigDto = {
  maxToolTier: "sandbox",
  approvalLevel: "none",
  enabledTools: [],
  disabledTools: [],
  approvalRules: [],
};

/**
 * 有效工具配置的唯一解析实现（I38d §4.4）：
 * 会话级持久值 ?? 模式级初始值 ?? 缺省。
 */
export function resolveEffectiveToolConfig(
  conversationToolConfig: ConversationToolConfigDto | null | undefined,
  modeToolConfig: ConversationToolConfigDto | null | undefined,
): ConversationToolConfigDto {
  const source = conversationToolConfig ?? modeToolConfig;
  if (!source) return DEFAULT_TOOL_CONFIG;
  return {
    maxToolTier: source.maxToolTier,
    approvalLevel: source.approvalLevel,
    enabledTools: [...source.enabledTools],
    disabledTools: [...source.disabledTools],
    approvalRules: [...source.approvalRules],
  };
}

/** 两次装配之间的差异，供变更提示文案使用（I38d §4.7）。 */
export interface ToolConfigDiff {
  addedTools: string[];
  removedTools: string[];
  /** 变更后的层级上限；未变更时为 `null`。 */
  tierChanged: ToolTierDto | null;
  /** 变更后的审批级别；未变更时为 `null`。 */
  levelChanged: ApprovalLevelDto | null;
  /** 审批规则是否变化（按三元组序列逐项相等比较，顺序即生效顺序）。 */
  rulesChanged: boolean;
}

/** 比较两次有效配置；只报告事实，文案由调用方组装。 */
export function diffToolConfig(
  prev: ConversationToolConfigDto,
  next: ConversationToolConfigDto,
): ToolConfigDiff {
  const prevIds = new Set(prev.enabledTools);
  const nextIds = new Set(next.enabledTools);
  return {
    addedTools: next.enabledTools.filter((id) => !prevIds.has(id)),
    removedTools: prev.enabledTools.filter((id) => !nextIds.has(id)),
    tierChanged:
      prev.maxToolTier === next.maxToolTier ? null : next.maxToolTier,
    levelChanged:
      prev.approvalLevel === next.approvalLevel ? null : next.approvalLevel,
    rulesChanged: !areApprovalRulesEqual(
      prev.approvalRules,
      next.approvalRules,
    ),
  };
}

/**
 * 规则集合是否等价：条数、顺序、三元组逐项相等。
 *
 * 顺序即生效顺序（last-match-wins），因此不能用「集合相等」代替；
 * 也不能只比条数——改一行 `effect` 同样改变行为。
 */
export function areApprovalRulesEqual(
  prev: ApprovalRuleDto[],
  next: ApprovalRuleDto[],
): boolean {
  if (prev.length !== next.length) return false;
  return prev.every((rule, index) => {
    const other = next[index];
    return (
      rule.action === other.action &&
      rule.resource === other.resource &&
      rule.effect === other.effect
    );
  });
}

/** 差异是否为空（无任何可用性变化）。 */
export function isEmptyToolConfigDiff(diff: ToolConfigDiff): boolean {
  return (
    diff.addedTools.length === 0 &&
    diff.removedTools.length === 0 &&
    diff.tierChanged === null &&
    diff.levelChanged === null &&
    !diff.rulesChanged
  );
}

/** 元工具：能力发现与升级申请的唯一入口，和普通工具一样需要用户勾选（I58 §3.6）。 */
const META_TOOL_IDS: ReadonlySet<string> = new Set(["request_tool"]);

/** 伴随工具：没有独立身份，跟随基工具获得准入，面板不产生独立开关。 */
export const isCompanionTool = (tool: ToolDescriptorDto): boolean =>
  tool.companionOf !== null;

/** 元工具：可勾选，但面板需要额外标注它是「申请入口」而非普通能力。 */
export const isMetaTool = (tool: ToolDescriptorDto): boolean =>
  META_TOOL_IDS.has(tool.id);

/** 面板可勾选条目：只有伴随项不写 `enabledTools`，元工具与普通工具一致。 */
export const isSelectableTool = (tool: ToolDescriptorDto): boolean =>
  !isCompanionTool(tool);

/**
 * 剔除两个名单里的伴随 id（I58 §3.6）。
 *
 * 伴随工具没有独立身份：Rust 侧 `exclude_companions` 一律排除这些 id，
 * 名单里留着它们只是历史残留（旧面板曾把伴随项渲染成勾选项）。
 * 面板与设置页在每次写回前调用本函数，使残留项在用户下一次保存时被静默修掉，
 * 不需要迁移旧配置、也不为脏数据定义额外提示。
 */
export function stripCompanionIds(
  config: ConversationToolConfigDto,
  tools: ToolDescriptorDto[],
): ConversationToolConfigDto {
  const companionIds = new Set(tools.filter(isCompanionTool).map((t) => t.id));
  if (companionIds.size === 0) return config;
  const enabledTools = config.enabledTools.filter(
    (id) => !companionIds.has(id),
  );
  const disabledTools = config.disabledTools.filter(
    (id) => !companionIds.has(id),
  );
  if (
    enabledTools.length === config.enabledTools.length &&
    disabledTools.length === config.disabledTools.length
  ) {
    return config;
  }
  return { ...config, enabledTools, disabledTools };
}

/**
 * 按 tier 分组；顺序与 `TOOL_TIERS` 一致，无工具的 tier 也保留空数组。
 *
 * 组内顺序：普通工具 → 元工具（排末尾，与「能力」区分开）→ 伴随条目紧跟在基工具之后。
 * 伴随条目若基工具不在本 tier，退回并列（I58 §3.6）。只调整组内顺序，
 * 不新建分组、不跨 tier 移动。
 */
export function groupToolsByTier(
  tools: ToolDescriptorDto[],
): Map<ToolTierDto, ToolDescriptorDto[]> {
  const map = new Map<ToolTierDto, ToolDescriptorDto[]>();
  for (const tier of TOOL_TIERS) map.set(tier, []);
  for (const tool of tools) {
    if (isCompanionTool(tool) || isMetaTool(tool)) continue;
    map.get(tool.tier)?.push(tool);
  }
  for (const tool of tools) {
    if (!isMetaTool(tool)) continue;
    map.get(tool.tier)?.push(tool);
  }
  for (const tool of tools) {
    if (!isCompanionTool(tool)) continue;
    const bucket = map.get(tool.tier);
    if (!bucket) continue;
    let insertAt = bucket.length;
    for (let index = bucket.length - 1; index >= 0; index -= 1) {
      const item = bucket[index];
      if (
        item.id === tool.companionOf ||
        item.companionOf === tool.companionOf
      ) {
        insertAt = index + 1;
        break;
      }
    }
    bucket.splice(insertAt, 0, tool);
  }
  return map;
}

/** 面板渲染单元：一个条目 + 跟随它的伴随条目（I58 §3.6）。 */
export interface ToolUnit {
  /** 单元首位条目：普通工具、元工具，或基工具不在本 tier 而并列的伴随条目。 */
  head: ToolDescriptorDto;
  /** 与 `head` 同处一个不可断行单元的伴随条目。 */
  companions: ToolDescriptorDto[];
}

/**
 * 把已排好序的条目折成渲染单元：伴随条目并入其基工具所在单元。
 *
 * 面板是 `flex-wrap` 的横向流，扁平列表会在基工具与伴随条目之间断行，
 * 让伴随条目看起来像一个无关的独立工具；折成单元后两者始终同处一行。
 * 基工具不在本 tier 的伴随条目自成一个单元（`groupToolsByTier` 的并列规则）。
 */
export function groupToolUnits(items: ToolDescriptorDto[]): ToolUnit[] {
  const units: ToolUnit[] = [];
  for (const item of items) {
    const baseId = item.companionOf;
    if (baseId === null) {
      units.push({ head: item, companions: [] });
      continue;
    }
    const owner = units.find((unit) => unit.head.id === baseId);
    if (owner) owner.companions.push(item);
    else units.push({ head: item, companions: [] });
  }
  return units;
}

/** 该 tier 中已被勾选的工具 id，顺序与传入清单一致。 */
export function selectedIdsInTier(
  config: ConversationToolConfigDto,
  tools: ToolDescriptorDto[],
  tier: ToolTierDto,
): string[] {
  const inTier = tools.filter(
    (tool) => tool.tier === tier && isSelectableTool(tool),
  );
  return config.enabledTools.filter((id) => inTier.some((t) => t.id === id));
}

/** 已启用计数：只计可勾选条目——伴随项不计（它没有独立开关），元工具计入（它就是一个勾选项）。 */
export function countEnabledSelectableTools(
  config: ConversationToolConfigDto,
  tools: ToolDescriptorDto[],
): number {
  return tools.filter(
    (tool) => isSelectableTool(tool) && config.enabledTools.includes(tool.id),
  ).length;
}

/**
 * 可勾选条目总数：分母只数「用户真的能开关的条目」。
 *
 * 伴随项永远拿不到独立开关，把它算进分母会让「已启用 / 全部」看起来永远差几个，
 * 而配置里可能残留的未知 id 也不该出现在任何一侧（I58 §3.6）。
 */
export function countSelectableTools(tools: ToolDescriptorDto[]): number {
  return tools.filter(isSelectableTool).length;
}

/** 整体替换某个 tier 的勾选集合，保留其它 tier 的选择。 */
export function setTierSelection(
  config: ConversationToolConfigDto,
  tools: ToolDescriptorDto[],
  tier: ToolTierDto,
  ids: string[],
): ConversationToolConfigDto {
  const others = config.enabledTools.filter(
    (id) => !tools.some((tool) => tool.id === id && tool.tier === tier),
  );
  return { ...config, enabledTools: [...others, ...ids] };
}

/** 规则条数上限，与 Rust `APPROVAL_RULES_MAX` 同值（I70 §6.1 E1）。 */
export const APPROVAL_RULES_MAX = 64;

/** `action` 长度上限（E2）。 */
export const APPROVAL_RULE_ACTION_MAX = 32;

/** `resource` 长度上限（E3）。 */
export const APPROVAL_RULE_RESOURCE_MAX = 256;

/**
 * 动作词汇的生产集（I70 §3.4）：`action` 本身是开放字符串，
 * 未知动作不匹配任何意图（等于无效果，天然 fail-safe），因此下拉只做提示。
 *
 * `delete` 与 `purge` 是两个动作：`delete` 规则不覆盖永久删除。
 */
export const APPROVAL_ACTION_OPTIONS: string[] = [
  "read",
  "edit",
  "delete",
  "purge",
  "execute",
  "mcp",
  "readToolOutput",
  "readScratch",
];

/**
 * 快捷规则（I70 §3.3）：一键追加一条标准用户规则，**不维护开关状态**。
 *
 * `mcp`、`readToolOutput` 与 `readScratch` 不提供 `allow`——放宽底线会被 §6 E4 拒绝，
 * 这里直接不给出该选项，让底线在 UI 上可见。
 */
export interface ApprovalRuleShortcut {
  key: "execute" | "edit" | "read" | "mcp" | "readOutput" | "readScratch";
  action: string;
  effects: ApprovalRuleEffectDto[];
}

export const APPROVAL_RULE_SHORTCUTS: ApprovalRuleShortcut[] = [
  { key: "execute", action: "execute", effects: ["allow", "ask", "deny"] },
  { key: "edit", action: "edit", effects: ["allow", "ask", "deny"] },
  { key: "read", action: "read", effects: ["ask", "deny"] },
  { key: "mcp", action: "mcp", effects: ["ask", "deny"] },
  { key: "readOutput", action: "readToolOutput", effects: ["ask", "deny"] },
  { key: "readScratch", action: "readScratch", effects: ["ask", "deny"] },
];

/** 校验失败原因（与 Rust `RuleValidationError` 一一对应，I70 §6.1 E1–E5）。 */
export type ApprovalRuleErrorCode =
  | "tooMany"
  | "actionLength"
  | "resourceLength"
  | "allowBottomLine"
  | "reservedTier";

export interface ApprovalRuleError {
  code: ApprovalRuleErrorCode;
  /** 1-based 行号；条数超限这类整体错误为 `null`。 */
  index: number | null;
  /** 触发错误的 `action`（E4 用于错误文案）。 */
  action: string;
  /** 实际长度或上限，由文案按 `code` 选用。 */
  len: number;
  limit: number;
}

/**
 * 动作模式的 glob 匹配：`*` 不跨 `/`，`**` 可跨，与 Rust `pattern_matches` 同语义。
 *
 * 比较一律按大小写不敏感：后端只在 Windows 上不敏感，这里更保守——
 * 只会多拒绝，不会放过底线动作。
 */
export function approvalActionMatches(pattern: string, value: string): boolean {
  if (pattern === "*" || pattern === "**") return true;
  return globMatches(
    Array.from(pattern.replace(/\\/g, "/")),
    Array.from(value.replace(/\\/g, "/")),
  );
}

function globMatches(pattern: string[], value: string[]): boolean {
  if (pattern.length === 0) return value.length === 0;
  if (pattern[0] === "*") {
    const double = pattern.length > 1 && pattern[1] === "*";
    const rest = pattern.slice(double ? 2 : 1);
    if (rest.length === 0) return double || !value.includes("/");
    for (let index = 0; index <= value.length; index += 1) {
      if (globMatches(rest, value.slice(index))) return true;
      if (index < value.length && !double && value[index] === "/") return false;
    }
    return false;
  }
  if (value.length === 0) return false;
  if (pattern[0].toLowerCase() !== value[0].toLowerCase()) return false;
  return globMatches(pattern.slice(1), value.slice(1));
}

/**
 * 提交前预校验（I70 §6.1 E1–E5），与 Rust `validate_rules` 同规则。
 *
 * 后端仍会复校验并以其结果为准；这里只负责让错误在本地即时可见。
 * E6（未知 `effect`）由封闭联合类型挡住，前端不提供第四种取值。
 */
export function validateApprovalRules(
  rules: ApprovalRuleDto[],
): ApprovalRuleError | null {
  if (rules.length > APPROVAL_RULES_MAX) {
    return {
      code: "tooMany",
      index: null,
      action: "",
      len: rules.length,
      limit: APPROVAL_RULES_MAX,
    };
  }
  for (const [offset, rule] of rules.entries()) {
    const index = offset + 1;
    const actionLength = Array.from(rule.action).length;
    if (actionLength === 0 || actionLength > APPROVAL_RULE_ACTION_MAX) {
      return {
        code: "actionLength",
        index,
        action: rule.action,
        len: actionLength,
        limit: APPROVAL_RULE_ACTION_MAX,
      };
    }
    const resourceLength = Array.from(rule.resource).length;
    if (resourceLength === 0 || resourceLength > APPROVAL_RULE_RESOURCE_MAX) {
      return {
        code: "resourceLength",
        index,
        action: rule.action,
        len: resourceLength,
        limit: APPROVAL_RULE_RESOURCE_MAX,
      };
    }
    if (rule.resource.startsWith("tier:")) {
      return {
        code: "reservedTier",
        index,
        action: rule.action,
        len: resourceLength,
        limit: APPROVAL_RULE_RESOURCE_MAX,
      };
    }
    if (
      rule.effect === "allow" &&
      (approvalActionMatches(rule.action, "readToolOutput") ||
        approvalActionMatches(rule.action, "readScratch") ||
        approvalActionMatches(rule.action, "mcp"))
    ) {
      return {
        code: "allowBottomLine",
        index,
        action: rule.action,
        len: actionLength,
        limit: APPROVAL_RULE_ACTION_MAX,
      };
    }
  }
  return null;
}
