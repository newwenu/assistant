import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ChatOrchestrator } from "./chatOrchestrator";
import type {
  ChatDeltaPayload,
  ChatStreamStartPayload,
  ChatUserMessageConfirmedPayload,
} from "../chat/core/chatTypes";
import { ChatEvent } from "../chat/core/chatTypes";
import {
  createChatDeltaCoalescer,
  createSeqDedupFilter,
  textChunkScheduler,
  type CoalescibleDelta,
} from "../chat/core/streamCoalescer";
import { createLogger } from "@/services/logger";

const CHAT_DELTA_EVENT = ChatEvent.Delta;
const log = createLogger("chatStreamEvents");

let orchestratorInstance: ChatOrchestrator | null = null;

export function setChatOrchestrator(orchestrator: ChatOrchestrator): void {
  orchestratorInstance = orchestrator;
}

interface StreamStartIds {
  conversationId: string;
  attemptId: string;
  messageId: string;
}

export function subscribeChatStreamEvents(): () => void {
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
    listen<ChatStreamStartPayload>(ChatEvent.StreamStart, (event) => {
      if (disposed) return;
      const payload = event.payload;
      streamStartIds = {
        conversationId: payload.conversationId,
        attemptId: payload.attemptId,
        messageId: payload.messageId,
      };
      firstDeltaChecked = false;
      if (
        orchestratorInstance &&
        orchestratorInstance.getActiveConversationId() ===
          payload.conversationId
      ) {
        orchestratorInstance.onStreamStart(
          payload.attemptId,
          payload.messageId,
        );
      }
    }),
  );

  track(
    listen<ChatUserMessageConfirmedPayload>(
      ChatEvent.UserMessageConfirmed,
      (event) => {
        if (disposed) return;
        const payload = event.payload;
        if (
          orchestratorInstance &&
          orchestratorInstance.getActiveConversationId() ===
            payload.conversationId
        ) {
          orchestratorInstance.onUserMessageConfirmed(
            payload.userMessageId,
            payload.userMessageContent,
          );
        }
      },
    ),
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
    listen<ChatDeltaPayload>(CHAT_DELTA_EVENT, (event) => {
      if (disposed) return;

      const payload = event.payload;

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
          deltaText: null,
          deltaReasoning: payload.deltaReasoning,
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

  return () => {
    disposed = true;
    coalescer.dispose();
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
}
