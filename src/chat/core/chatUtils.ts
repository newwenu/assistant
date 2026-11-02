import type {
  Message,
  ContextUsage,
  TokenUsage,
  TokenUsageSummary,
} from "./chatTypes";
import { chatService } from "@/services/chatService";

export interface FetchTokenUsageDeps {
  isActiveConv: (convId: string) => boolean;
  setContextUsage: (usage: ContextUsage) => void;
}

export function contextUsageFromTokenUsage(usage: TokenUsage): ContextUsage {
  const maxTokens =
    usage.contextWindowTokens && usage.contextWindowTokens > 0
      ? usage.contextWindowTokens
      : undefined;
  return {
    usedTokens: usage.promptTokens,
    hasUsage: true,
    maxTokens,
    percentage: maxTokens
      ? Math.min(usage.promptTokens / maxTokens, 1)
      : undefined,
    completionTokens: usage.completionTokens,
    cacheReadTokens: usage.cacheReadTokens,
    cacheWriteTokens: usage.cacheWriteTokens,
    reasoningTokens: usage.reasoningTokens,
  };
}

/** 从记录表派生摘要映射为展示模型；三组口径互不覆盖 */
export function contextUsageFromSummary(
  summary: TokenUsageSummary,
): ContextUsage {
  const o = summary.occupancy;
  return {
    usedTokens: o.usedTokens ?? 0,
    hasUsage: typeof o.usedTokens === "number",
    maxTokens: o.contextWindowTokens,
    percentage: o.contextWindowTokens ? o.percentage : undefined,
    completionTokens: o.completionTokens,
    cacheReadTokens: o.cacheReadTokens,
    cacheWriteTokens: o.cacheWriteTokens,
    reasoningTokens: o.reasoningTokens,
    cumulativeTokens: summary.cumulative?.totalTokens,
    estimated: o.estimated,
    estimatedSource: o.estimatedSource,
  };
}

const tokenUsageRequestSeq = new Map<string, number>();

export function fetchTokenUsage(convId: string, deps: FetchTokenUsageDeps) {
  const seq = (tokenUsageRequestSeq.get(convId) ?? 0) + 1;
  tokenUsageRequestSeq.set(convId, seq);
  chatService
    .getConversationTokenUsage(convId)
    .then((summary) => {
      if (!summary) return;
      if (tokenUsageRequestSeq.get(convId) !== seq) return;
      if (!deps.isActiveConv(convId)) return;
      deps.setContextUsage(contextUsageFromSummary(summary));
    })
    .catch(() => {});
}

export function mergeStreamingContent(
  messages: Message[],
  streamingMessageId: string | null,
  streamingContent: string,
): Message[] {
  if (!streamingMessageId) return messages;
  return messages.map((m) =>
    m.id === streamingMessageId
      ? { ...m, content: streamingContent || m.content }
      : m,
  );
}
