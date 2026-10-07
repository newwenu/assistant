import { useEffect, useRef } from "react";
import { useChatStore } from "../stores/chatStore";
import { useMascotStore } from "./mascotStore";
import { pickVisemeCode } from "./riveMaps";
import type { MascotFace } from "./faceStates";

const CONCERNED_RE = /\b(sorry|failed|error|cannot|unable|problem)\b/i;
const HAPPY_RE = /\b(done|completed|fixed|success|ready|great)\b/i;

function pickAckFace(fullText: string): MascotFace | null {
  if (!fullText.trim()) return null;
  if (CONCERNED_RE.test(fullText)) return "concerned";
  if (HAPPY_RE.test(fullText)) return "happy";
  return null;
}

export function useMascotChatSync(): void {
  const prevStreamingRef = useRef(false);
  const prevReasoningRef = useRef(false);
  const prevErrorRef = useRef(false);

  useEffect(() => {
    const unsubscribe = useChatStore.subscribe((state) => {
      const { isStreaming, isReasoningStreaming } = state;
      const hasError = state.messages.some(
        (m) => m.error != null && m.role === "assistant",
      );

      const justStartedStreaming = isStreaming && !prevStreamingRef.current;
      const justStoppedStreaming = !isStreaming && prevStreamingRef.current;
      const justStoppedReasoning =
        !isReasoningStreaming && prevReasoningRef.current;

      if (justStartedStreaming && isReasoningStreaming) {
        useMascotStore.getState().setFace("thinking");
      } else if (justStartedStreaming) {
        useMascotStore.getState().setFace("speaking");
      }

      if (justStoppedReasoning && isStreaming) {
        useMascotStore.getState().setFace("speaking");
      }

      if (justStoppedStreaming) {
        const lastAssistantMsg = [...state.messages]
          .reverse()
          .find((m) => m.role === "assistant");
        const fullText = lastAssistantMsg?.content ?? "";
        const ackFace = pickAckFace(fullText) ?? "happy";
        useMascotStore.getState().holdThenIdle(ackFace);
      }

      if (hasError && !prevErrorRef.current) {
        useMascotStore.getState().holdThenIdle("concerned");
      }

      if (isStreaming && !isReasoningStreaming) {
        const lastDelta = state.streamingContent.slice(-1);
        if (lastDelta) {
          useMascotStore.getState().setVisemeCode(pickVisemeCode(lastDelta));
        }
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
