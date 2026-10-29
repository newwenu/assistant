import type { Message, ContextUsage } from "./chatTypes";
import { chatService } from "@/services/chatService";
import { useProviderStore } from "@/stores/providerStore";

export function resolveContextWindowTokens(
  modelId: string,
  providerId: string | null,
): number | undefined {
  if (!providerId) return undefined;
  const provider = useProviderStore
    .getState()
    .providers.find((p) => p.id === providerId);
  if (!provider) return undefined;
  const model = provider.models.find((m) => m.id === modelId);
  if (model && model.contextWindow && model.contextWindow > 0) {
    return model.contextWindow;
  }
  return undefined;
}

export interface FetchTokenUsageDeps {
  isActiveConv: (convId: string) => boolean;
  setContextUsage: (usage: ContextUsage) => void;
}

export function fetchTokenUsage(
  convId: string,
  modelId: string,
  providerId: string | null,
  deps: FetchTokenUsageDeps,
) {
  chatService
    .getConversationTokenUsage(
      convId,
      resolveContextWindowTokens(modelId, providerId),
    )
    .then((summary) => {
      if (summary && deps.isActiveConv(convId)) {
        deps.setContextUsage({
          usedTokens: summary.totalTokens,
          maxTokens: summary.contextWindowTokens,
          percentage: summary.percentage,
        });
      }
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
