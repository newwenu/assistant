import { useEffect, useRef } from "react";
import { useChatStore } from "../stores/chat/chatStore";
import { useChatStreamStore } from "../stores/chat/chatStreamStore";
import { useMascotStore } from "./mascotStore";
import type { MascotScene, ExpressionPolicy, AckRule } from "./core/types";
import { DEFAULT_EXPRESSION_POLICY } from "./core/expressionPolicy";
import type { Message } from "../chat/core/chatTypes";

export type { ExpressionPolicy, AckRule } from "./core/types";
export { DEFAULT_EXPRESSION_POLICY } from "./core/expressionPolicy";

function pickAckScene(
  fullText: string,
  hadToolCall: boolean,
  rules: AckRule[],
  fallback: MascotScene,
): MascotScene {
  if (!fullText.trim()) return fallback;
  const sorted = [...rules].sort(
    (a, b) => (b.priority ?? 0) - (a.priority ?? 0),
  );
  for (const rule of sorted) {
    if (rule.requiresToolCall && !hadToolCall) continue;
    if (rule.pattern.test(fullText)) return rule.scene;
  }
  return fallback;
}

function hadToolCallInRecentTurn(messages: Message[]): boolean {
  const lastUserMsgIdx = [...messages]
    .reverse()
    .findIndex((m) => m.role === "user");
  return (
    lastUserMsgIdx >= 0 &&
    messages
      .slice(messages.length - 1 - lastUserMsgIdx)
      .some((m) => m.role === "tool")
  );
}

export function useMascotChatSync(
  policy: ExpressionPolicy = DEFAULT_EXPRESSION_POLICY,
): void {
  const prevStreamingRef = useRef(false);
  const prevReasoningRef = useRef(false);
  const prevErrorRef = useRef(false);

  useEffect(() => {
    const unsubscribeChat = useChatStore.subscribe((chatState) => {
      const messages = chatState.messages;
      const streamState = useChatStreamStore.getState();
      const { isStreaming, isReasoningStreaming } = streamState;

      const hasError = messages.some(
        (m) => m.error != null && m.role === "assistant",
      );
      const toolCallDetected = hadToolCallInRecentTurn(messages);

      const justStartedStreaming = isStreaming && !prevStreamingRef.current;
      const justStoppedStreaming = !isStreaming && prevStreamingRef.current;
      const justStoppedReasoning =
        !isReasoningStreaming && prevReasoningRef.current;
      const { chatScene, ackRules } = policy;

      if (justStartedStreaming && isReasoningStreaming) {
        useMascotStore.getState().setScene(chatScene.onStreamStart.reasoning);
      } else if (justStartedStreaming && toolCallDetected) {
        useMascotStore.getState().setScene(chatScene.onStreamStart.toolCall);
      } else if (justStartedStreaming) {
        useMascotStore.getState().setScene(chatScene.onStreamStart.default);
      }

      if (justStoppedReasoning && isStreaming) {
        if (toolCallDetected) {
          useMascotStore.getState().setScene(chatScene.onReasoningEnd.toolCall);
        } else {
          useMascotStore.getState().setScene(chatScene.onReasoningEnd.default);
        }
      }

      if (justStoppedStreaming) {
        const lastAssistantMsg = [...messages]
          .reverse()
          .find((m) => m.role === "assistant");
        const fullText = lastAssistantMsg?.content ?? "";
        const ackScene = pickAckScene(
          fullText,
          toolCallDetected,
          ackRules,
          chatScene.onStreamEnd,
        );
        useMascotStore.getState().holdSceneThenIdle(ackScene);
      }

      if (hasError && !prevErrorRef.current) {
        useMascotStore.getState().holdSceneThenIdle(chatScene.onError);
      }

      prevStreamingRef.current = isStreaming;
      prevReasoningRef.current = isReasoningStreaming;
      prevErrorRef.current = hasError;
    });

    const initialStream = useChatStreamStore.getState();
    prevStreamingRef.current = initialStream.isStreaming;
    prevReasoningRef.current = initialStream.isReasoningStreaming;

    return () => {
      unsubscribeChat();
    };
  }, [policy]);
}
