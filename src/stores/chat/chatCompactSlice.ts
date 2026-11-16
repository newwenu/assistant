import type { Message } from "../../chat/core/chatTypes";
import { createOptimisticMessage } from "../../chat/core/stream/optimisticMessage";
import {
  chatService,
  isCompactCancelled,
} from "../../services/chat/chatService";
import { createLogger } from "@/services/system/logger";
import { notifyError } from "../system/notifyStore";
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
      const summaryMsg: Message = createOptimisticMessage({
        id: summaryMsgId,
        conversationId: convId,
        role: "compact_summary",
        content: "",
        timestamp: Date.now(),
        parentId: null,
      });
      useChatStreamStore.getState().beginStream(summaryMsgId);
      set({
        messages: [...get().messages, summaryMsg],
      });
      try {
        await chatService.manualCompactConversation(convId);
      } catch (e) {
        useChatStreamStore.getState().cancelStreaming();
        set({
          messages: get().messages.filter((m) => m.id !== summaryMsgId),
        });
        if (isCompactCancelled(e)) {
          log.info("Manual compact cancelled", e);
          return;
        }
        log.error("Manual compact failed", e);
        notifyError(
          "chat.errors.compactFailed.title",
          "chat.errors.compactFailed.description",
        );
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
