import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ChatOrchestrator } from "../services/chatOrchestrator";
import type {
  ChatDeltaPayload,
  ChatStreamStartPayload,
  ConversationTerminatedPayload,
  ConversationCompactFailedPayload,
  ConversationUnlockedPayload,
  ChatMessagesChangedPayload,
} from "../chat/core/chatTypes";
import {
  createChatDeltaCoalescer,
  createSeqDedupFilter,
  textChunkScheduler,
  type CoalescibleDelta,
} from "../chat/core/streamCoalescer";
import { toChatError } from "../chat/core/chatErrors";
import { useChatStore } from "../stores/chatStore";
import { createLogger } from "@/services/logger";

const CHAT_DELTA_EVENT = "chat-delta";
const log = createLogger("chatDeltaEvents");

let orchestratorInstance: ChatOrchestrator | null = null;

export function setChatOrchestrator(orchestrator: ChatOrchestrator): void {
  orchestratorInstance = orchestrator;
}

interface StreamStartIds {
  conversationId: string;
  attemptId: string;
  messageId: string;
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
        log.debug("listener unavailable", error);
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
        conversationId: payload.conversationId,
        attemptId: payload.attemptId,
        messageId: payload.messageId,
      };
      firstDeltaChecked = false;
      const store = useChatStore.getState();
      if (store.currentConversationId === payload.conversationId) {
        store.setActiveAttemptId(payload.attemptId, payload.messageId);
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
          ids.conversationId !== payload.conversationId ||
          ids.attemptId !== payload.attemptId ||
          ids.messageId !== payload.messageId
        ) {
          log.error(
            `R3 stream-start/delta id mismatch: start=(${ids.conversationId},${ids.attemptId},${ids.messageId}) delta=(${payload.conversationId},${payload.attemptId},${payload.messageId})`,
          );
        }
      }

      const lastSeq = lastSeqByAttempt.get(payload.attemptId);
      if (lastSeq !== undefined && payload.seq <= lastSeq) {
        log.error(
          `R4 seq non-monotonic: attemptId=${payload.attemptId} prevSeq=${lastSeq} currSeq=${payload.seq}`,
        );
      }
      lastSeqByAttempt.set(payload.attemptId, payload.seq);

      if (payload.deltaText) {
        coalescer.push({
          conversationId: payload.conversationId,
          attemptId: payload.attemptId,
          messageId: payload.messageId,
          seq: payload.seq,
          delta: payload.deltaText,
        });
      }

      if (
        payload.deltaReasoning ||
        payload.deltaToolCallId ||
        payload.deltaToolCallName ||
        payload.deltaToolCallArguments ||
        payload.usage ||
        payload.finishReason ||
        payload.error ||
        payload.deltaRetry
      ) {
        coalescer.flush();
        const nonTextPayload: ChatDeltaPayload = {
          conversationId: payload.conversationId,
          attemptId: payload.attemptId,
          messageId: payload.messageId,
          seq: payload.seq,
          deltaReasoning: payload.deltaReasoning,
          deltaToolCallId: payload.deltaToolCallId,
          deltaToolCallName: payload.deltaToolCallName,
          deltaToolCallArguments: payload.deltaToolCallArguments,
          usage: payload.usage,
          finishReason: payload.finishReason,
          error: payload.error,
          deltaRetry: payload.deltaRetry,
        };
        if (orchestratorInstance) {
          orchestratorInstance.handleDeltaEvent(nonTextPayload);
        }
      }

      if (payload.finishReason) {
        seqDedup.reset();
        lastSeqByAttempt.delete(payload.attemptId);
      }
    }),
  );

  track(
    listen("conversation_terminated", (event) => {
      if (disposed) return;
      const payload = event.payload as ConversationTerminatedPayload;
      const store = useChatStore.getState();
      if (store.currentConversationId === payload.conversationId) {
        useChatStore.setState({
          isFull: payload.isFull,
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
      if (store.currentConversationId === payload.conversationId) {
        useChatStore.setState({ compactError: toChatError(payload.error) });
      }
    }),
  );

  track(
    listen("conversation_unlocked", (event) => {
      if (disposed) return;
      const payload = event.payload as ConversationUnlockedPayload;
      const store = useChatStore.getState();
      if (store.currentConversationId === payload.conversationId) {
        useChatStore.setState({
          isFull: false,
          compactSummary: null,
          compactError: null,
        });
      }
      void useChatStore.getState().loadConversations(true);
    }),
  );

  let lastProcessedSeq = 0;

  track(
    listen<ChatMessagesChangedPayload>("chat-messages-changed", (event) => {
      if (disposed) return;
      const payload = event.payload;
      const store = useChatStore.getState();
      if (store.currentConversationId === payload.conversationId) {
        if (payload._seq !== undefined && payload._seq <= lastProcessedSeq) {
          void store.switchConversation(payload.conversationId);
          return;
        }
        if (payload._seq !== undefined) {
          lastProcessedSeq = payload._seq;
        }
        useChatStore.setState({
          messages: payload.messages,
          activeBranchId: payload.activeBranchId,
        });
      }
    }),
  );

  return () => {
    disposed = true;
    coalescer.dispose();
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
