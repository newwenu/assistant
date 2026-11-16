import { describe, it, expect, beforeEach } from "vitest";
import { useChatStreamStore } from "../chat/chatStreamStore";
import type { RetryNotification } from "../../chat/core/chatTypes";

describe("chatStreamStore", () => {
  beforeEach(() => {
    useChatStreamStore.setState({
      isStreaming: false,
      streamingContent: "",
      streamingMessageId: null,
      activeAttemptId: null,
      isReasoningStreaming: false,
      retryStatus: null,
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

  describe("resumeStream", () => {
    it("标记流式中并记录 attemptId，messageId 未知时保留原值", () => {
      useChatStreamStore.getState().resumeStream("attempt-1", null);

      const s = useChatStreamStore.getState();
      expect(s.isStreaming).toBe(true);
      expect(s.activeAttemptId).toBe("attempt-1");
      expect(s.streamingMessageId).toBeNull();
    });

    it("已知 messageId 时一并写入，不覆盖已有流式内容", () => {
      useChatStreamStore.setState({ streamingContent: "already" });
      useChatStreamStore.getState().resumeStream("attempt-2", "msg-2");

      const s = useChatStreamStore.getState();
      expect(s.isStreaming).toBe(true);
      expect(s.activeAttemptId).toBe("attempt-2");
      expect(s.streamingMessageId).toBe("msg-2");
      expect(s.streamingContent).toBe("already");
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

  describe("resetStreaming vs cancelStreaming behavior equivalence", () => {
    it("both produce identical scalar state", () => {
      const setup = {
        isStreaming: true,
        streamingContent: "some content",
        streamingMessageId: "msg-1",
        activeAttemptId: "attempt-1",
        isReasoningStreaming: true,
        retryStatus: {
          attempt: 1,
          maxAttempts: 3,
          delayMs: 1000,
          errorCode: null,
          message: "retry",
        } as RetryNotification,
      };

      useChatStreamStore.setState(setup);
      useChatStreamStore.getState().cancelStreaming();
      const afterCancel = {
        isStreaming: useChatStreamStore.getState().isStreaming,
        streamingContent: useChatStreamStore.getState().streamingContent,
        streamingMessageId: useChatStreamStore.getState().streamingMessageId,
        activeAttemptId: useChatStreamStore.getState().activeAttemptId,
        isReasoningStreaming:
          useChatStreamStore.getState().isReasoningStreaming,
        retryStatus: useChatStreamStore.getState().retryStatus,
      };

      useChatStreamStore.setState(setup);
      useChatStreamStore.getState().resetStreaming();
      const afterReset = {
        isStreaming: useChatStreamStore.getState().isStreaming,
        streamingContent: useChatStreamStore.getState().streamingContent,
        streamingMessageId: useChatStreamStore.getState().streamingMessageId,
        activeAttemptId: useChatStreamStore.getState().activeAttemptId,
        isReasoningStreaming:
          useChatStreamStore.getState().isReasoningStreaming,
        retryStatus: useChatStreamStore.getState().retryStatus,
      };

      expect(afterCancel).toEqual(afterReset);
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

  describe("full flow: beginStream → appendDelta → cancelStreaming", () => {
    it("streams content then cancels back to idle", () => {
      const store = useChatStreamStore.getState();

      store.beginStream("msg-1");
      expect(useChatStreamStore.getState().isStreaming).toBe(true);
      expect(useChatStreamStore.getState().streamingMessageId).toBe("msg-1");

      store.appendDelta("Hello");
      store.appendDelta(" World");
      expect(useChatStreamStore.getState().streamingContent).toBe(
        "Hello World",
      );

      useChatStreamStore.getState().cancelStreaming();
      const s = useChatStreamStore.getState();
      expect(s.isStreaming).toBe(false);
      expect(s.streamingContent).toBe("");
      expect(s.streamingMessageId).toBeNull();
    });
  });
});
