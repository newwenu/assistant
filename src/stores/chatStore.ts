import { create } from "zustand";
import { type ChatOrchestrator } from "../services/chatOrchestrator";
import type { ContextUsage } from "../chat/core/chatTypes";
import {
  createStatePort,
  createEnvPort,
  createOrchestrator,
} from "./chatStorePorts";
import { type FetchTokenUsageDeps } from "../chat/core/chatUtils";
import {
  createConversationSlice,
  type ChatConversationSlice,
} from "./chatConversationSlice";
import { createMessageSlice, type ChatMessageSlice } from "./chatMessageSlice";
import { createCompactSlice, type ChatCompactSlice } from "./chatCompactSlice";
import { PersistQueue } from "../utils/persistQueue";

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
  };

let _orchestrator: ChatOrchestrator | null = null;
const persistQueue = new PersistQueue();

export const useChatStore = create<ChatState>()((set, get): ChatState => {
  const fetchTokenUsageDeps: FetchTokenUsageDeps = {
    isActiveConv: (convId: string) => get().currentConversationId === convId,
    setContextUsage: (usage: ContextUsage) => set({ contextUsage: usage }),
  };
  const statePort = createStatePort(set, get, fetchTokenUsageDeps);
  const envPort = createEnvPort(
    get,
    set,
    () => get().selectedProviderId,
    () => get().selectedModelId,
  );
  const orchestrator = createOrchestrator(statePort, envPort, {
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
  );
  const messageSlice = createMessageSlice(set, get, orchestrator, persistQueue);
  const compactSlice = createCompactSlice(set, get);

  return {
    ...conversationSlice,
    ...messageSlice,
    ...compactSlice,
    contextUsage: { usedTokens: 0, maxTokens: 128_000, percentage: 0 },
    selectedProviderId: null,
    selectedModelId: "",
    setSelectedProviderModel: (providerId, modelId) =>
      set({ selectedProviderId: providerId, selectedModelId: modelId }),
  };
});

export {
  createChatOrchestrator,
  type ChatOrchestrator,
  type ChatStatePort,
  type ChatEnvPort,
} from "../services/chatOrchestrator";
export function getChatOrchestrator(): ChatOrchestrator | null {
  return _orchestrator;
}
