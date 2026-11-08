import type { ConversationToolConfig } from "./chatTypes";

/** 会话链路：由会话所属 mode 的 `kind` 唯一决定（I38d §4.8）。 */
export type ConversationLink = "chat" | "agent";

/** 一次发送所需的链路判定结果；`toolConfig` 只对 agent 链路存在。 */
export interface ConversationRoute {
  link: ConversationLink;
  modeId: string | null;
  toolConfig: ConversationToolConfig | null;
}

/** 链路判定与工具下发只依赖这三个字段，避免把完整 `AgentMode` 传进纯函数。 */
export interface ModeLinkFields {
  id: string;
  kind: string;
  toolConfig: ConversationToolConfig;
}

/**
 * `mode.kind` → 链路。唯一规则：`agent` 归 agent 链路；
 * 其余（`chat` / 旧值 `default`、`quick`、`deep`、`custom` / `null` / 未知值 / mode 行缺失）
 * 一律归入 chat 链路。旧值仅作兼容标签，不参与判定。
 */
export function linkFromModeKind(
  kind: string | null | undefined,
): ConversationLink {
  return kind === "agent" ? "agent" : "chat";
}

/**
 * 会话链路解析的唯一实现：`modeId = null` 或查不到 mode 行时按 `default`
 * 处理（chat 链路）。`toolConfig` 只对 agent 链路下发，chat 恒为 `null`。
 * agent 链路取「会话持久值 ?? 模式初始值」，即每轮有效配置。
 */
export function resolveConversationRoute(
  modeId: string | null | undefined,
  modes: ReadonlyArray<ModeLinkFields>,
  conversationToolConfig: ConversationToolConfig | null = null,
): ConversationRoute {
  if (!modeId) return { link: "chat", modeId: null, toolConfig: null };
  const mode = modes.find((m) => m.id === modeId);
  const link = linkFromModeKind(mode?.kind);
  return {
    link,
    modeId,
    toolConfig:
      mode && link === "agent"
        ? (conversationToolConfig ?? mode.toolConfig)
        : null,
  };
}
