import type { Message } from "../chat/core/chatTypes";
import { chatService } from "../services/chatService";
import { createLogger } from "@/services/logger";
import { notifyError } from "./notifyStore";
import { useChatStreamStore } from "./chatStreamStore";

const log = createLogger("chatCompactSlice");

export interface ChatCompactSlice {
  isFull: boolean;

  manualCompact: () => Promise<void>;
  unlockConversation: () => Promise<void>;
}

type FullState = ChatCompactSlice & {
  messages: Message[];
  currentConversationId: string | null;
};

export function createCompactSlice(
  set: (
    partial: Partial<FullState> | ((state: FullState) => Partial<FullState>),
  ) => void,
  get: () => FullState,
): ChatCompactSlice {
  return {
    isFull: false,

    manualCompact: async () => {
      const convId = get().currentConversationId;
      if (!convId) return;
      const summaryMsgId = `temp-compact-${Date.now()}`;
      const summaryMsg: Message = {
        id: summaryMsgId,
        conversationId: convId,
        role: "compact_summary",
        content: "",
        tokens: null,
        timestamp: Date.now(),
        parentId: null,
        isPinned: false,
        reasoning: null,
        siblingIndex: 0,
        siblingCount: null,
        providerId: null,
        modelId: null,
      };
      useChatStreamStore.getState().beginStream(summaryMsgId);
      set({
        messages: [...get().messages, summaryMsg],
      });
      try {
        await chatService.manualCompactConversation(convId);
      } catch (e) {
        log.error("Manual compact failed", e);
        notifyError(
          "chat.errors.compactFailed.title",
          "chat.errors.compactFailed.description",
        );
        useChatStreamStore.getState().cancelStreaming();
        set({
          messages: get().messages.filter((m) => m.id !== summaryMsgId),
        });
      }
    },

    unlockConversation: async () => {
      const convId = get().currentConversationId;
      if (!convId) return;
      try {
        await chatService.unlockConversation(convId);
      } catch (e) {
        log.error("Unlock conversation failed", e);
      }
    },
  };
}
