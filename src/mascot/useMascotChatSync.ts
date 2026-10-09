import { useEffect, useRef } from "react";
import { useChatStore } from "../stores/chatStore";
import { useMascotStore } from "./mascotStore";
import type { MascotFace } from "./faceStates";

const CONCERNED_RE = /\b(sorry|failed|error|cannot|unable|problem)\b/i;
const HAPPY_RE = /\b(done|completed|fixed|success|ready|great)\b/i;

function pickAckFace(
  fullText: string,
  hadToolCall?: boolean,
): MascotFace | null {
  if (!fullText.trim()) return null;
  if (CONCERNED_RE.test(fullText)) return "concerned";
  if (hadToolCall && HAPPY_RE.test(fullText)) return "proud";
  if (HAPPY_RE.test(fullText)) return "happy";
  if (hadToolCall) return "happy";
  return null;
}

export function useMascotChatSync(): void {
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

      if (justStartedStreaming && isReasoningStreaming) {
        useMascotStore.getState().setFace("thinking");
      } else if (justStartedStreaming && hadToolCall) {
        useMascotStore.getState().setFace("working");
      } else if (justStartedStreaming) {
        useMascotStore.getState().setFace("speaking");
      }

      if (justStoppedReasoning && isStreaming) {
        if (hadToolCall) {
          useMascotStore.getState().setFace("working");
        } else {
          useMascotStore.getState().setFace("speaking");
        }
      }

      if (justStoppedStreaming) {
        const lastAssistantMsg = [...messages]
          .reverse()
          .find((m) => m.role === "assistant");
        const fullText = lastAssistantMsg?.content ?? "";
        const ackFace = pickAckFace(fullText, hadToolCall) ?? "happy";
        useMascotStore.getState().holdThenIdle(ackFace);
      }

      if (hasError && !prevErrorRef.current) {
        useMascotStore.getState().holdThenIdle("concerned");
      }

      prevStreamingRef.current = isStreaming;
      prevReasoningRef.current = isReasoningStreaming;
      prevErrorRef.current = hasError;
    });

    const initial = useChatStore.getState();
    prevStreamingRef.current = initial.isStreaming;
    prevReasoningRef.current = initial.isReasoningStreaming;

    return unsubscribe;
  }, []);
}