import { create } from "zustand";
import type { RetryNotification } from "../chat/core/chatTypes";

export interface ChatStreamState {
  isStreaming: boolean;
  streamingContent: string;
  streamingMessageId: string | null;
  activeAttemptId: string | null;
  isReasoningStreaming: boolean;
  retryStatus: RetryNotification | null;

  beginStream: (messageId: string) => void;
  cancelStreaming: () => void;
  appendDelta: (delta: string) => void;
  setReasoningStreaming: (streaming: boolean) => void;
  setActiveAttemptId: (attemptId: string, messageId: string) => void;
  clearActiveAttemptId: () => void;
  setRetryStatus: (retry: RetryNotification | null) => void;
  resetStreaming: () => void;
}

export const useChatStreamStore = create<ChatStreamState>()((set) => {
  const resetState = {
    isStreaming: false,
    streamingContent: "",
    streamingMessageId: null,
    activeAttemptId: null,
    isReasoningStreaming: false,
    retryStatus: null,
  };

  return {
    ...resetState,

    beginStream: (messageId: string) => {
      set({ ...resetState, isStreaming: true, streamingMessageId: messageId });
    },

    cancelStreaming: () => set(resetState),

    appendDelta: (delta: string) => {
      set((state) => ({ streamingContent: state.streamingContent + delta }));
    },

    setReasoningStreaming: (streaming: boolean) => {
      set({ isReasoningStreaming: streaming });
    },

    setActiveAttemptId: (attemptId: string, messageId: string) => {
      set({ activeAttemptId: attemptId, streamingMessageId: messageId });
    },

    clearActiveAttemptId: () => {
      set({ activeAttemptId: null, streamingMessageId: null });
    },

    setRetryStatus: (retry: RetryNotification | null) => {
      set({ retryStatus: retry });
    },

    resetStreaming: () => set(resetState),
  };
});
