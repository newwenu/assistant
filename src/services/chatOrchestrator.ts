import { chatService } from "./chatService";
import { providerService } from "./providerService";
import type {
  Message,
  Conversation,
  TokenUsage,
  ContextUsage,
  ChatDeltaPayload,
  RetryNotification,
} from "../chat/core/chatTypes";
import type { ChatError } from "../chat/core/chatErrors";
import { ChatErrorCode, toChatError } from "../chat/core/chatErrors";
import { CHAT_AUTO_TITLE_LENGTH } from "../config/constants";

export type ChatPhase =
  "idle" | "sending" | "streaming" | "tool_executing" | "error";

export interface ChatStatePort {
  beginSend: (messages: Message[], assistantMsgId: string) => void;
  cancelStreaming: () => void;
  setActiveAttemptId: (attemptId: string, realMessageId: string) => void;
  clearActiveAttemptId: () => void;
  appendDelta: (delta: string) => void;
  setReasoningStreaming: (streaming: boolean) => void;
  appendReasoningDelta: (messageId: string, delta: string) => void;
  appendToolCallDelta: (
    messageId: string,
    toolCallId: string | undefined,
    toolCallName: string | undefined,
    toolCallArguments: string | undefined,
  ) => void;
  updateContextUsage: (usage: ContextUsage) => void;
  finishStream: (usage?: TokenUsage) => void;
  errorStream: (error: ChatError) => void;
  setRetryStatus: (retry: RetryNotification | null) => void;
}

export interface ChatEnvPort {
  getActiveConversationId: () => string | null;
  getIsStreaming: () => boolean;
  getStreamingMessageId: () => string | null;
  getActiveAttemptId: () => string | null;
  getMessages: () => Message[];
  getConversations: () => Conversation[];
  getSelectedModeId: () => string | null;
  getPendingBranchPoint: () => string | null;
  clearPendingBranchPoint: () => void;
}

export interface ChatOrchestratorDeps {
  state: ChatStatePort;
  env: ChatEnvPort;
  loadConversations: () => Promise<void>;
  switchConversation: (id: string) => Promise<void>;
  renameConversation: (
    id: string,
    title: string,
    titleSource: string,
  ) => Promise<void>;
}

export interface ChatOrchestrator {
  send: (content: string) => Promise<void>;
  cancel: () => Promise<void>;
  handleDeltaEvent: (payload: ChatDeltaPayload) => void;
  onTextDelta: (delta: string) => void;
  ensureIdle: () => Promise<void>;
  getSendInProgress: () => boolean;
  getPhase: () => ChatPhase;
}

function fallbackTitleFromUserMessage(
  userMessage: string,
  maxLength = CHAT_AUTO_TITLE_LENGTH,
): string {
  const firstLine = userMessage.split("\n").find((l) => l.trim()) || "";
  if (firstLine.length <= maxLength) return firstLine.trim();
  const puncts = ["。", "？", "！", ".", "?", "!", "；", ";", "，", ","];
  for (const p of puncts) {
    const idx = firstLine.indexOf(p);
    if (idx > 0 && idx < maxLength) {
      return firstLine.slice(0, idx + 1).trim();
    }
  }
  return firstLine.slice(0, maxLength).trim() + "…";
}

const ENSURE_IDLE_TIMEOUT_MS = 5000;

export function createChatOrchestrator(
  deps: ChatOrchestratorDeps,
): ChatOrchestrator {
  let sendInProgress = false;
  let phase: ChatPhase = "idle";
  const handledTerminalEvents = new Set<string>();

  function setPhase(next: ChatPhase) {
    phase = next;
  }

  async function ensureIdle(): Promise<void> {
    if (!sendInProgress && phase === "idle") return;

    const convId = deps.env.getActiveConversationId();
    if (convId) {
      await chatService.cancelChat(convId).catch(() => {});
    }
    deps.state.cancelStreaming();

    const start = Date.now();
    await new Promise<void>((resolve) => {
      const check = () => {
        if (phase === "idle" || phase === "error") {
          if (phase === "error") setPhase("idle");
          resolve();
        } else if (Date.now() - start > ENSURE_IDLE_TIMEOUT_MS) {
          console.warn(
            "[orchestrator] ensureIdle timeout, force resetting phase",
          );
          setPhase("idle");
          resolve();
        } else {
          setTimeout(check, 50);
        }
      };
      setTimeout(check, 50);
    });

    sendInProgress = false;
  }

  async function send(content: string) {
    if (sendInProgress) return;
    sendInProgress = true;
    handledTerminalEvents.clear();
    setPhase("sending");

    try {
      if (deps.env.getIsStreaming()) {
        const prevConvId = deps.env.getActiveConversationId();
        if (prevConvId) {
          await chatService.cancelChat(prevConvId).catch(() => {});
        }
        deps.state.clearActiveAttemptId();
        deps.state.cancelStreaming();
      }

      let convId = deps.env.getActiveConversationId();
      if (!convId) {
        const provList = await providerService.listProviders();
        if (provList.length === 0) {
          const errConvId = `err-${Date.now()}`;
          const userMsg: Message = {
            id: `temp-user-${Date.now()}`,
            conversationId: errConvId,
            role: "user",
            content,
            tokens: null,
            timestamp: Date.now(),
            siblingIndex: 0,
          };
          const assistantMsgId = `temp-assistant-${Date.now()}`;
          const assistantMsg: Message = {
            id: assistantMsgId,
            conversationId: errConvId,
            role: "assistant",
            content: "",
            tokens: null,
            timestamp: Date.now() + 1,
            siblingIndex: 0,
          };
          deps.state.beginSend([userMsg, assistantMsg], assistantMsgId);
          deps.state.errorStream({
            code: ChatErrorCode.ApiKeyMissing,
            retryAfterMs: null,
            retryable: false,
          });
          return;
        }
        const p = provList[0];
        const modeId = deps.env.getSelectedModeId();
        const newId = await chatService.createConversation(
          p.id,
          p.defaultModel || "",
          modeId,
        );
        await deps.loadConversations();
        await deps.switchConversation(newId);
        convId = newId;
      }

      if (!convId) {
        deps.state.errorStream({
          code: ChatErrorCode.InternalError,
          message: "No conversation available",
          retryAfterMs: null,
          retryable: false,
        });
        return;
      }

      const currentMessages = deps.env.getMessages();
      const pendingBranchPoint = deps.env.getPendingBranchPoint();
      if (pendingBranchPoint) {
        deps.env.clearPendingBranchPoint();
      }

      const branchPointMsg = pendingBranchPoint
        ? currentMessages.find((m) => m.id === pendingBranchPoint)
        : undefined;

      const userParentId = branchPointMsg
        ? branchPointMsg.parentId
        : (currentMessages[currentMessages.length - 1]?.id ?? null);

      let optimisticMessages: Message[];

      if (branchPointMsg) {
        const branchPointIndex = currentMessages.findIndex(
          (m) => m.id === pendingBranchPoint,
        );
        const baseMessages =
          branchPointIndex >= 0
            ? currentMessages.slice(0, branchPointIndex)
            : currentMessages;
        optimisticMessages = [...baseMessages];
      } else {
        optimisticMessages = [...currentMessages];
      }

      if (branchPointMsg && branchPointMsg.role === "assistant") {
        const assistantMsgId = `temp-assistant-${Date.now()}`;
        const assistantMsg: Message = {
          id: assistantMsgId,
          conversationId: convId,
          role: "assistant",
          content: "",
          tokens: null,
          timestamp: Date.now(),
          parentId: userParentId,
          siblingIndex: 0,
        };
        optimisticMessages.push(assistantMsg);
        deps.state.beginSend(optimisticMessages, assistantMsgId);
      } else {
        const userMsg: Message = {
          id: `temp-user-${Date.now()}`,
          conversationId: convId,
          role: "user",
          content,
          tokens: null,
          timestamp: Date.now(),
          parentId: userParentId,
          siblingIndex: 0,
        };
        const assistantMsgId = `temp-assistant-${Date.now()}`;
        const assistantMsg: Message = {
          id: assistantMsgId,
          conversationId: convId,
          role: "assistant",
          content: "",
          tokens: null,
          timestamp: Date.now() + 1,
          parentId: userMsg.id,
          siblingIndex: 0,
        };
        optimisticMessages.push(userMsg, assistantMsg);
        deps.state.beginSend(optimisticMessages, assistantMsgId);
      }

      const convs = deps.env.getConversations();
      const conv = convs.find((c) => c.id === convId);
      if (conv && conv.titleSource === "placeholder") {
        const fbTitle = fallbackTitleFromUserMessage(content);
        void deps.renameConversation(convId, fbTitle, "fallback");
      }

      try {
        const modeId = deps.env.getSelectedModeId();
        await chatService.sendChatMessage(
          convId,
          content,
          modeId,
          pendingBranchPoint,
        );
      } catch (e) {
        console.error("[orchestrator] sendChatMessage failed:", e);
        deps.state.errorStream(toChatError(e));
      }
    } finally {
      sendInProgress = false;
    }
  }

  async function cancel() {
    const convId = deps.env.getActiveConversationId();
    if (!convId) return;
    try {
      await chatService.cancelChat(convId);
    } catch (e) {
      console.error("[orchestrator] cancel failed:", e);
    }
    deps.state.cancelStreaming();
    setPhase("idle");
  }

  function handleDeltaEvent(payload: ChatDeltaPayload) {
    const activeAttemptId = deps.env.getActiveAttemptId();

    if (activeAttemptId === null || payload.attemptId !== activeAttemptId) {
      return;
    }

    if (payload.error) {
      const terminalKey = `${payload.attemptId}:error`;
      if (handledTerminalEvents.has(terminalKey)) return;
      handledTerminalEvents.add(terminalKey);
      setPhase("error");
      deps.state.errorStream(toChatError(payload.error));
      return;
    }

    if (payload.deltaRetry) {
      deps.state.setRetryStatus(payload.deltaRetry);
      return;
    }

    if (payload.deltaReasoning) {
      const messageId = deps.env.getStreamingMessageId();
      if (messageId) {
        if (phase === "sending") setPhase("streaming");
        deps.state.setReasoningStreaming(true);
        deps.state.appendReasoningDelta(messageId, payload.deltaReasoning);
      }
    }

    if (
      payload.deltaToolCallId ||
      payload.deltaToolCallName ||
      payload.deltaToolCallArguments
    ) {
      const messageId = deps.env.getStreamingMessageId();
      if (messageId) {
        if (phase === "sending") setPhase("streaming");
        deps.state.appendToolCallDelta(
          messageId,
          payload.deltaToolCallId,
          payload.deltaToolCallName,
          payload.deltaToolCallArguments,
        );
      }
    }

    if (payload.usage) {
      const usage = payload.usage;
      const ctx =
        usage.contextWindowTokens && usage.contextWindowTokens > 0
          ? usage.contextWindowTokens
          : 128_000;
      deps.state.updateContextUsage({
        usedTokens: usage.promptTokens,
        maxTokens: ctx,
        percentage: usage.promptTokens / ctx,
      });
    }

    if (payload.finishReason) {
      const terminalKey = `${payload.attemptId}:finish`;
      if (handledTerminalEvents.has(terminalKey)) return;
      handledTerminalEvents.add(terminalKey);
      setPhase("idle");
      deps.state.setRetryStatus(null);
      deps.state.finishStream(payload.usage ?? undefined);
    }
  }

  function onTextDelta(delta: string) {
    if (phase === "sending") setPhase("streaming");
    deps.state.setReasoningStreaming(false);
    deps.state.appendDelta(delta);
  }

  return {
    send,
    cancel,
    handleDeltaEvent,
    onTextDelta,
    ensureIdle,
    getSendInProgress: () => sendInProgress,
    getPhase: () => phase,
  };
}
