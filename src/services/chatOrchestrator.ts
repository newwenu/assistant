import { chatService } from "./chatService";
import type {
  Message,
  Conversation,
  TokenUsage,
  ContextUsage,
} from "../chat/chatTypes";
import type { ChatError } from "../chat/chatErrors";
import { ChatErrorCode, toChatError } from "../chat/chatErrors";
import { CHAT_AUTO_TITLE_LENGTH } from "../config/constants";

export type ChatPhase =
  "idle" | "sending" | "streaming" | "tool_executing" | "error";

export interface ChatStatePort {
  beginSend: (messages: Message[], assistantMsgId: string) => void;
  cancelStreaming: () => void;
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
}

export interface ChatEnvPort {
  getActiveConversationId: () => string | null;
  getIsStreaming: () => boolean;
  getStreamingMessageId: () => string | null;
  getMessages: () => Message[];
  getConversations: () => Conversation[];
  getSelectedModeId: () => string | null;
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

export interface ChatDeltaPayload {
  conversation_id: string;
  message_id: string;
  seq?: number;
  delta_text?: string;
  delta_reasoning?: string;
  delta_tool_call_id?: string;
  delta_tool_call_name?: string;
  delta_tool_call_arguments?: string;
  finish_reason?: string;
  usage?: TokenUsage;
  error?: unknown;
}

export interface ChatOrchestrator {
  send: (content: string) => Promise<void>;
  cancel: () => Promise<void>;
  handleDeltaEvent: (payload: ChatDeltaPayload) => void;
  onTextDelta: (delta: string) => void;
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

export function createChatOrchestrator(
  deps: ChatOrchestratorDeps,
): ChatOrchestrator {
  let sendInProgress = false;
  let phase: ChatPhase = "idle";

  function setPhase(next: ChatPhase) {
    phase = next;
  }

  async function send(content: string) {
    if (sendInProgress) return;
    sendInProgress = true;
    setPhase("sending");

    try {
      if (deps.env.getIsStreaming()) {
        const prevConvId = deps.env.getActiveConversationId();
        if (prevConvId) {
          await chatService.cancelChat(prevConvId).catch(() => {});
        }
        deps.state.cancelStreaming();
      }

      let convId = deps.env.getActiveConversationId();
      if (!convId) {
        const provList = await chatService.listProviders();
        if (provList.length === 0) {
          const errConvId = `err-${Date.now()}`;
          const userMsg: Message = {
            id: `temp-user-${Date.now()}`,
            conversation_id: errConvId,
            role: "user",
            content,
            tokens: null,
            timestamp: Date.now(),
          };
          const assistantMsgId = `temp-assistant-${Date.now()}`;
          const assistantMsg: Message = {
            id: assistantMsgId,
            conversation_id: errConvId,
            role: "assistant",
            content: "",
            tokens: null,
            timestamp: Date.now() + 1,
          };
          deps.state.beginSend([userMsg, assistantMsg], assistantMsgId);
          deps.state.errorStream({
            code: ChatErrorCode.ApiKeyMissing,
            retry_after_ms: null,
            retryable: false,
          });
          return;
        }
        const p = provList[0];
        const modeId = deps.env.getSelectedModeId();
        const newId = await chatService.createConversation(
          p.id,
          p.default_model || "",
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
          retry_after_ms: null,
          retryable: false,
        });
        return;
      }

      const currentMessages = deps.env.getMessages();
      const userMsg: Message = {
        id: `temp-user-${Date.now()}`,
        conversation_id: convId,
        role: "user",
        content,
        tokens: null,
        timestamp: Date.now(),
      };
      const assistantMsgId = `temp-assistant-${Date.now()}`;
      const assistantMsg: Message = {
        id: assistantMsgId,
        conversation_id: convId,
        role: "assistant",
        content: "",
        tokens: null,
        timestamp: Date.now() + 1,
      };

      deps.state.beginSend(
        [...currentMessages, userMsg, assistantMsg],
        assistantMsgId,
      );

      const convs = deps.env.getConversations();
      const conv = convs.find((c) => c.id === convId);
      if (conv && conv.title_source === "placeholder") {
        const fbTitle = fallbackTitleFromUserMessage(content);
        void deps.renameConversation(convId, fbTitle, "fallback");
      }

      try {
        const modeId = deps.env.getSelectedModeId();
        await chatService.sendChatMessage(convId, content, modeId);
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
    if (payload.error) {
      setPhase("error");
      deps.state.errorStream(toChatError(payload.error));
      return;
    }

    if (payload.delta_reasoning) {
      const messageId = deps.env.getStreamingMessageId();
      if (messageId) {
        if (phase === "sending") setPhase("streaming");
        deps.state.setReasoningStreaming(true);
        deps.state.appendReasoningDelta(messageId, payload.delta_reasoning);
      }
    }

    if (
      payload.delta_tool_call_id ||
      payload.delta_tool_call_name ||
      payload.delta_tool_call_arguments
    ) {
      const messageId = deps.env.getStreamingMessageId();
      if (messageId) {
        if (phase === "sending") setPhase("streaming");
        deps.state.appendToolCallDelta(
          messageId,
          payload.delta_tool_call_id,
          payload.delta_tool_call_name,
          payload.delta_tool_call_arguments,
        );
      }
    }

    if (payload.usage) {
      const u = payload.usage;
      const ctx =
        u.context_window_tokens && u.context_window_tokens > 0
          ? u.context_window_tokens
          : 128_000;
      deps.state.updateContextUsage({
        used_tokens: u.prompt_tokens,
        max_tokens: ctx,
        percentage: u.prompt_tokens / ctx,
      });
    }

    if (payload.finish_reason) {
      setPhase("idle");
      deps.state.finishStream(payload.usage);
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
    getSendInProgress: () => sendInProgress,
    getPhase: () => phase,
  };
}
