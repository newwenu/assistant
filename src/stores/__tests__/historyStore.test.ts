import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@/i18n", () => ({
  translateKey: (key: string) => key,
}));

vi.mock("../../services/historyService", () => ({
  historyService: {
    getConversationAllMessages: vi.fn(),
  },
}));

vi.mock("../../services/chat/chatService", () => ({
  chatService: {
    listConversations: vi.fn(() => Promise.resolve([])),
    listProviders: vi.fn(() => Promise.resolve([])),
    createConversation: vi.fn(() => Promise.resolve("conv-1")),
    getConversationPaginated: vi.fn(() => Promise.resolve(null)),
    getConversationAllMessages: vi.fn(() => Promise.resolve(null)),
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

import { useHistoryStore } from "../historyStore";
import { useChatStore } from "../chat/chatStore";
import { historyService } from "../../services/historyService";
import { chatService } from "../../services/chat/chatService";

const MESSAGES_FAILED = "history.load_messages_failed";
const CONVERSATIONS_FAILED = "history.load_conversations_failed";

describe("historyStore read failure visibility", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useHistoryStore.setState({
      selectedId: null,
      messages: [],
      totalCount: 0,
      loadingMessages: false,
      messagesError: null,
    });
    useChatStore.setState({ conversations: [], conversationsError: null });
  });

  it("selectConversation clears messagesError on success", async () => {
    vi.mocked(historyService.getConversationAllMessages).mockResolvedValue(
      null,
    );
    await useHistoryStore.getState().selectConversation("c1");
    expect(useHistoryStore.getState().messagesError).toBeNull();
  });

  it("selectConversation exposes messagesError on failure", async () => {
    vi.mocked(historyService.getConversationAllMessages).mockRejectedValue(
      new Error("boom"),
    );
    await useHistoryStore.getState().selectConversation("c1");
    expect(useHistoryStore.getState().messagesError).toBe(MESSAGES_FAILED);
  });

  it("refreshMessages clears messagesError on success", async () => {
    useHistoryStore.setState({ selectedId: "c1" });
    vi.mocked(historyService.getConversationAllMessages).mockResolvedValue(
      null,
    );
    await useHistoryStore.getState().refreshMessages();
    expect(useHistoryStore.getState().messagesError).toBeNull();
  });

  it("refreshMessages exposes messagesError on failure", async () => {
    useHistoryStore.setState({ selectedId: "c1" });
    vi.mocked(historyService.getConversationAllMessages).mockRejectedValue(
      new Error("boom"),
    );
    await useHistoryStore.getState().refreshMessages();
    expect(useHistoryStore.getState().messagesError).toBe(MESSAGES_FAILED);
  });

  it("loadConversations forwards failure to chat store", async () => {
    vi.mocked(chatService.listConversations).mockRejectedValue(
      new Error("boom"),
    );
    await useHistoryStore.getState().loadConversations();
    expect(useChatStore.getState().conversationsError).toBe(
      CONVERSATIONS_FAILED,
    );
  });
});
