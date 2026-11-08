import { describe, it, expect, beforeEach, vi } from "vitest";
import type {
  TokenUsageCacheSummary,
  TokenUsageCumulativeSummary,
  TokenUsageOccupancySummary,
  TokenUsageSummary,
} from "../../chat/core/chatTypes";

type SummaryInput = {
  occupancy?: Partial<TokenUsageOccupancySummary>;
  cumulative?: TokenUsageCumulativeSummary | null;
  cache?: TokenUsageCacheSummary | null;
};

function summary(input: SummaryInput = {}): TokenUsageSummary {
  return {
    cumulative: null,
    cache: null,
    ...input,
    occupancy: {
      usedTokens: null,
      completionTokens: null,
      cacheReadTokens: null,
      cacheWriteTokens: null,
      reasoningTokens: null,
      contextWindowTokens: null,
      percentage: null,
      estimated: false,
      estimatedSource: null,
      ...input.occupancy,
    },
  };
}

vi.mock("../../services/chatService", () => ({
  chatService: {
    listConversations: vi.fn(() => Promise.resolve([])),
    listProviders: vi.fn(() => Promise.resolve([])),
    createConversation: vi.fn(() => Promise.resolve("conv-1")),
    getConversationPaginated: vi.fn(() => Promise.resolve(null)),
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

vi.mock("../providerStore", () => ({
  useProviderStore: {
    getState: () => ({ providers: [] }),
    subscribe: () => () => {},
  },
}));

vi.mock("../../utils/persistQueue", () => ({
  PersistQueue: class {
    enqueue = (fn: () => Promise<void>) => fn();
  },
}));

import { useChatStore } from "../chatStore";
import { useChatStreamStore } from "../chatStreamStore";
import { createStatePort } from "../chatStorePorts";
import { chatService } from "../../services/chatService";

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

function detail(id: string) {
  return {
    conversation: {
      id,
      activeBranchId: null,
      isFull: false,
    },
    messages: [],
    totalCount: 0,
  };
}

describe("context usage across conversation switches", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useChatStreamStore.setState({
      isStreaming: false,
      streamingContent: "",
      streamingMessageId: null,
      activeAttemptId: null,
      isReasoningStreaming: false,
      retryStatus: null,
    });
    useChatStore.setState({
      currentConversationId: null,
      messages: [],
      contextUsage: { usedTokens: 0, hasUsage: false },
    });
  });

  it("starts empty then adopts the stored occupancy of the opened conversation", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail("c1") as never,
    );
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({
        occupancy: {
          usedTokens: 12_000,
          contextWindowTokens: 128_000,
          percentage: 12_000 / 128_000,
        },
        cumulative: {
          totalTokens: 30_000,
          promptTokens: 20_000,
          completionTokens: 10_000,
        },
      }),
    );

    await useChatStore.getState().switchConversation("c1");
    await flush();
    const usage = useChatStore.getState().contextUsage;
    expect(usage.hasUsage).toBe(true);
    expect(usage.usedTokens).toBe(12_000);
    expect(usage.maxTokens).toBe(128_000);
    expect(usage.cumulativeTokens).toBe(30_000);
  });

  it("keeps the same occupancy when switching back to the same conversation", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail("c1") as never,
    );
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({
        occupancy: {
          usedTokens: 12_000,
          contextWindowTokens: 128_000,
          percentage: 12_000 / 128_000,
        },
        cumulative: {
          totalTokens: 30_000,
          promptTokens: 20_000,
          completionTokens: 10_000,
        },
      }),
    );

    await useChatStore.getState().switchConversation("c1");
    await flush();
    expect(useChatStore.getState().contextUsage.usedTokens).toBe(12_000);

    await useChatStore.getState().switchConversation("c1");
    await flush();
    expect(useChatStore.getState().contextUsage.usedTokens).toBe(12_000);
  });

  it("refreshes occupancy after a branch switch", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail("c1") as never,
    );
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({
        occupancy: { usedTokens: 12_000, contextWindowTokens: 128_000 },
        cumulative: {
          totalTokens: 30_000,
          promptTokens: 20_000,
          completionTokens: 10_000,
        },
      }),
    );

    await useChatStore.getState().switchConversation("c1");
    await flush();
    expect(useChatStore.getState().contextUsage.usedTokens).toBe(12_000);

    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({
        occupancy: { usedTokens: 7_000, contextWindowTokens: 128_000 },
        cumulative: {
          totalTokens: 30_000,
          promptTokens: 20_000,
          completionTokens: 10_000,
        },
      }),
    );

    useChatStore.getState().refreshTokenUsage();
    await flush();

    const usage = useChatStore.getState().contextUsage;
    expect(usage.usedTokens).toBe(7_000);
    expect(usage.cumulativeTokens).toBe(30_000);
  });

  it("shows the absolute number when the window is unknown", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail("c1") as never,
    );
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({
        occupancy: { usedTokens: 12_000 },
        cumulative: {
          totalTokens: 30_000,
          promptTokens: 20_000,
          completionTokens: 10_000,
        },
      }),
    );

    await useChatStore.getState().switchConversation("c1");
    await flush();

    const usage = useChatStore.getState().contextUsage;
    expect(usage.usedTokens).toBe(12_000);
    expect(usage.maxTokens).toBeUndefined();
    expect(usage.percentage).toBeUndefined();
  });

  it("shows no usage when the conversation has no records", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail("c1") as never,
    );
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({
        occupancy: {},
      }),
    );

    await useChatStore.getState().switchConversation("c1");
    await flush();

    const usage = useChatStore.getState().contextUsage;
    expect(usage.hasUsage).toBe(false);
    expect(usage.usedTokens).toBe(0);
    expect(usage.cumulativeTokens).toBeUndefined();
  });

  it("keeps the window for an empty branch so the bar shows 0 against it", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail("c1") as never,
    );
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({
        occupancy: { contextWindowTokens: 128_000, percentage: 0 },
      }),
    );

    await useChatStore.getState().switchConversation("c1");
    await flush();

    const usage = useChatStore.getState().contextUsage;
    expect(usage.hasUsage).toBe(false);
    expect(usage.usedTokens).toBe(0);
    expect(usage.maxTokens).toBe(128_000);
    expect(usage.percentage).toBe(0);
  });

  it("shows a projection estimate when the branch has messages but no records", async () => {
    vi.mocked(chatService.getConversationPaginated).mockResolvedValue(
      detail("c1") as never,
    );
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(
      summary({
        occupancy: {
          usedTokens: 6_000,
          contextWindowTokens: 128_000,
          percentage: 6_000 / 128_000,
          estimated: true,
          estimatedSource: "projection",
        },
      }),
    );

    await useChatStore.getState().switchConversation("c1");
    await flush();

    const usage = useChatStore.getState().contextUsage;
    expect(usage.hasUsage).toBe(true);
    expect(usage.usedTokens).toBe(6_000);
    expect(usage.estimated).toBe(true);
    expect(usage.estimatedSource).toBe("projection");
    expect(usage.cumulativeTokens).toBeUndefined();
  });
});

function statePort() {
  return createStatePort(
    (partial) => useChatStore.setState(partial),
    () => useChatStore.getState(),
    {
      isActiveConv: (convId) =>
        useChatStore.getState().currentConversationId === convId,
      setContextUsage: (usage) =>
        useChatStore.setState({ contextUsage: usage }),
    },
  ).port;
}

describe("finishStream without usage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useChatStreamStore.setState({
      isStreaming: false,
      streamingContent: "",
      streamingMessageId: null,
      activeAttemptId: null,
      isReasoningStreaming: false,
      retryStatus: null,
    });
    useChatStore.setState({
      currentConversationId: "c1",
      messages: [],
      contextUsage: {
        usedTokens: 5_000,
        hasUsage: true,
        maxTokens: 128_000,
        percentage: 5_000 / 128_000,
        cumulativeTokens: 9_000,
      },
    });
  });

  it("does not reset occupancy to zero", async () => {
    useChatStreamStore.setState({
      isStreaming: true,
      streamingMessageId: "m2",
      streamingContent: "hi",
    });
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(null);

    statePort().finishStream(undefined);
    await flush();

    const usage = useChatStore.getState().contextUsage;
    expect(usage.usedTokens).toBe(5_000);
    expect(usage.hasUsage).toBe(true);
    expect(usage.cumulativeTokens).toBe(9_000);
  });

  it("replaces occupancy with the new request usage", async () => {
    useChatStreamStore.setState({
      isStreaming: true,
      streamingMessageId: "m2",
      streamingContent: "hi",
    });
    vi.mocked(chatService.getConversationTokenUsage).mockResolvedValue(null);

    statePort().finishStream({
      promptTokens: 6_000,
      completionTokens: 300,
      totalTokens: 6_300,
      cacheReadTokens: null,
      cacheWriteTokens: null,
      reasoningTokens: null,
      contextWindowTokens: 128_000,
    });
    await flush();

    const usage = useChatStore.getState().contextUsage;
    expect(usage.usedTokens).toBe(6_000);
    expect(usage.completionTokens).toBe(300);
    expect(usage.cumulativeTokens).toBe(9_000);
  });
});
