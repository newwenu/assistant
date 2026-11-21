import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@/i18n", () => ({
  translateKey: (key: string) => key,
}));

vi.mock("../../services/chat/chatService", () => ({
  chatService: {
    listConversations: vi.fn(() => Promise.resolve([])),
    listProviders: vi.fn(() => Promise.resolve([])),
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

vi.mock("../../services/system/logger", () => ({
  createLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock("../system/notifyStore", () => ({
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

vi.mock("../agent/agentModeStore", () => ({
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

import { useChatStore } from "../chat/chatStore";
import { chatService } from "../../services/chat/chatService";

const CONVERSATIONS_FAILED = "history.load_conversations_failed";

describe("chatConversationSlice read failure visibility", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useChatStore.setState({
      conversations: [],
      conversationsError: null,
      currentConversationId: null,
      isLoadingConversations: false,
    });
  });

  it("loadConversations clears conversationsError on success", async () => {
    vi.mocked(chatService.listConversations).mockResolvedValue([]);
    await useChatStore.getState().loadConversations();
    expect(useChatStore.getState().conversationsError).toBeNull();
  });

  it("loadConversations exposes conversationsError on failure", async () => {
    vi.mocked(chatService.listConversations).mockRejectedValue(
      new Error("boom"),
    );
    await useChatStore.getState().loadConversations();
    expect(useChatStore.getState().conversationsError).toBe(
      CONVERSATIONS_FAILED,
    );
    expect(useChatStore.getState().isLoadingConversations).toBe(false);
  });
});
