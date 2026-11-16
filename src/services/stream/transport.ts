import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentToolCall,
  ChatDeltaPayload,
  ChatStreamStartPayload,
  ChatUserMessageConfirmedPayload,
  SessionState,
  SessionStateChangedPayload,
} from "../../chat/core/chatTypes";
import { ChatEvent } from "../../chat/core/chatTypes";
import {
  createChatDeltaCoalescer,
  createSeqDedupFilter,
  textChunkScheduler,
  type CoalescibleDelta,
} from "../../chat/core/stream/streamCoalescer";
import { createLogger } from "@/services/system/logger";

const log = createLogger("streamTransport");

/**
 * 文本增量：携带会话 / 轮次 / 迭代标识。
 * `iteration` 是链路私有语义，传输层只透传不解释（I38d §3.7）。
 */
export interface StreamTextChunk {
  conversationId: string;
  attemptId: string;
  messageId: string;
  /** 轮内迭代序号（从 1 起）；迭代边界即气泡边界。 */
  iteration: number;
  delta: string;
}

/**
 * 传输层的唯一出口：已清洗的增量 / 工具事件 / 状态事件。
 * 这里不出现任何链路判断（chat / agent 由编排层按策略选择）。
 */
export interface StreamTransportHandlers {
  getActiveConversationId: () => string | null;
  onStreamStart: (
    conversationId: string,
    attemptId: string,
    messageId: string,
    turnIndex: number | null,
    iteration: number | null,
  ) => void;
  onUserMessageConfirmed: (
    userMessageId: string,
    userMessageContent: string,
    turnIndex: number | null,
  ) => void;
  onSessionState: (payload: SessionStateChangedPayload) => void;
  onToolCall: (call: AgentToolCall) => void;
  onTextDelta: (chunk: StreamTextChunk) => void;
  onDeltaEvent: (payload: ChatDeltaPayload) => void;
}

interface StreamStartIds {
  conversationId: string;
  attemptId: string;
  messageId: string;
}

function describeSessionState(state: SessionState): string {
  if (state === "idle") return "idle";
  if ("processing" in state) {
    return `processing(${state.processing.phase},turn=${state.processing.turnId})`;
  }
  return `error(${state.error.message},recoverable=${state.error.recoverable})`;
}

/**
 * 订阅后端流式事件（两条链路共用一份实现）。
 *
 * 共用部分是纯传输职责：事件名、`conversationId` / `attemptId` 过滤、`seq` 去重、
 * delta 合并、帧调度、`seq` 单调性自检。复制第二份必然漂移（I16 的 `attemptId`
 * 过滤缺陷即前车之鉴），因此这里禁止按链路分叉。
 */
export function subscribeStreamTransport(
  handlers: StreamTransportHandlers,
): () => void {
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
      handlers.onStreamStart(
        payload.conversationId,
        payload.attemptId,
        payload.messageId,
        payload.turnIndex,
        payload.iteration,
      );
    }),
  );

  track(
    listen<ChatUserMessageConfirmedPayload>(
      ChatEvent.UserMessageConfirmed,
      (event) => {
        if (disposed) return;
        const payload = event.payload;
        if (handlers.getActiveConversationId() === payload.conversationId) {
          handlers.onUserMessageConfirmed(
            payload.userMessageId,
            payload.userMessageContent,
            payload.turnIndex,
          );
        }
      },
    ),
  );

  track(
    listen<SessionStateChangedPayload>(
      ChatEvent.SessionStateChanged,
      (event) => {
        if (disposed) return;
        const payload = event.payload;
        log.debug(
          `session-state conv=${payload.conversationId} turn=${payload.turnId} state=${describeSessionState(payload.state)}`,
        );
        handlers.onSessionState(payload);
      },
    ),
  );

  track(
    listen<AgentToolCall>(ChatEvent.AgentToolCall, (event) => {
      if (disposed) return;
      const payload = event.payload;
      if (handlers.getActiveConversationId() === payload.conversationId) {
        handlers.onToolCall(payload);
      }
    }),
  );

  const coalescer = createChatDeltaCoalescer<CoalescibleDelta>(
    (event) => {
      if (!event.delta) return;
      if (import.meta.env.DEV) {
        console.debug(
          `[coalescer-deliver] seq=${event.seq} iteration=${event.iteration} len=${event.delta.length} snippet=${JSON.stringify(event.delta.slice(0, 40))}`,
        );
      }
      handlers.onTextDelta({
        conversationId: event.conversationId,
        attemptId: event.attemptId,
        messageId: event.messageId,
        iteration: event.iteration,
        delta: event.delta,
      });
    },
    textChunkScheduler,
    seqDedup.accept,
  );

  const lastSeqByAttempt = new Map<string, number>();

  track(
    listen<ChatDeltaPayload>(ChatEvent.Delta, (event) => {
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
          iteration: payload.iteration,
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
        handlers.onDeltaEvent({
          conversationId: payload.conversationId,
          attemptId: payload.attemptId,
          messageId: payload.messageId,
          seq: payload.seq,
          iteration: payload.iteration,
          deltaText: null,
          deltaReasoning: payload.deltaReasoning,
          usage: payload.usage,
          finishReason: payload.finishReason,
          error: payload.error,
          deltaRetry: payload.deltaRetry,
        });
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
