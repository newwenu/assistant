import { useEffect, useRef } from "react";
import { useChatStore } from "../stores/chatStore";
import { useMascotStore } from "./mascotStore";
import type { MascotFace, ExpressionPolicy, AckRule } from "./core/types";
import { DEFAULT_EXPRESSION_POLICY } from "./core/expressionPolicy";

export type { ExpressionPolicy, AckRule } from "./core/types";
export { DEFAULT_EXPRESSION_POLICY } from "./core/expressionPolicy";

function pickAckFace(
  fullText: string,
  hadToolCall: boolean,
  rules: AckRule[],
  fallback: MascotFace,
): MascotFace {
  if (!fullText.trim()) return fallback;
  const sorted = [...rules].sort(
    (a, b) => (b.priority ?? 0) - (a.priority ?? 0),
  );
  for (const rule of sorted) {
    if (rule.requiresToolCall && !hadToolCall) continue;
    if (rule.pattern.test(fullText)) return rule.face;
  }
  return fallback;
}

export function useMascotChatSync(
  policy: ExpressionPolicy = DEFAULT_EXPRESSION_POLICY,
): void {
  const prevStreamingRef = useRef(false);
  const prevReasoningRef = useRef(false);
  const prevErrorRef = useRef(false);

  useEffect(() => {
    const unsubscribe = useChatStore.subscribe((state) => {
      const { isStreaming, isReasoningStreaming, messages } = state;
      const hasError = messages.some(
        (m) => m.error != null && m.role === "assistant",
      );

      const lastUserMsgIdx = [...messages]
        .reverse()
        .findIndex((m) => m.role === "user");
      const hadToolCall =
        lastUserMsgIdx >= 0 &&
        messages
          .slice(messages.length - 1 - lastUserMsgIdx)
          .some(
            (m) =>
              m.role === "tool" ||
              (m.role === "assistant" && (m.toolCalls?.length ?? 0) > 0),
          );

      const justStartedStreaming = isStreaming && !prevStreamingRef.current;
      const justStoppedStreaming = !isStreaming && prevStreamingRef.current;
      const justStoppedReasoning =
        !isReasoningStreaming && prevReasoningRef.current;
      const { chatFace, ackRules } = policy;

      if (justStartedStreaming && isReasoningStreaming) {
        useMascotStore.getState().setFace(chatFace.onStreamStart.reasoning);
      } else if (justStartedStreaming && hadToolCall) {
        useMascotStore.getState().setFace(chatFace.onStreamStart.toolCall);
      } else if (justStartedStreaming) {
        useMascotStore.getState().setFace(chatFace.onStreamStart.default);
      }

      if (justStoppedReasoning && isStreaming) {
        if (hadToolCall) {
          useMascotStore.getState().setFace(chatFace.onReasoningEnd.toolCall);
        } else {
          useMascotStore.getState().setFace(chatFace.onReasoningEnd.default);
        }
      }

      if (justStoppedStreaming) {
        const lastAssistantMsg = [...messages]
          .reverse()
          .find((m) => m.role === "assistant");
        const fullText = lastAssistantMsg?.content ?? "";
        const ackFace = pickAckFace(
          fullText,
          hadToolCall,
          ackRules,
          chatFace.onStreamEnd,
        );
        useMascotStore.getState().holdThenIdle(ackFace);
      }

      if (hasError && !prevErrorRef.current) {
        useMascotStore.getState().holdThenIdle(chatFace.onError);
      }

      prevStreamingRef.current = isStreaming;
      prevReasoningRef.current = isReasoningStreaming;
      prevErrorRef.current = hasError;
    });

    const initial = useChatStore.getState();
    prevStreamingRef.current = initial.isStreaming;
    prevReasoningRef.current = initial.isReasoningStreaming;

    return unsubscribe;
  }, [policy]);
}
