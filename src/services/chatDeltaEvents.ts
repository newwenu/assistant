import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ChatOrchestrator } from "../services/chatOrchestrator";
import type {
  ChatDeltaPayload,
  ChatStreamStartPayload,
  ConversationTerminatedPayload,
  ConversationCompactFailedPayload,
  ConversationUnlockedPayload,
} from "../chat/core/protocol";
import {
  createChatDeltaCoalescer,
  createSeqDedupFilter,
  textChunkScheduler,
  type CoalescibleDelta,
} from "../chat/core/streamCoalescer";
import { useChatStore } from "../stores/chatStore";

const CHAT_DELTA_EVENT = "chat-delta";

let orchestratorInstance: ChatOrchestrator | null = null;

export function setChatOrchestrator(orchestrator: ChatOrchestrator): void {
  orchestratorInstance = orchestrator;
}

interface StreamStartIds {
  conversation_id: string;
  attempt_id: string;
  message_id: string;
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
  let streamStartIds: StreamStartIds | null = null;
  let firstDeltaChecked = false;

  track(
    listen("chat-stream-start", (event) => {
      if (disposed) return;
      const payload = event.payload as ChatStreamStartPayload;
      streamStartIds = {
        conversation_id: payload.conversation_id,
        attempt_id: payload.attempt_id,
        message_id: payload.message_id,
      };
      firstDeltaChecked = false;
      const store = useChatStore.getState();
      if (store.currentConversationId === payload.conversation_id) {
        store.setActiveAttemptId(payload.attempt_id, payload.message_id);
      }
    }),
  );

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

  const lastSeqByAttempt = new Map<string, number>();

  track(
    listen(CHAT_DELTA_EVENT, (event) => {
      if (disposed) return;

      const payload = event.payload as ChatDeltaPayload;

      if (!firstDeltaChecked && streamStartIds) {
        firstDeltaChecked = true;
        const ids = streamStartIds;
        if (
          ids.conversation_id !== payload.conversation_id ||
          ids.attempt_id !== payload.attempt_id ||
          ids.message_id !== payload.message_id
        ) {
          console.error(
            `[R3] stream-start/delta id mismatch: start=(${ids.conversation_id},${ids.attempt_id},${ids.message_id}) delta=(${payload.conversation_id},${payload.attempt_id},${payload.message_id})`,
          );
        }
      }

      const lastSeq = lastSeqByAttempt.get(payload.attempt_id);
      if (lastSeq !== undefined && payload.seq <= lastSeq) {
        console.error(
          `[R4] seq non-monotonic: attempt_id=${payload.attempt_id} prev_seq=${lastSeq} curr_seq=${payload.seq}`,
        );
      }
      lastSeqByAttempt.set(payload.attempt_id, payload.seq);

      if (payload.delta_text) {
        coalescer.push({
          conversation_id: payload.conversation_id,
          attempt_id: payload.attempt_id,
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
        payload.error ||
        payload.delta_retry
      ) {
        coalescer.flush();
        const nonTextPayload: ChatDeltaPayload = {
          conversation_id: payload.conversation_id,
          attempt_id: payload.attempt_id,
          message_id: payload.message_id,
          seq: payload.seq,
          delta_reasoning: payload.delta_reasoning,
          delta_tool_call_id: payload.delta_tool_call_id,
          delta_tool_call_name: payload.delta_tool_call_name,
          delta_tool_call_arguments: payload.delta_tool_call_arguments,
          usage: payload.usage,
          finish_reason: payload.finish_reason,
          error: payload.error,
          delta_retry: payload.delta_retry,
        };
        if (orchestratorInstance) {
          orchestratorInstance.handleDeltaEvent(nonTextPayload);
        }
      }

      if (payload.finish_reason) {
        seqDedup.reset();
        lastSeqByAttempt.delete(payload.attempt_id);
      }
    }),
  );

  track(
    listen("conversation_terminated", (event) => {
      if (disposed) return;
      const payload = event.payload as ConversationTerminatedPayload;
      const store = useChatStore.getState();
      if (store.currentConversationId === payload.conversation_id) {
        useChatStore.setState({
          isFull: payload.is_full,
          compactSummary: payload.summary,
          compactError: null,
        });
      }
      void useChatStore.getState().loadConversations(true);
    }),
  );

  track(
    listen("conversation_compact_failed", (event) => {
      if (disposed) return;
      const payload = event.payload as ConversationCompactFailedPayload;
      const store = useChatStore.getState();
      if (store.currentConversationId === payload.conversation_id) {
        useChatStore.setState({ compactError: payload.error });
      }
    }),
  );

  track(
    listen("conversation_unlocked", (event) => {
      if (disposed) return;
      const payload = event.payload as ConversationUnlockedPayload;
      const store = useChatStore.getState();
      if (store.currentConversationId === payload.conversation_id) {
        useChatStore.setState({
          isFull: false,
          compactSummary: null,
          compactError: null,
        });
      }
      void useChatStore.getState().loadConversations(true);
    }),
  );

  return () => {
    disposed = true;
    coalescer.dispose();
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
