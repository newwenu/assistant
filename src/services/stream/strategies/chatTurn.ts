import type { StreamStatePort, TurnStrategy } from "../turnStrategy";

/**
 * chat 链路策略：整轮只有一条 assistant 消息、一个气泡。
 *
 * - 文本增量直接累加到当前缓冲；
 * - `finishReason` 即 turn 结束（chat 没有多迭代概念）；
 * - 不订阅 `agent-tool-call`，也不存在工具终态收敛。
 */
export function createChatTurnStrategy(state: StreamStatePort): TurnStrategy {
  return {
    kind: "chat",

    onBeginSend: () => {},

    onTextDelta: (chunk) => {
      state.setReasoningStreaming(false);
      state.appendDelta(chunk.delta);
    },

    onReasoningDelta: (chunk) => {
      if (!chunk.deltaReasoning) return;
      state.setReasoningStreaming(true);
      state.appendReasoningDelta(chunk.deltaReasoning);
    },

    onFinishReason: (payload) => {
      state.setRetryStatus(null);
      state.finishStream(payload.usage ?? undefined);
    },

    onDeltaError: (error) => {
      state.errorStream(error);
    },

    onToolCall: () => {},

    onTurnIdle: () => {
      state.finishStream();
    },

    onTurnError: () => {
      state.cancelStreaming();
    },

    onCancel: () => {
      state.cancelStreaming();
    },

    onResume: () => {},

    onMessageIdResolved: () => {},
  };
}
