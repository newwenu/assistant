import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ChatOrchestrator,
  ChatDeltaPayload,
} from "../services/chatOrchestrator";
import {
  createChatDeltaCoalescer,
  createSeqDedupFilter,
  textChunkScheduler,
  type CoalescibleDelta,
} from "../chat/streamCoalescer";

const CHAT_DELTA_EVENT = "chat-delta";

let orchestratorInstance: ChatOrchestrator | null = null;

export function setChatOrchestrator(orchestrator: ChatOrchestrator): void {
  orchestratorInstance = orchestrator;
}

export function subscribeChatDeltaEvents(): () => void {
  let disposed = false;
  const unlisteners: UnlistenFn[] = [];

  const track = (promise: Promise<UnlistenFn>) => {
    promise
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      })
      .catch((error) => {
        console.debug("[chat-delta] listener unavailable:", error);
      });
  };

  const seqDedup = createSeqDedupFilter();

  const coalescer = createChatDeltaCoalescer<CoalescibleDelta>(
    (event) => {
      if (event.delta) {
        if (import.meta.env.DEV) {
          console.debug(
            `[coalescer-deliver] seq=${event.seq} len=${event.delta.length} snippet=${JSON.stringify(event.delta.slice(0, 40))}`,
          );
        }
        if (orchestratorInstance) {
          orchestratorInstance.onTextDelta(event.delta);
        }
      }
    },
    textChunkScheduler,
    seqDedup.accept,
  );

  track(
    listen(CHAT_DELTA_EVENT, (event) => {
      if (disposed) return;

      const payload = event.payload as ChatDeltaPayload;

      if (payload.delta_text) {
        coalescer.push({
          conversation_id: payload.conversation_id,
          message_id: payload.message_id,
          seq: payload.seq,
          delta: payload.delta_text,
        });
      }

      if (
        payload.delta_reasoning ||
        payload.delta_tool_call_id ||
        payload.delta_tool_call_name ||
        payload.delta_tool_call_arguments ||
        payload.usage ||
        payload.finish_reason ||
        payload.error
      ) {
        coalescer.flush();
        const nonTextPayload: ChatDeltaPayload = {
          conversation_id: payload.conversation_id,
          message_id: payload.message_id,
          seq: payload.seq,
          delta_reasoning: payload.delta_reasoning,
          delta_tool_call_id: payload.delta_tool_call_id,
          delta_tool_call_name: payload.delta_tool_call_name,
          delta_tool_call_arguments: payload.delta_tool_call_arguments,
          usage: payload.usage,
          finish_reason: payload.finish_reason,
          error: payload.error,
        };
        if (orchestratorInstance) {
          orchestratorInstance.handleDeltaEvent(nonTextPayload);
        }
      }

      if (payload.finish_reason) {
        seqDedup.reset();
      }
    }),
  );

  return () => {
    disposed = true;
    coalescer.dispose();
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}