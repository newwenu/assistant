import type {
  ApprovalLevelDto,
  ConversationToolConfigDto,
  ToolTierDto,
} from "../../types/protocol.contract";

/** 工具层级的完整取值集合（顺序即 UI 展示顺序）。 */
export const TOOL_TIERS: ToolTierDto[] = ["sandbox", "os", "external"];

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
