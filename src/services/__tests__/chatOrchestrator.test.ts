import { describe, it, expect, vi, beforeEach } from "vitest";
import { createChatOrchestrator } from "../chatOrchestrator";
import type {
  ChatStatePort,
  ChatEnvPort,
  ChatOrchestratorDeps,
  ChatOrchestrator,
} from "../chatOrchestrator";
import type { TokenUsage } from "../../chat/chatTypes";
import { ChatErrorCode } from "../../chat/chatErrors";

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

describe("chatOrchestrator — 契约测试", () => {
  let orchestrator: ChatOrchestrator;
  let mockDeps: ReturnType<typeof createMockDeps>;

  beforeEach(() => {
    mockDeps = createMockDeps();
    orchestrator = createChatOrchestrator(mockDeps.deps);
  });

  describe("send", () => {
    it("calls beginSend with user + assistant messages and correct assistantMsgId", async () => {
      await orchestrator.send("hello");
      expect(mockDeps.state.beginSend).toHaveBeenCalled();
      const call = vi.mocked(mockDeps.state.beginSend).mock.calls[0];
      const messages = call[0];
      const assistantMsgId = call[1];
      expect(messages.length).toBe(2);
      expect(messages[0].role).toBe("user");
      expect(messages[0].content).toBe("hello");
      expect(messages[1].role).toBe("assistant");
      expect(messages[1].content).toBe("");
      expect(messages[1].id).toBe(assistantMsgId);
    });

    it("calls chatService.sendChatMessage with convId, content, modeId", async () => {
      const { chatService } = await import("../chatService");
      await orchestrator.send("test message");
      expect(chatService.sendChatMessage).toHaveBeenCalledWith(
        "conv-1",
        "test message",
        null,
      );
    });

    it("prevents concurrent sends (sendInProgress guard)", async () => {
      const { chatService } = await import("../chatService");
      vi.mocked(chatService.sendChatMessage).mockImplementation(
        () => new Promise((resolve) => setTimeout(resolve, 100)),
      );
      const p1 = orchestrator.send("first");
      const p2 = orchestrator.send("second");
      await Promise.all([p1, p2]);
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
      expect(mockDeps.state.cancelStreaming).toHaveBeenCalled();
    });

    it("creates conversation when no active conversation exists", async () => {
      const { chatService } = await import("../chatService");
      vi.mocked(mockDeps.env.getActiveConversationId)
        .mockReturnValueOnce(null)
        .mockReturnValueOnce("conv-new");
      await orchestrator.send("hello");
      expect(chatService.createConversation).toHaveBeenCalled();
      expect(mockDeps.deps.loadConversations).toHaveBeenCalled();
      expect(mockDeps.deps.switchConversation).toHaveBeenCalledWith("conv-new");
    });

    it("emits ApiKeyMissing error when no providers available", async () => {
      const { chatService } = await import("../chatService");
      vi.mocked(chatService.listProviders).mockResolvedValueOnce([]);
      vi.mocked(mockDeps.env.getActiveConversationId).mockReturnValue(null);
      await orchestrator.send("hello");
      expect(mockDeps.state.errorStream).toHaveBeenCalledWith(
        expect.objectContaining({ code: ChatErrorCode.ApiKeyMissing }),
      );
    });
  });

  describe("cancel", () => {
    it("calls chatService.cancelChat with active conversation id", async () => {
      const { chatService } = await import("../chatService");
      await orchestrator.cancel();
      expect(chatService.cancelChat).toHaveBeenCalledWith("conv-1");
    });

    it("calls cancelStreaming on state port", async () => {
      await orchestrator.cancel();
      expect(mockDeps.state.cancelStreaming).toHaveBeenCalled();
    });

    it("does nothing when no active conversation", async () => {
      const { chatService } = await import("../chatService");
      vi.mocked(mockDeps.env.getActiveConversationId).mockReturnValue(null);
      await orchestrator.cancel();
      expect(chatService.cancelChat).not.toHaveBeenCalled();
    });
  });

  describe("handleDeltaEvent", () => {
    it("routes delta_reasoning to appendReasoningDelta", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        message_id: "msg-1",
        delta_reasoning: "thinking...",
      });
      expect(mockDeps.state.setReasoningStreaming).toHaveBeenCalledWith(true);
      expect(mockDeps.state.appendReasoningDelta).toHaveBeenCalledWith(
        "msg-assistant",
        "thinking...",
      );
    });

    it("routes tool_call delta to appendToolCallDelta", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        message_id: "msg-1",
        delta_tool_call_id: "tc-1",
        delta_tool_call_name: "search",
        delta_tool_call_arguments: '{"q":',
      });
      expect(mockDeps.state.appendToolCallDelta).toHaveBeenCalledWith(
        "msg-assistant",
        "tc-1",
        "search",
        '{"q":',
      );
    });

    it("routes usage to updateContextUsage", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        message_id: "msg-1",
        usage: {
          prompt_tokens: 100,
          completion_tokens: 50,
          total_tokens: 150,
          context_window_tokens: 1000,
        },
      });
      expect(mockDeps.state.updateContextUsage).toHaveBeenCalledWith({
        used_tokens: 100,
        max_tokens: 1000,
        percentage: 0.1,
      });
    });

    it("routes finish_reason to finishStream", () => {
      const usage = {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
      } as TokenUsage;
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        message_id: "msg-1",
        finish_reason: "stop",
        usage,
      });
      expect(mockDeps.state.finishStream).toHaveBeenCalledWith(usage);
    });

    it("routes error to errorStream", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        message_id: "msg-1",
        error: "stream error",
      });
      expect(mockDeps.state.errorStream).toHaveBeenCalled();
    });

    it("skips reasoning/toolCall when no streamingMessageId", () => {
      vi.mocked(mockDeps.env.getStreamingMessageId).mockReturnValue(null);
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        message_id: "msg-1",
        delta_reasoning: "thinking...",
      });
      expect(mockDeps.state.appendReasoningDelta).not.toHaveBeenCalled();
    });
  });

  describe("onTextDelta", () => {
    it("calls setReasoningStreaming(false) and appendDelta", () => {
      orchestrator.onTextDelta("hello");
      expect(mockDeps.state.setReasoningStreaming).toHaveBeenCalledWith(false);
      expect(mockDeps.state.appendDelta).toHaveBeenCalledWith("hello");
    });

    it("transitions phase from sending to streaming", () => {
      expect(orchestrator.getPhase()).toBe("idle");
      orchestrator.onTextDelta("x");
      expect(orchestrator.getPhase()).toBe("idle");
    });
  });

  describe("ChatPhase state machine", () => {
    it("transitions idle → sending on send", async () => {
      expect(orchestrator.getPhase()).toBe("idle");
      const sendPromise = orchestrator.send("hello");
      expect(orchestrator.getPhase()).toBe("sending");
      await sendPromise;
    });

    it("transitions to error on error event", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        message_id: "msg-1",
        error: "fail",
      });
      expect(orchestrator.getPhase()).toBe("error");
    });

    it("transitions to idle on finish", () => {
      orchestrator.handleDeltaEvent({
        conversation_id: "conv-1",
        message_id: "msg-1",
        finish_reason: "stop",
      });
      expect(orchestrator.getPhase()).toBe("idle");
    });

    it("transitions to idle on cancel", async () => {
      await orchestrator.cancel();
      expect(orchestrator.getPhase()).toBe("idle");
    });
  });
});
