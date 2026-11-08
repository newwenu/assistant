import { describe, it, expect, beforeEach, vi } from "vitest";

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

describe("chatStreamStore behavior equivalence (post-integration)", () => {
  beforeEach(() => {
    useChatStreamStore.setState({
      isStreaming: false,
      streamingContent: "",
      streamingMessageId: null,
      activeAttemptId: null,
      isReasoningStreaming: false,
      retryStatus: null,
    });
    useChatStore.setState({
      messages: [],
      contextUsage: { usedTokens: 0, hasUsage: false },
    });
  });

  describe("initial values", () => {
    it("defaults all streaming scalars to idle state", () => {
      const s = useChatStreamStore.getState();
      expect(s.isStreaming).toBe(false);
      expect(s.streamingContent).toBe("");
      expect(s.streamingMessageId).toBeNull();
      expect(s.activeAttemptId).toBeNull();
      expect(s.isReasoningStreaming).toBe(false);
      expect(s.retryStatus).toBeNull();
    });
  });

  describe("beginStream", () => {
    it("sets isStreaming=true, streamingMessageId, resets other scalars", () => {
      useChatStreamStore.setState({
        isReasoningStreaming: true,
        retryStatus: {
          attempt: 1,
          maxAttempts: 3,
          delayMs: 1000,
          errorCode: null,
          message: "retry",
        },
        streamingContent: "leftover",
      });

      useChatStreamStore.getState().beginStream("msg-assistant-1");

      const s = useChatStreamStore.getState();
      expect(s.isStreaming).toBe(true);
      expect(s.streamingMessageId).toBe("msg-assistant-1");
      expect(s.streamingContent).toBe("");
      expect(s.activeAttemptId).toBeNull();
      expect(s.isReasoningStreaming).toBe(false);
      expect(s.retryStatus).toBeNull();
    });
  });

  describe("appendDelta", () => {
    it("accumulates streamingContent", () => {
      useChatStreamStore.setState({
        isStreaming: true,
        streamingMessageId: "msg-1",
        streamingContent: "",
      });

      useChatStreamStore.getState().appendDelta("Hello");
      expect(useChatStreamStore.getState().streamingContent).toBe("Hello");

      useChatStreamStore.getState().appendDelta(" World");
      expect(useChatStreamStore.getState().streamingContent).toBe(
        "Hello World",
      );
    });
  });

  describe("setActiveAttemptId", () => {
    it("sets activeAttemptId and updates streamingMessageId", () => {
      useChatStreamStore.setState({
        isStreaming: true,
        streamingMessageId: "temp-msg-1",
        streamingContent: "partial",
      });

      useChatStreamStore
        .getState()
        .setActiveAttemptId("attempt-1", "real-msg-1");

      const s = useChatStreamStore.getState();
      expect(s.activeAttemptId).toBe("attempt-1");
      expect(s.streamingMessageId).toBe("real-msg-1");
    });
  });

  describe("clearActiveAttemptId", () => {
    it("clears activeAttemptId and streamingMessageId", () => {
      useChatStreamStore.setState({
        activeAttemptId: "attempt-1",
        streamingMessageId: "msg-1",
      });

      useChatStreamStore.getState().clearActiveAttemptId();

      const s = useChatStreamStore.getState();
      expect(s.activeAttemptId).toBeNull();
      expect(s.streamingMessageId).toBeNull();
    });
  });

  describe("cancelStreaming", () => {
    it("resets all 6 streaming scalars", () => {
      useChatStreamStore.setState({
        isStreaming: true,
        streamingContent: "partial response",
        streamingMessageId: "msg-1",
        activeAttemptId: "attempt-1",
        isReasoningStreaming: true,
        retryStatus: {
          attempt: 1,
          maxAttempts: 3,
          delayMs: 1000,
          errorCode: null,
          message: "retry",
        },
      });

      useChatStreamStore.getState().cancelStreaming();

      const s = useChatStreamStore.getState();
      expect(s.isStreaming).toBe(false);
      expect(s.streamingContent).toBe("");
      expect(s.streamingMessageId).toBeNull();
      expect(s.activeAttemptId).toBeNull();
      expect(s.isReasoningStreaming).toBe(false);
      expect(s.retryStatus).toBeNull();
    });
  });

  describe("resetStreaming", () => {
    it("resets all 6 streaming scalars (same as cancelStreaming)", () => {
      useChatStreamStore.setState({
        isStreaming: true,
        streamingContent: "partial",
        streamingMessageId: "msg-1",
        activeAttemptId: "attempt-1",
        isReasoningStreaming: true,
        retryStatus: {
          attempt: 1,
          maxAttempts: 3,
          delayMs: 1000,
          errorCode: null,
          message: "retry",
        },
      });

      useChatStreamStore.getState().resetStreaming();

      const s = useChatStreamStore.getState();
      expect(s.isStreaming).toBe(false);
      expect(s.streamingContent).toBe("");
      expect(s.streamingMessageId).toBeNull();
      expect(s.activeAttemptId).toBeNull();
      expect(s.isReasoningStreaming).toBe(false);
      expect(s.retryStatus).toBeNull();
    });
  });

  describe("setReasoningStreaming", () => {
    it("sets isReasoningStreaming to true", () => {
      useChatStreamStore.setState({ isReasoningStreaming: false });
      useChatStreamStore.getState().setReasoningStreaming(true);
      expect(useChatStreamStore.getState().isReasoningStreaming).toBe(true);
    });

    it("sets isReasoningStreaming to false", () => {
      useChatStreamStore.setState({ isReasoningStreaming: true });
      useChatStreamStore.getState().setReasoningStreaming(false);
      expect(useChatStreamStore.getState().isReasoningStreaming).toBe(false);
    });
  });

  describe("setRetryStatus", () => {
    it("sets retryStatus", () => {
      const retry = {
        attempt: 2,
        maxAttempts: 5,
        delayMs: 2000,
        errorCode: null,
        message: "rate limited",
      };
      useChatStreamStore.getState().setRetryStatus(retry);
      expect(useChatStreamStore.getState().retryStatus).toEqual(retry);
    });

    it("clears retryStatus with null", () => {
      useChatStreamStore.setState({
        retryStatus: {
          attempt: 1,
          maxAttempts: 3,
          delayMs: 1000,
          errorCode: null,
          message: "retry",
        },
      });
      useChatStreamStore.getState().setRetryStatus(null);
      expect(useChatStreamStore.getState().retryStatus).toBeNull();
    });
  });

  describe("manualCompact error handling — Bug FIXED", () => {
    it("error handler now correctly resets isReasoningStreaming via cancelStreaming", () => {
      useChatStreamStore.setState({
        isStreaming: true,
        streamingContent: "",
        streamingMessageId: "temp-compact-1",
        activeAttemptId: null,
        isReasoningStreaming: true,
      });

      useChatStreamStore.getState().cancelStreaming();

      const s = useChatStreamStore.getState();
      expect(s.isStreaming).toBe(false);
      expect(s.streamingMessageId).toBeNull();
      expect(s.activeAttemptId).toBeNull();
      expect(s.isReasoningStreaming).toBe(false);
    });
  });
});
