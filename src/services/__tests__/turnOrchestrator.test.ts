import { describe, it, expect, vi, beforeEach } from "vitest";
import { createTurnOrchestrator } from "../stream/turnOrchestrator";
import type {
  TurnOrchestratorDeps,
  TurnOrchestrator,
} from "../stream/turnOrchestrator";
import type { StreamStatePort } from "../stream/turnStrategy";
import type { TurnEnvPort } from "../stream/turnOrchestrator";
import { createChatTurnStrategy } from "../stream/strategies/chatTurn";
import { createAgentTurnStrategy } from "../stream/strategies/agentTurn";
import type { AgentSegments } from "../../stores/agent/agentSegments";
import { chatService } from "../chatService";
import type { ReasoningChoice } from "../../chat/core/reasoning";
import type {
  ChatDeltaPayload,
  Message,
  TokenUsage,
} from "../../chat/core/chatTypes";
import type { ProviderErrorPayloadDto } from "../../types/protocol.contract";
import {
  makeConversationRoute,
  makeMessage,
} from "../../test-utils/messageFixtures";

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

function createMockStatePort(): StreamStatePort {
  return {
    beginSend: vi.fn(),
    cancelStreaming: vi.fn(),
    setActiveAttemptId: vi.fn(),
    clearActiveAttemptId: vi.fn(),
    resumeStream: vi.fn(),
    confirmUserMessage: vi.fn(),
    appendDelta: vi.fn(),
    setReasoningStreaming: vi.fn(),
    appendReasoningDelta: vi.fn(),
    updateContextUsage: vi.fn(),
    finishStream: vi.fn(),
    errorStream: vi.fn(),
    setErrorOnMessage: vi.fn(),
    markMessageInterrupted: vi.fn(),
    setRetryStatus: vi.fn(),
  };
}

function createMockSegments(): AgentSegments {
  return {
    reset: vi.fn(),
    markUnknownSegment: vi.fn(),
    adoptSegment: vi.fn(),
    renameSegment: vi.fn(),
    appendDelta: vi.fn(),
    appendReasoningDelta: vi.fn(),
    applyToolCall: vi.fn(),
    commitSegment: vi.fn(),
    settleToolCalls: vi.fn(),
  };
}

function createMockDeps(): {
  deps: TurnOrchestratorDeps;
  state: StreamStatePort;
  env: TurnEnvPort;
  segments: AgentSegments;
} {
  const state = createMockStatePort();
  const segments = createMockSegments();

  const env: TurnEnvPort = {
    getActiveConversationId: vi.fn(() => "conv-1"),
    getIsStreaming: vi.fn(() => false),
    getStreamingMessageId: vi.fn(() => "msg-assistant"),
    getActiveAttemptId: vi.fn(() => "conv-1:1"),
    getMessages: vi.fn(() => []),
    getConversations: vi.fn(() => []),
    getConversationRoute: vi.fn(() => makeConversationRoute()),
    getReasoningChoice: vi.fn<() => ReasoningChoice>(() => "inherit"),
    getSelectedProviderId: vi.fn(() => "provider-1"),
    getSelectedModelId: vi.fn(() => "model-1"),
    getPendingBranchPoint: vi.fn(() => null),
    clearPendingBranchPoint: vi.fn(),
  };

  const deps: TurnOrchestratorDeps = {
    state,
    env,
    strategies: {
      chat: createChatTurnStrategy(state),
      agent: createAgentTurnStrategy(state, segments),
    },
    dispatchSend: (conversationId, args, kind) =>
      kind === "agent"
        ? chatService.sendAgentMessage(conversationId, args)
        : chatService.sendChatMessage(conversationId, args),
    cancelTurn: (conversationId) =>
      env.getConversationRoute(conversationId).link === "agent"
        ? chatService.cancelAgent(conversationId)
        : chatService.cancelChat(conversationId),
    listProviders: vi.fn(() =>
      Promise.resolve([
        {
          id: "p1",
          name: "provider-1",
          type: "openai",
          baseUrl: "https://example.test",
          apiKeyMasked: "",
          models: [],
          defaultModel: "m1",
          updatedAt: 0,
          effortSupport: [],
        },
      ]),
    ),
    createConversation: (providerId, modelId, modeId) =>
      chatService.createConversation(providerId, modelId, modeId),
    loadConversations: vi.fn(() => Promise.resolve()),
    switchConversation: vi.fn(() => Promise.resolve()),
    renameConversation: vi.fn(() => Promise.resolve()),
  };

  return { deps, state, env, segments };
}

describe("turnOrchestrator", () => {
  let orchestrator: TurnOrchestrator;
  let mockDeps: ReturnType<typeof createMockDeps>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockDeps = createMockDeps();
    orchestrator = createTurnOrchestrator(mockDeps.deps);
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
      vi.mocked(mockDeps.env.getIsStreaming).mockReturnValue(true);
      vi.mocked(mockDeps.env.getActiveConversationId).mockReturnValue(
        "conv-prev",
      );
      await orchestrator.send("hello");
      expect(chatService.cancelChat).toHaveBeenCalledWith("conv-prev");
    });

    it("agent 会话发送前取消走 cancelAgent", async () => {
      vi.mocked(mockDeps.env.getIsStreaming).mockReturnValue(true);
      vi.mocked(mockDeps.env.getActiveConversationId).mockReturnValue(
        "conv-prev",
      );
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue({
        link: "agent",
        modeId: "mode-custom",
        toolConfig: null,
      });
      await orchestrator.send("hello");
      expect(chatService.cancelAgent).toHaveBeenCalledWith("conv-prev");
      expect(chatService.cancelChat).not.toHaveBeenCalled();
    });
  });

  describe("send — link routing (I38d §4.8)", () => {
    it("agent 会话走 send_agent_message，并下发 mode 派生的工具配置", async () => {
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
    it("chat：deltaReasoning → state.appendReasoningDelta（单缓冲）", () => {
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
        "thinking...",
      );
      expect(mockDeps.segments.appendReasoningDelta).not.toHaveBeenCalled();
    });

    it("agent：deltaReasoning 按 iteration 归属到段（§3.1.1）", () => {
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue(
        makeConversationRoute({ link: "agent" }),
      );
      orchestrator.handleDeltaEvent(
        makeDelta({ iteration: 2, deltaReasoning: "thinking..." }),
      );
      expect(mockDeps.segments.appendReasoningDelta).toHaveBeenCalledWith(
        2,
        "thinking...",
      );
      expect(mockDeps.state.appendReasoningDelta).not.toHaveBeenCalled();
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

    it("chat：finishReason → finishStream with business TokenUsage", () => {
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

    it("agent 会话的 finishReason 只提交本段，不结束 turn（§3.1.1）", () => {
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue(
        makeConversationRoute({ link: "agent" }),
      );
      orchestrator.handleDeltaEvent(
        makeDelta({
          conversationId: "conv-1",
          attemptId: "conv-1:1",
          messageId: "msg-assistant",
          seq: 1,
          iteration: 1,
          finishReason: "tool_calls",
        }),
      );
      expect(mockDeps.segments.commitSegment).toHaveBeenCalledTimes(1);
      expect(mockDeps.state.finishStream).not.toHaveBeenCalled();
      expect(mockDeps.segments.settleToolCalls).not.toHaveBeenCalled();
    });
  });

  describe("onAgentToolCall — 按迭代归属", () => {
    it("工具调用事件按 iteration 交给 agent 策略（气泡边界 = 迭代）", () => {
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue(
        makeConversationRoute({ link: "agent" }),
      );
      orchestrator.onAgentToolCall({
        conversationId: "conv-1",
        turnId: "conv-1:1",
        iteration: 2,
        callId: "call-1",
        name: "read_file",
        payload: { phase: "started" },
      });
      expect(mockDeps.segments.applyToolCall).toHaveBeenCalledWith(
        2,
        expect.objectContaining({ callId: "call-1" }),
      );
    });

    it("旧轮 turnId 的迟到事件被丢弃", () => {
      orchestrator.onAgentToolCall({
        conversationId: "conv-1",
        turnId: "conv-1:0",
        iteration: 1,
        callId: "call-1",
        name: "read_file",
        payload: { phase: "started" },
      });
      expect(mockDeps.segments.applyToolCall).not.toHaveBeenCalled();
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

  describe("session-state → 策略收口", () => {
    it("agent 会话 idle 时收敛工具终态并结束流式", () => {
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue(
        makeConversationRoute({ link: "agent" }),
      );
      vi.mocked(mockDeps.env.getIsStreaming).mockReturnValue(true);
      orchestrator.onSessionState({
        conversationId: "conv-1",
        turnId: "conv-1:1",
        state: "idle",
      });
      expect(mockDeps.segments.settleToolCalls).toHaveBeenCalled();
      expect(mockDeps.state.finishStream).toHaveBeenCalled();
    });

    it("chat 会话 idle 只结束流式，不触碰段表", () => {
      vi.mocked(mockDeps.env.getIsStreaming).mockReturnValue(true);
      orchestrator.onSessionState({
        conversationId: "conv-1",
        turnId: "conv-1:1",
        state: "idle",
      });
      expect(mockDeps.state.finishStream).toHaveBeenCalled();
      expect(mockDeps.segments.settleToolCalls).not.toHaveBeenCalled();
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

    it("agent 会话切回时标记段未知，避免覆盖已落库正文", () => {
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue(
        makeConversationRoute({ link: "agent" }),
      );
      orchestrator.onSessionState({
        conversationId: "conv-2",
        turnId: "conv-2:1",
        state: { processing: { turnId: "conv-2:1", phase: "streaming" } },
      });
      orchestrator.onConversationSwitch("conv-2");
      expect(mockDeps.segments.markUnknownSegment).toHaveBeenCalled();
    });

    it("切会话不触发任何取消命令", () => {
      orchestrator.onConversationSwitch("conv-2");
      expect(chatService.cancelChat).not.toHaveBeenCalled();
      expect(chatService.cancelAgent).not.toHaveBeenCalled();
    });
  });

  describe("onPendingToolCalls — 补拉快照回填", () => {
    it("窗口重载后 phase 为空：用快照恢复运行态并绑定历史行", () => {
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue(
        makeConversationRoute({ link: "agent" }),
      );
      orchestrator.onPendingToolCalls("conv-1", "conv-1:1", "msg-1", 2);
      expect(orchestrator.getPhase()).toBe("tool_executing");
      expect(mockDeps.state.resumeStream).toHaveBeenCalledWith(
        "conv-1:1",
        "msg-1",
      );
      expect(mockDeps.segments.adoptSegment).toHaveBeenCalledWith(2, "msg-1");
      expect(mockDeps.segments.markUnknownSegment).not.toHaveBeenCalled();
    });

    it("历史没加载到发起行：仍恢复流式视图，段表退回未知", () => {
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue(
        makeConversationRoute({ link: "agent" }),
      );
      orchestrator.onPendingToolCalls("conv-1", "conv-1:1", null, 2);
      expect(mockDeps.state.resumeStream).toHaveBeenCalledWith(
        "conv-1:1",
        null,
      );
      expect(mockDeps.segments.markUnknownSegment).toHaveBeenCalled();
      expect(mockDeps.segments.adoptSegment).not.toHaveBeenCalled();
    });

    it("非当前会话：只记录轮次状态，不驱动 UI", () => {
      vi.mocked(mockDeps.env.getActiveConversationId).mockReturnValue("conv-2");
      orchestrator.onPendingToolCalls("conv-1", "conv-1:1", "msg-1", 2);
      expect(orchestrator.getPhase()).toBe("idle");
      expect(mockDeps.state.resumeStream).not.toHaveBeenCalled();
    });

    it("已有 phase 时不覆盖（快照只补未知状态）", () => {
      orchestrator.onSessionState({
        conversationId: "conv-1",
        turnId: "conv-1:1",
        state: { processing: { turnId: "conv-1:1", phase: "streaming" } },
      });
      orchestrator.onPendingToolCalls("conv-1", "conv-1:1", "msg-1", 2);
      expect(orchestrator.getPhase()).toBe("streaming");
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
    it("chat 会话的文本增量写入单缓冲", () => {
      orchestrator.onTextDelta({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-1",
        iteration: 1,
        delta: "hi",
      });
      expect(mockDeps.state.appendDelta).toHaveBeenCalledWith("hi");
      expect(mockDeps.segments.appendDelta).not.toHaveBeenCalled();
    });

    it("agent 会话的文本增量按迭代归属到段（§3.1.1）", () => {
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue(
        makeConversationRoute({ link: "agent" }),
      );
      orchestrator.onTextDelta({
        conversationId: "conv-1",
        attemptId: "conv-1:1",
        messageId: "msg-1",
        iteration: 3,
        delta: "third",
      });
      expect(mockDeps.segments.appendDelta).toHaveBeenCalledWith(3, "third");
      expect(mockDeps.state.appendDelta).not.toHaveBeenCalled();
    });

    it("后台会话的文本增量不污染当前会话", () => {
      orchestrator.onTextDelta({
        conversationId: "conv-other",
        attemptId: "conv-other:1",
        messageId: "msg-other",
        iteration: 1,
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
        iteration: 1,
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

    it("占位 id 换成真实 id 时通知策略改名", () => {
      vi.mocked(mockDeps.env.getConversationRoute).mockReturnValue(
        makeConversationRoute({ link: "agent" }),
      );
      orchestrator.onStreamStart("conv-1", "conv-1:3", "msg-new");
      expect(mockDeps.segments.renameSegment).toHaveBeenCalledWith(
        "msg-assistant",
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

  describe("cancel — 压缩占位气泡（I41 §1）", () => {
    const compactSummary: Message = makeMessage("temp-compact-1", {
      role: "compact_summary",
    });

    it("压缩流取消不在临时摘要气泡上写 interrupted", async () => {
      vi.mocked(mockDeps.env.getStreamingMessageId).mockReturnValue(
        "temp-compact-1",
      );
      vi.mocked(mockDeps.env.getMessages).mockReturnValue([compactSummary]);

      await orchestrator.cancel();

      expect(mockDeps.state.markMessageInterrupted).not.toHaveBeenCalled();
    });

    it("普通助手流取消仍写 interrupted", async () => {
      vi.mocked(mockDeps.env.getStreamingMessageId).mockReturnValue(
        "msg-assistant",
      );

      await orchestrator.cancel();

      expect(mockDeps.state.markMessageInterrupted).toHaveBeenCalledWith(
        "msg-assistant",
      );
    });
  });
});
