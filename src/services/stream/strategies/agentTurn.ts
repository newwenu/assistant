import type { AgentSegments } from "../../../stores/agent/agentSegments";
import type { StreamStatePort, TurnStrategy } from "../turnStrategy";

/**
 * agent 链路策略：一轮多迭代，迭代边界即气泡边界（I38d §3.1.1）。
 *
 * 与 chat 的差异集中在这里，不再散落到通用编排里：
 * - 文本 / 推理增量按 `iteration` 归属到对应段；
 * - `finishReason` 只结束单次模型调用，turn 结束由 `session-state-changed idle` 收口；
 * - 工具卡片按事件 `iteration` 归到发起它的那条消息；
 * - turn 收尾时把仍挂起的工具调用收敛到终态。
 */
export function createAgentTurnStrategy(
  state: StreamStatePort,
  segments: AgentSegments,
): TurnStrategy {
  return {
    kind: "agent",

    onBeginSend: (assistantMsgId) => {
      segments.reset(assistantMsgId);
    },

    onTextDelta: (chunk) => {
      state.setReasoningStreaming(false);
      segments.appendDelta(chunk.iteration, chunk.delta);
    },

    onReasoningDelta: (chunk) => {
      if (!chunk.deltaReasoning) return;
      state.setReasoningStreaming(true);
      segments.appendReasoningDelta(chunk.iteration, chunk.deltaReasoning);
    },

    onFinishReason: () => {
      state.setRetryStatus(null);
      segments.commitSegment();
    },

    onDeltaError: (error) => {
      segments.settleToolCalls();
      state.errorStream(error);
    },

    onToolCall: (call) => {
      segments.applyToolCall(call.iteration, call);
    },

    onTurnIdle: () => {
      segments.settleToolCalls();
      state.finishStream();
    },

    onTurnError: () => {
      segments.settleToolCalls();
      state.cancelStreaming();
    },

    onCancel: () => {
      segments.settleToolCalls();
      state.cancelStreaming();
    },

    onResume: (messageId, iteration) => {
      if (messageId !== null && iteration !== undefined) {
        segments.adoptSegment(iteration, messageId);
        return;
      }
      segments.markUnknownSegment();
    },

    onMessageIdResolved: (previousId, realMessageId) => {
      segments.renameSegment(previousId, realMessageId);
    },
  };
}
