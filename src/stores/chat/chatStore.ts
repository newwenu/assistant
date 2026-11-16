import { create } from "zustand";
import { type TurnOrchestrator } from "../../services/stream/turnOrchestrator";
import type { ContextUsage } from "../../chat/core/chatTypes";
import {
  resolveProviderModel,
  type ProviderModelRef,
} from "../../chat/core/providerModel";
import { useProviderStore } from "../providerStore";
import {
  createStatePort,
  createEnvPort,
  createOrchestrator,
} from "./chatStorePorts";
import { type FetchTokenUsageDeps } from "../../chat/core/chatUtils";
import {
  createConversationSlice,
  type ChatConversationSlice,
} from "./chatConversationSlice";
import { createMessageSlice, type ChatMessageSlice } from "./chatMessageSlice";
import { createCompactSlice, type ChatCompactSlice } from "./chatCompactSlice";
import { PersistQueue } from "../../utils/persistQueue";
import {
  DEFAULT_REASONING_CHOICE,
  type ReasoningChoice,
} from "../../chat/core/reasoning";

type ChatState = ChatConversationSlice &
  ChatMessageSlice &
  ChatCompactSlice & {
    contextUsage: ContextUsage;
    selectedProviderId: string | null;
    selectedModelId: string;
    setSelectedProviderModel: (
      providerId: string | null,
      modelId: string,
    ) => void;
    reasoningChoice: ReasoningChoice;
    setReasoningChoice: (choice: ReasoningChoice) => void;
    resetContextWindow: () => void;
    /**
     * 选中 provider / model 的唯一校正入口：以当前会话声明的组合为准，
     * 该组合失效（provider 被删除、模型被移除）时回退到可用组合。
     */
    syncSelectionFromConversation: (ref?: ProviderModelRef) => void;
  };

let _orchestrator: TurnOrchestrator | null = null;
const persistQueue = new PersistQueue();

export const useChatStore = create<ChatState>()((set, get): ChatState => {
  const fetchTokenUsageDeps: FetchTokenUsageDeps = {
    isActiveConv: (convId: string) => get().currentConversationId === convId,
    setContextUsage: (usage: ContextUsage) => set({ contextUsage: usage }),
  };
  const { port: statePort, segments } = createStatePort(
    set,
    get,
    fetchTokenUsageDeps,
  );
  const envPort = createEnvPort(
    get,
    set,
    () => get().selectedProviderId,
    () => get().selectedModelId,
  );
  const orchestrator = createOrchestrator(statePort, envPort, segments, {
    loadConversations: () => get().loadConversations(),
    switchConversation: (id: string) => get().switchConversation(id),
    renameConversation: (id: string, title: string, titleSource: string) =>
      get().renameConversation(id, title, titleSource),
  });

  _orchestrator = orchestrator;

  const conversationSlice = createConversationSlice(
    set,
    get,
    fetchTokenUsageDeps,
    persistQueue,
    () => _orchestrator,
  );
  const messageSlice = createMessageSlice(set, get, orchestrator, persistQueue);
  const compactSlice = createCompactSlice(set, get);

  return {
    ...conversationSlice,
    ...messageSlice,
    ...compactSlice,
    contextUsage: { usedTokens: 0, hasUsage: false },
    selectedProviderId: null,
    selectedModelId: "",
    setSelectedProviderModel: (providerId, modelId) =>
      set({ selectedProviderId: providerId, selectedModelId: modelId }),
    reasoningChoice: DEFAULT_REASONING_CHOICE,
    setReasoningChoice: (choice) => set({ reasoningChoice: choice }),
    resetContextWindow: () => {
      const usage = get().contextUsage;
      set({
        contextUsage: {
          usedTokens: usage.usedTokens,
          hasUsage: usage.hasUsage,
        },
      });
    },
    syncSelectionFromConversation: (ref?: ProviderModelRef) => {
      const providers = useProviderStore.getState().providers;
      if (providers.length === 0) return;
      const state = get();
      const conv = state.conversations.find(
        (c) => c.id === state.currentConversationId,
      );
      const source =
        ref ??
        (conv ? { providerId: conv.providerId, modelId: conv.modelId } : null);
      if (!source) return;
      const resolved = resolveProviderModel(
        providers,
        source.providerId,
        source.modelId,
      );
      if (
        resolved.providerId === state.selectedProviderId &&
        resolved.modelId === state.selectedModelId
      ) {
        return;
      }
      set({
        selectedProviderId: resolved.providerId,
        selectedModelId: resolved.modelId,
      });
    },
  };
});

/**
 * provider 列表变化（保存 / 删除 / 跨窗口同步）后校正选中项，
 * 让「选中项可用」由 store 自己保证，视图不再为此挂 effect。
 */
useProviderStore.subscribe((state, prev) => {
  if (state.providers === prev.providers) return;
  useChatStore.getState().syncSelectionFromConversation();
});

export type { TurnOrchestrator };
export function getChatOrchestrator(): TurnOrchestrator | null {
  return _orchestrator;
}
