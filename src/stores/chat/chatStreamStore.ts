import { create } from "zustand";
import type { RetryNotification } from "../../chat/core/chatTypes";

export interface ChatStreamState {
  isStreaming: boolean;
  streamingContent: string;
  streamingMessageId: string | null;
  /**
   * 当前流式段所属的迭代号（I38d §3.1.1：一次迭代 = 一条消息 = 一个气泡）。
   * 迭代号变化即提交上一段、开新段；`null` 表示尚未确定（如刚切回运行中的会话）。
   */
  streamingIteration: number | null;
  activeAttemptId: string | null;
  /**
   * 本轮 turn 序号（I46 §3.2）：由 `chat-stream-start` 下发，是占位消息赋坐标的唯一来源。
   * `null` 表示尚未确定（如刚切回运行中的会话），此时渲染侧平铺不折叠。
   */
  currentTurnIndex: number | null;
  isReasoningStreaming: boolean;
  retryStatus: RetryNotification | null;

  beginStream: (messageId: string) => void;
  /**
   * 开一条新的流式段（新气泡）：清空正文缓冲、换占位消息 id 与迭代号。
   * 与 `beginStream` 的差别是保留 `activeAttemptId`——turn 仍在进行。
   */
  beginSegment: (messageId: string, iteration: number) => void;
  setStreamingIteration: (iteration: number) => void;
  setCurrentTurnIndex: (turnIndex: number | null) => void;
  cancelStreaming: () => void;
  appendDelta: (delta: string) => void;
  setReasoningStreaming: (streaming: boolean) => void;
  setActiveAttemptId: (attemptId: string, messageId: string) => void;
  clearActiveAttemptId: () => void;
  /**
   * 切回仍在运行的会话：恢复流式视图（C5.2）。
   * `messageId` 未知时传 `null`，由后续 delta 补全；不覆盖已有内容。
   */
  resumeStream: (attemptId: string, messageId: string | null) => void;
  setRetryStatus: (retry: RetryNotification | null) => void;
  resetStreaming: () => void;
}

export const useChatStreamStore = create<ChatStreamState>()((set) => {
  const resetState = {
    isStreaming: false,
    streamingContent: "",
    streamingMessageId: null,
    streamingIteration: null,
    activeAttemptId: null,
    currentTurnIndex: null,
    isReasoningStreaming: false,
    retryStatus: null,
  };

  return {
    ...resetState,

    beginStream: (messageId: string) => {
      set({ ...resetState, isStreaming: true, streamingMessageId: messageId });
    },

    beginSegment: (messageId: string, iteration: number) => {
      set((state) => ({
        isStreaming: true,
        streamingContent: "",
        streamingMessageId: messageId,
        streamingIteration: iteration,
        activeAttemptId: state.activeAttemptId,
        isReasoningStreaming: false,
      }));
    },

    setStreamingIteration: (iteration: number) => {
      set({ streamingIteration: iteration });
    },

    setCurrentTurnIndex: (turnIndex: number | null) => {
      set({ currentTurnIndex: turnIndex });
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

    resumeStream: (attemptId, messageId) => {
      set((state) => ({
        isStreaming: true,
        activeAttemptId: attemptId,
        streamingMessageId: messageId ?? state.streamingMessageId,
      }));
    },

    setRetryStatus: (retry: RetryNotification | null) => {
      set({ retryStatus: retry });
    },

    resetStreaming: () => set(resetState),
  };
});
