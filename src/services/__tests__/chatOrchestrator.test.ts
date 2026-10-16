import { describe, it, expect, vi, beforeEach } from "vitest";
import { createChatOrchestrator } from "../chatOrchestrator";
import type {
  ChatStatePort,
  ChatEnvPort,
  ChatOrchestratorDeps,
  ChatOrchestrator,
} from "../chatOrchestrator";
import type { ProviderErrorPayloadRow } from "../../chat/core/protocol";
import { ChatErrorCode } from "../../chat/core/chatErrors";

function makeError(message: string): ProviderErrorPayloadRow {
  return {
    code: ChatErrorCode.StreamError,
    message,
    retry_after_ms: null,
    retryable: false,
  };
}

vi.mock("../chatService", () => ({
  chatService: {
    listProviders: vi.fn(() =>
      Promise.resolve([{ id: "p1", default_model: "m1" }]),
    ),
    createConversation: vi.fn(() => Promise.resolve("conv-new")),
    sendChatMessage: vi.fn(() => Promise.resolve()),
    cancelChat: vi.fn(() => Promise.resolve()),
  },
}));

function createMockDeps(): {
  deps: ChatOrchestratorDeps;
  state: ChatStatePort;
  env: ChatEnvPort;
} {
  const state: ChatStatePort = {
    beginSend: vi.fn(),
    cancelStreaming: vi.fn(),
    setActiveAttemptId: vi.fn(),
    clearActiveAttemptId: vi.fn(),
    appendDelta: vi.fn(),
    setReasoningStreaming: vi.fn(),
    appendReasoningDelta: vi.fn(),
    appendToolCallDelta: vi.fn(),
    updateContextUsage: vi.fn(),
    finishStream: vi.fn(),
    errorStream: vi.fn(),
    setRetryStatus: vi.fn(),
  };

  const env: ChatEnvPort = {
    getActiveConversationId: vi.fn(() => "conv-1"),
    getIsStreaming: vi.fn(() => false),
    getStreamingMessageId: vi.fn(() => "msg-assistant"),
    getActiveAttemptId: vi.fn(() => "conv-1:1"),
    getMessages: vi.fn(() => []),
    getConversations: vi.fn(() => []),
    getSelectedModeId: vi.fn(() => null),
  };

  const deps: ChatOrchestratorDeps = {
    state,
    env,
    loadConversations: vi.fn(() => Promise.resolve()),
    switchConversation: vi.fn(() => Promise.resolve()),
    renameConversation: vi.fn(() => Promise.resolve()),
  };

  return { deps, state, env };
}

describe("chatOrchestrator", () => {
  let orchestrator: ChatOrchestrator;
  let mockDeps: ReturnType<typeof createMockDeps>;

  beforeEach(() => {
    mockDeps = createMockDeps();
    orchestrator = createChatOrchestrator(mockDeps.deps);
  });

  describe("send", () => {
    it("calls beginSend with user + assistant messages", async () => {
      await orchestrator.send("hello");
      const [messages, assistantMsgId] = vi.mocked(mockDeps.state.beginSend)
        .mock.calls[0];
      expect(messages[0].role).toBe("user");
      expect(messages[0].content).toBe("hello");
      expect(messages[1].role).toBe("assistant");
      expect(messages[1].id).toBe(assistantMsgId);
    });

    it("prevents concurrent sends", async () => {
      const { chatService } = await import("../chatService");
      vi.mocked(chatService.sendChatMessage).mockImplementation(
        () => new Promise((resolve) => setTimeout(resolve, 100)),
      );
      await Promise.all([
        orchestrator.send("first"),
        orchestrator.send("second"),
      ]);
      expect(chatService.sendChatMessage).toHaveBeenCalledTimes(1);
    });

    it("cancels existing stream before sending", async () => {
      const { chatService } = await import("../chatService");
      vi.mocked(mockDeps.env.getIsStreaming).mockReturnValue(true);
      vi.mocked(mockDeps.env.getActiveConversationId).mockReturnValue(
        "conv-prev",
      );
      await orchestrator.send("hello");
      expect(chatService.cancelChat).toHaveBeenCalledWith("conv-prev");
    });
  });

  describe("handleDeltaEvent — routing", () => {
    it("routes delta_reasoning → appendReasoningDelta", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: "conv-1:1",
        message_id: "msg-1",
        seq: 1,
        delta_reasoning: "thinking...",
      });
      expect(mockDeps.state.appendReasoningDelta).toHaveBeenCalledWith(
        "msg-assistant",
        "thinking...",
      );
    });

    it("routes usage → updateContextUsage (DTO→business conversion)", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: "conv-1:1",
        message_id: "msg-1",
        seq: 1,
        usage: {
          prompt_tokens: 100,
          completion_tokens: 50,
          total_tokens: 150,
          cache_read_tokens: 0,
          cache_write_tokens: 0,
          context_window_tokens: 1000,
        },
      });
      expect(mockDeps.state.updateContextUsage).toHaveBeenCalledWith({
        usedTokens: 100,
        maxTokens: 1000,
        percentage: 0.1,
      });
    });

    it("routes finish_reason → finishStream with business TokenUsage", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: "conv-1:1",
        message_id: "msg-assistant",
        seq: 1,
        finish_reason: "stop",
        usage: {
          prompt_tokens: 100,
          completion_tokens: 50,
          total_tokens: 150,
          cache_read_tokens: 0,
          cache_write_tokens: 0,
          context_window_tokens: 0,
        },
      });
      expect(mockDeps.state.finishStream).toHaveBeenCalledWith({
        promptTokens: 100,
        completionTokens: 50,
        totalTokens: 150,
        cacheReadTokens: 0,
        cacheWriteTokens: 0,
        contextWindowTokens: 0,
      });
    });
  });

  describe("handleDeltaEvent — stale attempt_id filtering", () => {
    it("ignores all event types from stale attempt_id", () => {
      const staleId = "conv-1:0";
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: staleId,
        message_id: "msg-1",
        seq: 1,
        delta_reasoning: "stale",
      });
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: staleId,
        message_id: "msg-1",
        seq: 1,
        usage: {
          prompt_tokens: 100,
          completion_tokens: 50,
          total_tokens: 150,
          cache_read_tokens: 0,
          cache_write_tokens: 0,
          context_window_tokens: 1000,
        },
      });
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: staleId,
        message_id: "msg-1",
        seq: 1,
        error: makeError("stale error"),
      });
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: staleId,
        message_id: "msg-1",
        seq: 1,
        finish_reason: "stop",
      });
      expect(mockDeps.state.appendReasoningDelta).not.toHaveBeenCalled();
      expect(mockDeps.state.updateContextUsage).not.toHaveBeenCalled();
      expect(mockDeps.state.errorStream).not.toHaveBeenCalled();
      expect(mockDeps.state.finishStream).not.toHaveBeenCalled();
    });

    it("processes events from current attempt_id after stale ones", () => {
      const currentId = "conv-1:2";
      vi.mocked(mockDeps.env.getActiveAttemptId).mockReturnValue(currentId);

      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: currentId,
        message_id: "msg-new",
        seq: 1,
        delta_reasoning: "fresh reasoning",
      });
      expect(mockDeps.state.appendReasoningDelta).toHaveBeenCalledWith(
        "msg-assistant",
        "fresh reasoning",
      );

      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: currentId,
        message_id: "msg-new",
        seq: 1,
        usage: {
          prompt_tokens: 200,
          completion_tokens: 80,
          total_tokens: 280,
          cache_read_tokens: 0,
          cache_write_tokens: 0,
          context_window_tokens: 2000,
        },
      });
      expect(mockDeps.state.updateContextUsage).toHaveBeenCalledWith({
        usedTokens: 200,
        maxTokens: 2000,
        percentage: 0.1,
      });
    });

    it("filters events when activeAttemptId is null", () => {
      vi.mocked(mockDeps.env.getActiveAttemptId).mockReturnValue(null);
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: "conv-1:1",
        message_id: "msg-1",
        seq: 1,
        error: makeError("cancelled"),
      });
      expect(mockDeps.state.errorStream).not.toHaveBeenCalled();
    });
  });

  describe("ChatPhase state machine", () => {
    it("idle → sending on send, → idle on finish, → error on error", async () => {
      expect(orchestrator.getPhase()).toBe("idle");
      const sendPromise = orchestrator.send("hello");
      expect(orchestrator.getPhase()).toBe("sending");
      await sendPromise;

      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: "conv-1:1",
        message_id: "msg-assistant",
        seq: 1,
        error: makeError("fail"),
      });
      expect(orchestrator.getPhase()).toBe("error");
    });

    it("transitions to idle on finish", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: "conv-1:1",
        message_id: "msg-assistant",
        seq: 1,
        finish_reason: "stop",
      });
      expect(orchestrator.getPhase()).toBe("idle");
    });
  });

  describe("attempt_id — editMessage 重发", () => {
    it("旧 attempt_id 的 finish/error 被忽略，新的正常处理", () => {
      const secondAttemptId = "conv-1:2";
      vi.mocked(mockDeps.env.getActiveAttemptId).mockReturnValue(
        secondAttemptId,
      );

      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: "conv-1:1",
        message_id: "msg-1",
        seq: 1,
        finish_reason: "stop",
      });
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: "conv-1:1",
        message_id: "msg-1",
        seq: 1,
        error: makeError("old error"),
      });
      expect(mockDeps.state.finishStream).not.toHaveBeenCalled();
      expect(mockDeps.state.errorStream).not.toHaveBeenCalled();

      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        attempt_id: secondAttemptId,
        message_id: "msg-2",
        seq: 1,
        finish_reason: "stop",
      });
      expect(mockDeps.state.finishStream).toHaveBeenCalled();
    });
  });
});
