import type { TurnOrchestrator } from "./stream/turnOrchestrator";
import { subscribeStreamTransport } from "./stream/transport";

let orchestratorInstance: TurnOrchestrator | null = null;

export function setChatOrchestrator(orchestrator: TurnOrchestrator): void {
  orchestratorInstance = orchestrator;
}

/**
 * 订阅后端流式事件。
 *
 * 事件名 / 过滤 / 去重 / 合并 / 调度等纯传输职责已搬到 `stream/transport.ts`
 * （两条链路共用一份，I38d §3.7）；这里只做「传输层 → 编排层」的接线。
 */
export function subscribeChatStreamEvents(): () => void {
  return subscribeStreamTransport({
    getActiveConversationId: () =>
      orchestratorInstance?.getActiveConversationId() ?? null,
    onStreamStart: (
      conversationId,
      attemptId,
      messageId,
      turnIndex,
      iteration,
    ) => {
      orchestratorInstance?.onStreamStart(
        conversationId,
        attemptId,
        messageId,
        turnIndex,
        iteration,
      );
    },
    onUserMessageConfirmed: (userMessageId, userMessageContent, turnIndex) => {
      orchestratorInstance?.onUserMessageConfirmed(
        userMessageId,
        userMessageContent,
        turnIndex,
      );
    },
    onSessionState: (payload) => {
      orchestratorInstance?.onSessionState(payload);
    },
    onToolCall: (call) => {
      orchestratorInstance?.onAgentToolCall(call);
    },
    onTextDelta: (chunk) => {
      orchestratorInstance?.onTextDelta(chunk);
    },
    onDeltaEvent: (payload) => {
      orchestratorInstance?.handleDeltaEvent(payload);
    },
  });
}
