import { describe, it, expect, vi, beforeEach } from "vitest";
import { createChatOrchestrator } from "../chatOrchestrator";
import type {
  ChatStatePort,
  ChatEnvPort,
  ChatOrchestratorDeps,
  ChatOrchestrator,
} from "../chatOrchestrator";
import type { ChatError } from "../../chat/core/chatErrors";
import { ChatErrorCode } from "../../chat/core/chatErrors";

function makeError(message: string): ChatError {
  return {
    code: ChatErrorCode.StreamError,
    message,
    retryAfterMs: null,
    retryable: false,
  };
}

vi.mock("../chatService", () => ({
  chatService: {
    listProviders: vi.fn(() =>
      Promise.resolve([{ id: "p1", defaultModel: "m1" }]),
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
    confirmUserMessage: vi.fn(),
    appendDelta: vi.fn(),
    setReasoningStreaming: vi.fn(),
    appendReasoningDelta: vi.fn(),
    appendToolCallDelta: vi.fn(),
    updateContextUsage: vi.fn(),
    finishStream: vi.fn(),
    errorStream: vi.fn(),
    setErrorOnMessage: vi.fn(),
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
    getSelectedProviderId: vi.fn(() => "provider-1"),
    getSelectedModelId: vi.fn(() => "model-1"),
    getPendingBranchPoint: vi.fn(() => null),
    clearPendingBranchPoint: vi.fn(),
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
    it("routes deltaReasoning → appendReasoningDelta", () => {
      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-1",
        seq: 1,
        deltaReasoning: "thinking...",
      });
      expect(mockDeps.state.appendReasoningDelta).toHaveBeenCalledWith(
        "msg-assistant",
        "thinking...",
      );
    });

    it("routes usage → updateContextUsage (DTO→business conversion)", () => {
      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-1",
        seq: 1,
        usage: {
          promptTokens: 100,
          completionTokens: 50,
          totalTokens: 150,
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          contextWindowTokens: 1000,
        },
      });
      expect(mockDeps.state.updateContextUsage).toHaveBeenCalledWith({
        usedTokens: 100,
        maxTokens: 1000,
        percentage: 0.1,
      });
    });

    it("routes finishReason → finishStream with business TokenUsage", () => {
      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-assistant",
        seq: 1,
        finishReason: "stop",
        usage: {
          promptTokens: 100,
          completionTokens: 50,
          totalTokens: 150,
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          contextWindowTokens: 0,
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

  describe("handleDeltaEvent — stale attemptId filtering", () => {
    it("ignores all event types from stale attemptId", () => {
      const staleId = "conv-1:0";
      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: staleId,
        messageId: "msg-1",
        seq: 1,
        deltaReasoning: "stale",
      });
      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: staleId,
        messageId: "msg-1",
        seq: 1,
        usage: {
          promptTokens: 100,
          completionTokens: 50,
          totalTokens: 150,
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          contextWindowTokens: 1000,
        },
      });
      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: staleId,
        messageId: "msg-1",
        seq: 1,
        error: makeError("stale error"),
      });
      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: staleId,
        messageId: "msg-1",
        seq: 1,
        finishReason: "stop",
      });
      expect(mockDeps.state.appendReasoningDelta).not.toHaveBeenCalled();
      expect(mockDeps.state.updateContextUsage).not.toHaveBeenCalled();
      expect(mockDeps.state.errorStream).not.toHaveBeenCalled();
      expect(mockDeps.state.finishStream).not.toHaveBeenCalled();
    });

    it("processes events from current attemptId after stale ones", () => {
      const currentId = "conv-1:2";
      vi.mocked(mockDeps.env.getActiveAttemptId).mockReturnValue(currentId);

      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: currentId,
        messageId: "msg-new",
        seq: 1,
        deltaReasoning: "fresh reasoning",
      });
      expect(mockDeps.state.appendReasoningDelta).toHaveBeenCalledWith(
        "msg-assistant",
        "fresh reasoning",
      );

      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: currentId,
        messageId: "msg-new",
        seq: 1,
        usage: {
          promptTokens: 200,
          completionTokens: 80,
          totalTokens: 280,
          cacheReadTokens: 0,
          cacheWriteTokens: 0,
          contextWindowTokens: 2000,
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
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-1",
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
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-assistant",
        seq: 1,
        error: makeError("fail"),
      });
      expect(orchestrator.getPhase()).toBe("error");
    });

    it("transitions to idle on finish", () => {
      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-assistant",
        seq: 1,
        finishReason: "stop",
      });
      expect(orchestrator.getPhase()).toBe("idle");
    });
  });

  describe("attemptId — editMessage 重发", () => {
    it("旧 attemptId 的 finish/error 被忽略，新的正常处理", () => {
      const secondAttemptId = "conv-1:2";
      vi.mocked(mockDeps.env.getActiveAttemptId).mockReturnValue(
        secondAttemptId,
      );

      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-1",
        seq: 1,
        finishReason: "stop",
      });
      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-1",
        seq: 1,
        error: makeError("old error"),
      });
      expect(mockDeps.state.finishStream).not.toHaveBeenCalled();
      expect(mockDeps.state.errorStream).not.toHaveBeenCalled();

      orchestrator.handleDeltaEvent({
        conversationId: "conv-1",
        attemptId: secondAttemptId,
        messageId: "msg-2",
        seq: 1,
        finishReason: "stop",
      });
      expect(mockDeps.state.finishStream).toHaveBeenCalled();
    });
  });

  describe("onStreamStart", () => {
    it("delegates to state.setActiveAttemptId", () => {
      orchestrator.onStreamStart("conv-1:3", "msg-new");
      expect(mockDeps.state.setActiveAttemptId).toHaveBeenCalledWith(
        "conv-1:3",
        "msg-new",
      );
    });
  });

  describe("getActiveConversationId", () => {
    it("delegates to env.getActiveConversationId", () => {
      vi.mocked(mockDeps.env.getActiveConversationId).mockReturnValue(
        "conv-42",
      );
      expect(orchestrator.getActiveConversationId()).toBe("conv-42");
    });
  });
});
