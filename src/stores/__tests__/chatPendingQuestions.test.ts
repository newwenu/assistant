import { describe, it, expect, beforeEach, vi } from "vitest";
import type { AgentToolCall, Message } from "../../chat/core/chatTypes";

vi.mock("../../services/chatService", () => ({
  chatService: {
    listConversations: vi.fn(() => Promise.resolve([])),
    createConversation: vi.fn(() => Promise.resolve("conv-1")),
    getConversationPaginated: vi.fn(() => Promise.resolve(null)),
    listPendingQuestions: vi.fn(() => Promise.resolve([])),
    getConversationTokenUsage: vi.fn(() => Promise.resolve(null)),
    deleteConversation: vi.fn(() => Promise.resolve()),
    renameConversation: vi.fn(() => Promise.resolve()),
    updateConversationModel: vi.fn(() => Promise.resolve()),
    sendChatMessage: vi.fn(() => Promise.resolve()),
    cancelChat: vi.fn(() => Promise.resolve()),
    incrementGeneration: vi.fn(() => Promise.resolve()),
    deleteMessagePersist: vi.fn(() => Promise.resolve()),
    editMessage: vi.fn(() => Promise.resolve()),
    manualCompactConversation: vi.fn(() => Promise.resolve()),
    unlockConversation: vi.fn(() => Promise.resolve()),
    toggleConversationPin: vi.fn(() => Promise.resolve()),
    duplicateConversation: vi.fn(() => Promise.resolve()),
    batchDeleteConversations: vi.fn(() => Promise.resolve()),
    purgeConversations: vi.fn(() => Promise.resolve()),
    switchBranch: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock("../../services/providerService", () => ({
  providerService: {
    listProviders: vi.fn(() => Promise.resolve([])),
  },
}));

vi.mock("../../services/logger", () => ({
  createLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock("../notifyStore", () => ({
  notifyError: vi.fn(),
  notifyWarning: vi.fn(),
  notifyInfo: vi.fn(),
  useNotifyStore: {
    getState: () => ({
      pending: [],
      notify: vi.fn(),
      dequeue: vi.fn(() => undefined),
    }),
    subscribe: () => () => {},
  },
}));

vi.mock("../agentModeStore", () => ({
  useAgentModeStore: {
    getState: () => ({
      selectedModeId: null,
      modes: [],
      selectMode: vi.fn(),
    }),
    subscribe: () => () => {},
  },
}));

import { useChatStore } from "../chatStore";
import { useChatStreamStore } from "../chatStreamStore";
import { chatService } from "../../services/chatService";

const MESSAGE_ID = "msg-1";
const CALL_ID = "call-1";

let seq = 0;
let convId = "";
let turnId = "";

function historyMessage(): Message {
  return {
    id: MESSAGE_ID,
    conversationId: convId,
    role: "assistant",
    content: "正在等待确认",
    tokens: null,
    timestamp: 1,
    parentId: "msg-user",
    isPinned: false,
    reasoning: null,
    siblingIndex: 0,
    siblingCount: null,
    providerId: null,
    modelId: null,
    toolCalls: [
      {
        callId: CALL_ID,
        name: "ask_user",
        status: "missing",
        resultText: null,
      },
    ],
  };
}

function detail() {
  return {
    conversation: { id: convId, activeBranchId: null, isFull: false },
    messages: [historyMessage()],
    totalCount: 1,
  };
}

function pendingQuestion(): AgentToolCall {
  return {
    conversationId: convId,
    turnId,
    iteration: 2,
    callId: CALL_ID,
    name: "ask_user",
    payload: {
      phase: "awaitingAnswer",
      question: {
        conversationId: convId,
        turnId,
        callId: CALL_ID,
        timeoutMs: 60_000,
        questions: [
          {
            question: "继续吗？",
            header: "确认",
            multiSelect: false,
            options: [{ label: "继续", description: "" }],
          },
        ],
      },
    },
  };
}

describe("switchConversation — 挂起提问补拉", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    /** 每次用新会话 id：orchestrator 的轮次状态按会话缓存，复用 id 会串测。 */
    seq += 1;
    convId = `conv-${seq}`;
    turnId = `${convId}:1`;
    useChatStreamStore.getState().cancelStreaming();
    useChatStore.setState({
      currentConversationId: null,
      messages: [],
      contextUsage: { usedTokens: 0, hasUsage: false },
    });
  });

  it("切回会话时把等待期的残卡重建为可作答的提问卡", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail() as never,
    );
    vi.mocked(chatService.listPendingQuestions).mockResolvedValue([
      pendingQuestion(),
    ]);

    await useChatStore.getState().switchConversation(convId);

    const view = useChatStore.getState().messages[0].toolCalls?.[0];
    expect(view?.status).toBe("awaiting_user");
    expect(view?.question?.timeoutMs).toBe(60_000);
    expect(vi.mocked(chatService.listPendingQuestions)).toHaveBeenCalledWith(
      convId,
    );
  });

  it("快照同时恢复流式视图，作答后的事件不会落到占位气泡", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail() as never,
    );
    vi.mocked(chatService.listPendingQuestions).mockResolvedValue([
      pendingQuestion(),
    ]);

    await useChatStore.getState().switchConversation(convId);

    const stream = useChatStreamStore.getState();
    expect(stream.isStreaming).toBe(true);
    expect(stream.activeAttemptId).toBe(turnId);
    expect(stream.streamingMessageId).toBe(MESSAGE_ID);
  });

  it("无挂起提问时保持空闲，不误判为运行中", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail() as never,
    );
    vi.mocked(chatService.listPendingQuestions).mockResolvedValue([]);

    await useChatStore.getState().switchConversation(convId);

    expect(useChatStore.getState().messages[0].toolCalls?.[0]?.status).toBe(
      "missing",
    );
    expect(useChatStreamStore.getState().isStreaming).toBe(false);
    expect(useChatStreamStore.getState().activeAttemptId).toBeNull();
  });

  it("补拉失败退化为无挂起提问，历史照常加载", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail() as never,
    );
    vi.mocked(chatService.listPendingQuestions).mockRejectedValue(
      new Error("boom"),
    );

    await useChatStore.getState().switchConversation(convId);

    expect(useChatStore.getState().messages).toHaveLength(1);
    expect(useChatStore.getState().messages[0].toolCalls?.[0]?.status).toBe(
      "missing",
    );
  });
});
