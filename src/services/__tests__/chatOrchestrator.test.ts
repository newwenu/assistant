import { describe, it, expect, vi, beforeEach } from "vitest";
import { createChatOrchestrator } from "../chatOrchestrator";
import type {
  ChatStatePort,
  ChatEnvPort,
  ChatOrchestratorDeps,
  ChatOrchestrator,
} from "../chatOrchestrator";
import type { ReasoningChoice } from "../../chat/core/reasoning";
import type { ChatDeltaPayload, TokenUsage } from "../../chat/core/chatTypes";
import type { ProviderErrorPayloadDto } from "../../types/protocol.contract";

function makeProviderError(message: string): ProviderErrorPayloadDto {
  return {
    code: "StreamError",
    message,
    retryAfterMs: null,
    retryable: false,
    httpStatus: null,
    provider: null,
    providerCode: null,
  };
}

function makeUsage(partial: Partial<TokenUsage>): TokenUsage {
  return {
    promptTokens: 0,
    completionTokens: 0,
    totalTokens: 0,
    cacheReadTokens: null,
    cacheWriteTokens: null,
    reasoningTokens: null,
    contextWindowTokens: null,
    ...partial,
  };
}

type DeltaInput = Partial<Omit<ChatDeltaPayload, "usage">> & {
  usage?: Partial<TokenUsage> | null;
};

function makeDelta(input: DeltaInput = {}): ChatDeltaPayload {
  return {
    conversationId: "conv-1",
    attemptId: "conv-1:1",
    messageId: "msg-1",
    seq: 1,
    iteration: 1,
    deltaText: null,
    deltaReasoning: null,
    finishReason: null,
    error: null,
    deltaRetry: null,
    ...input,
    usage: input.usage ? makeUsage(input.usage) : null,
  };
}

vi.mock("../chatService", () => ({
  chatService: {
    listProviders: vi.fn(() =>
      Promise.resolve([{ id: "p1", defaultModel: "m1" }]),
    ),
    createConversation: vi.fn(() => Promise.resolve("conv-new")),
    sendChatMessage: vi.fn(() => Promise.resolve()),
    sendAgentMessage: vi.fn(() => Promise.resolve()),
    cancelChat: vi.fn(() => Promise.resolve()),
    cancelAgent: vi.fn(() => Promise.resolve()),
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
    resumeStream: vi.fn(),
    confirmUserMessage: vi.fn(),
    appendDelta: vi.fn(),
    setReasoningStreaming: vi.fn(),
    appendReasoningDelta: vi.fn(),
    applyToolCall: vi.fn(),
    settleToolCalls: vi.fn(),
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
    getConversationRoute: vi.fn(() => ({
      link: "chat" as const,
      modeId: null,
      toolConfig: null,
    })),
    getReasoningChoice: vi.fn<() => ReasoningChoice>(() => "inherit"),
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

  describe("send — link routing (I38d §4.8)", () => {
    it("agent 会话走 send_agent_message，并下发 mode 派生的工具配置", async () => {
      const { chatService } = await import("../chatService");
      const toolConfig = {
        maxToolTier: "os" as const,
        approvalLevel: "dangerous_only" as const,
        enabledTools: ["read", "write"],
        disabledTools: [],
      };
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue({
        link: "agent",
        modeId: "mode-custom",
        toolConfig,
      });
      await orchestrator.send("hello");
      expect(chatService.sendChatMessage).not.toHaveBeenCalled();
      expect(chatService.sendAgentMessage).toHaveBeenCalledTimes(1);
      const [, args] = vi.mocked(chatService.sendAgentMessage).mock.calls[0];
      expect(args.modeId).toBe("mode-custom");
      expect(args.toolConfig).toEqual(toolConfig);
    });

    it("chat 会话走 send_chat_message，modeId 取自会话路由", async () => {
      const { chatService } = await import("../chatService");
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue({
        link: "chat",
        modeId: "builtin-translate",
        toolConfig: null,
      });
      await orchestrator.send("hello");
      expect(chatService.sendAgentMessage).not.toHaveBeenCalled();
      const [, args] = vi.mocked(chatService.sendChatMessage).mock.calls[0];
      expect(args.modeId).toBe("builtin-translate");
      expect(args.toolConfig).toBeNull();
    });

    it("agent 会话取消走 cancel_agent", async () => {
      const { chatService } = await import("../chatService");
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue({
        link: "agent",
        modeId: "mode-custom",
        toolConfig: null,
      });
      await orchestrator.cancel();
      expect(chatService.cancelChat).not.toHaveBeenCalled();
      expect(chatService.cancelAgent).toHaveBeenCalledWith("conv-1");
    });
  });

  describe("handleDeltaEvent — routing", () => {
    it("routes deltaReasoning → appendReasoningDelta", () => {
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: "conv-1:1",
          messageId: "msg-1",
          seq: 1,
          deltaReasoning: "thinking...",
        }),
      );
      expect(mockDeps.state.appendReasoningDelta).toHaveBeenCalledWith(
        "msg-assistant",
        "thinking...",
      );
    });

    it("routes usage → updateContextUsage (DTO→business conversion)", () => {
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: "conv-1:1",
          messageId: "msg-1",
          seq: 1,
          usage: {
            promptTokens: 100,
            completionTokens: 50,
            totalTokens: 150,
            contextWindowTokens: 1000,
          },
        }),
      );
      expect(mockDeps.state.updateContextUsage).toHaveBeenCalledWith({
        usedTokens: 100,
        hasUsage: true,
        maxTokens: 1000,
        percentage: 0.1,
        completionTokens: 50,
        cacheReadTokens: undefined,
        cacheWriteTokens: undefined,
        reasoningTokens: undefined,
      });
    });

    it("routes finishReason → finishStream with business TokenUsage", () => {
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: "conv-1:1",
          messageId: "msg-assistant",
          seq: 1,
          finishReason: "stop",
          usage: {
            promptTokens: 100,
            completionTokens: 50,
            totalTokens: 150,
          },
        }),
      );
      expect(mockDeps.state.finishStream).toHaveBeenCalledWith({
        promptTokens: 100,
        completionTokens: 50,
        totalTokens: 150,
        cacheReadTokens: null,
        cacheWriteTokens: null,
        reasoningTokens: null,
        contextWindowTokens: null,
      });
    });
  });

  describe("handleDeltaEvent — stale attemptId filtering", () => {
    it("ignores all event types from stale attemptId", () => {
      const staleId = "conv-1:0";
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: staleId,
          messageId: "msg-1",
          seq: 1,
          deltaReasoning: "stale",
        }),
      );
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: staleId,
          messageId: "msg-1",
          seq: 1,
          usage: {
            promptTokens: 100,
            completionTokens: 50,
            totalTokens: 150,
            contextWindowTokens: 1000,
          },
        }),
      );
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: staleId,
          messageId: "msg-1",
          seq: 1,
          error: makeProviderError("stale error"),
        }),
      );
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: staleId,
          messageId: "msg-1",
          seq: 1,
          finishReason: "stop",
        }),
      );
      expect(mockDeps.state.appendReasoningDelta).not.toHaveBeenCalled();
      expect(mockDeps.state.updateContextUsage).not.toHaveBeenCalled();
      expect(mockDeps.state.errorStream).not.toHaveBeenCalled();
      expect(mockDeps.state.finishStream).not.toHaveBeenCalled();
    });

    it("processes events from current attemptId after stale ones", () => {
      const currentId = "conv-1:2";
      vi.mocked(mockDeps.env.getActiveAttemptId).mockReturnValue(currentId);

      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: currentId,
          messageId: "msg-new",
          seq: 1,
          deltaReasoning: "fresh reasoning",
        }),
      );
      expect(mockDeps.state.appendReasoningDelta).toHaveBeenCalledWith(
        "msg-assistant",
        "fresh reasoning",
      );

      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: currentId,
          messageId: "msg-new",
          seq: 1,
          usage: {
            promptTokens: 200,
            completionTokens: 80,
            totalTokens: 280,
            contextWindowTokens: 2000,
          },
        }),
      );
      expect(mockDeps.state.updateContextUsage).toHaveBeenCalledWith({
        usedTokens: 200,
        hasUsage: true,
        maxTokens: 2000,
        percentage: 0.1,
        completionTokens: 80,
        cacheReadTokens: undefined,
        cacheWriteTokens: undefined,
        reasoningTokens: undefined,
      });
    });

    it("filters events when activeAttemptId is null", () => {
      vi.mocked(mockDeps.env.getActiveAttemptId).mockReturnValue(null);
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: "conv-1:1",
          messageId: "msg-1",
          seq: 1,
          error: makeProviderError("cancelled"),
        }),
      );
      expect(mockDeps.state.errorStream).not.toHaveBeenCalled();
    });
  });

  describe("ChatPhase — 由 session-state-changed 驱动", () => {
    it("phase 在 send 期间保持 idle，不再由前端自行推导", async () => {
      expect(orchestrator.getPhase()).toBe("idle");
      await orchestrator.send("hello");
      expect(orchestrator.getPhase()).toBe("idle");
    });

    it("processing(starting) → sending，idle → idle", () => {
      orchestrator.onSessionState({
        conversationId: "conv-1",
        turnId: "conv-1:1",
        state: { processing: { turnId: "conv-1:1", phase: "starting" } },
      });
      expect(orchestrator.getPhase()).toBe("sending");

      orchestrator.onSessionState({
        conversationId: "conv-1",
        turnId: "conv-1:1",
        state: "idle",
      });
      expect(orchestrator.getPhase()).toBe("idle");
    });

    it("error 态映射为 error", () => {
      orchestrator.onSessionState({
        conversationId: "conv-1",
        turnId: "conv-1:1",
        state: { error: { message: "boom", recoverable: false } },
      });
      expect(orchestrator.getPhase()).toBe("error");
    });

    it("后台会话的状态不影响当前会话 phase", () => {
      orchestrator.onSessionState({
        conversationId: "conv-other",
        turnId: "conv-other:1",
        state: { processing: { turnId: "conv-other:1", phase: "streaming" } },
      });
      expect(orchestrator.getPhase()).toBe("idle");
    });
  });

  describe("onConversationSwitch — 不取消后台 turn", () => {
    it("切到空闲会话：phase 归 idle 并清空本地流式视图", () => {
      vi.mocked(mockDeps.env.getIsStreaming).mockReturnValue(true);
      orchestrator.onConversationSwitch("conv-2");
      expect(orchestrator.getPhase()).toBe("idle");
      expect(mockDeps.state.cancelStreaming).toHaveBeenCalled();
      expect(mockDeps.state.resumeStream).not.toHaveBeenCalled();
    });

    it("切到仍在运行的会话：恢复流式视图且 messageId 未知", () => {
      orchestrator.onSessionState({
        conversationId: "conv-2",
        turnId: "conv-2:1",
        state: { processing: { turnId: "conv-2:1", phase: "streaming" } },
      });
      orchestrator.onConversationSwitch("conv-2");
      expect(orchestrator.getPhase()).toBe("streaming");
      expect(mockDeps.state.resumeStream).toHaveBeenCalledWith(
        "conv-2:1",
        null,
      );
    });

    it("切会话不触发任何取消命令", async () => {
      const { chatService } = await import("../chatService");
      orchestrator.onConversationSwitch("conv-2");
      expect(chatService.cancelChat).not.toHaveBeenCalled();
      expect(chatService.cancelAgent).not.toHaveBeenCalled();
    });
  });

  describe("handleDeltaEvent — 会话路由", () => {
    it("非当前会话的增量被丢弃", () => {
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-other",
          attemptId: "conv-1:1",
          messageId: "msg-1",
          seq: 1,
          deltaReasoning: "other",
        }),
      );
      expect(mockDeps.state.appendReasoningDelta).not.toHaveBeenCalled();
    });
  });

  describe("onTextDelta — 会话路由", () => {
    it("当前会话的文本增量写入流式缓冲", () => {
      orchestrator.onTextDelta({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-1",
        delta: "hi",
      });
      expect(mockDeps.state.appendDelta).toHaveBeenCalledWith("hi");
    });

    it("后台会话的文本增量不污染当前会话", () => {
      orchestrator.onTextDelta({
        conversationId: "conv-other",
        attemptId: "conv-other:1",
        messageId: "msg-other",
        delta: "bg",
      });
      expect(mockDeps.state.appendDelta).not.toHaveBeenCalled();
    });

    it("切回运行中的会话后由首个增量补全 messageId", () => {
      vi.mocked(mockDeps.env.getStreamingMessageId).mockReturnValue(null);
      orchestrator.onTextDelta({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-new",
        delta: "hi",
      });
      expect(mockDeps.state.setActiveAttemptId).toHaveBeenCalledWith(
        "conv-1:1",
        "msg-new",
      );
      expect(mockDeps.state.appendDelta).toHaveBeenCalledWith("hi");
    });
  });

  describe("attemptId — editMessage 重发", () => {
    it("旧 attemptId 的 finish/error 被忽略，新的正常处理", () => {
      const secondAttemptId = "conv-1:2";
      vi.mocked(mockDeps.env.getActiveAttemptId).mockReturnValue(
        secondAttemptId,
      );

      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: "conv-1:1",
          messageId: "msg-1",
          seq: 1,
          finishReason: "stop",
        }),
      );
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: "conv-1:1",
          messageId: "msg-1",
          seq: 1,
          error: makeProviderError("old error"),
        }),
      );
      expect(mockDeps.state.finishStream).not.toHaveBeenCalled();
      expect(mockDeps.state.errorStream).not.toHaveBeenCalled();

      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: secondAttemptId,
          messageId: "msg-2",
          seq: 1,
          finishReason: "stop",
        }),
      );
      expect(mockDeps.state.finishStream).toHaveBeenCalled();
    });
  });

  describe("onStreamStart", () => {
    it("当前会话委托 state.setActiveAttemptId，并按会话记录 turnId", () => {
      orchestrator.onStreamStart("conv-1", "conv-1:3", "msg-new");
      expect(mockDeps.state.setActiveAttemptId).toHaveBeenCalledWith(
        "conv-1:3",
        "msg-new",
      );
    });

    it("后台会话的 stream-start 不改变当前会话的流式视图", () => {
      orchestrator.onStreamStart("conv-other", "conv-other:1", "msg-other");
      expect(mockDeps.state.setActiveAttemptId).not.toHaveBeenCalled();
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
