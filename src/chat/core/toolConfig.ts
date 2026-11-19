import type {
  ApprovalLevelDto,
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

/** 审批级别的完整取值集合（顺序即 UI 展示顺序）。 */
export const APPROVAL_LEVELS: ApprovalLevelDto[] = [
  "none",
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
  };
}

/** 差异是否为空（无任何可用性变化）。 */
export function isEmptyToolConfigDiff(diff: ToolConfigDiff): boolean {
  return (
    diff.addedTools.length === 0 &&
    diff.removedTools.length === 0 &&
    diff.tierChanged === null &&
    diff.levelChanged === null
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
