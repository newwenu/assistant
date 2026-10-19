import { describe, it, expect } from "vitest";
import type {
  Message,
  Conversation,
  ConversationDetail,
  TokenUsage,
  RetryNotification,
  ChatDeltaPayload,
  ChatStreamStartPayload,
  ConversationTerminatedPayload,
  ConversationCompactFailedPayload,
  ConversationUnlockedPayload,
  ChatRegeneratePayload,
} from "../chatTypes";
import type { Provider } from "../../../settings/provider/types";
import type { SettingEventPayload } from "../../../types/events";
import { SettingAction } from "../../../types/events";

const sampleMessage: Message = {
  id: "msg-1",
  conversationId: "conv-1",
  role: "user",
  content: "hello",
  tokens: null,
  timestamp: 1000,
  parentId: null,
  siblingIndex: 0,
  isPinned: false,
  reasoning: undefined,
  toolCalls: undefined,
};

const sampleConversation: Conversation = {
  id: "conv-1",
  title: "Test",
  titleSource: "placeholder",
  providerId: null,
  modelId: "gpt-4",
  systemPrompt: null,
  isPinned: false,
  sortOrder: 0,
  generation: 0,
  createdAt: 1000,
  updatedAt: 1000,
  isFull: false,
  activeBranchId: null,
};

const sampleConversationDetail: ConversationDetail = {
  conversation: sampleConversation,
  messages: [sampleMessage],
  totalCount: 1,
};

describe("protocol DTO — camelCase contract", () => {
  it("Message uses camelCase fields", () => {
    const msg = sampleMessage;
    expect(msg).toHaveProperty("conversationId");
    expect(msg).toHaveProperty("parentId");
    expect(msg).toHaveProperty("isPinned");
    expect(msg).toHaveProperty("toolCalls");
    expect(msg).not.toHaveProperty("conversation_id");
    expect(msg).not.toHaveProperty("parent_id");
    expect(msg).not.toHaveProperty("is_pinned");
    expect(msg).not.toHaveProperty("tool_calls");
  });

  it("Conversation uses camelCase fields", () => {
    const conv = sampleConversation;
    expect(conv).toHaveProperty("providerId");
    expect(conv).toHaveProperty("titleSource");
    expect(conv).toHaveProperty("systemPrompt");
    expect(conv).toHaveProperty("isPinned");
    expect(conv).toHaveProperty("sortOrder");
    expect(conv).toHaveProperty("createdAt");
    expect(conv).toHaveProperty("updatedAt");
    expect(conv).toHaveProperty("isFull");
    expect(conv).not.toHaveProperty("provider_id");
    expect(conv).not.toHaveProperty("title_source");
    expect(conv).not.toHaveProperty("is_pinned");
    expect(conv).not.toHaveProperty("created_at");
  });

  it("ChatDeltaPayload uses camelCase fields", () => {
    const payload: ChatDeltaPayload = {
      conversationId: "c",
      attemptId: "a",
      messageId: "m",
      seq: 0,
      deltaText: "hi",
      deltaReasoning: undefined,
      deltaToolCallId: undefined,
      deltaToolCallName: undefined,
      deltaToolCallArguments: undefined,
      finishReason: undefined,
      usage: undefined,
      error: undefined,
      deltaRetry: undefined,
    };
    expect(payload).toHaveProperty("conversationId");
    expect(payload).toHaveProperty("attemptId");
    expect(payload).toHaveProperty("messageId");
    expect(payload).toHaveProperty("deltaText");
    expect(payload).toHaveProperty("deltaReasoning");
    expect(payload).toHaveProperty("deltaToolCallId");
    expect(payload).toHaveProperty("deltaToolCallName");
    expect(payload).toHaveProperty("deltaToolCallArguments");
    expect(payload).toHaveProperty("finishReason");
    expect(payload).not.toHaveProperty("conversation_id");
    expect(payload).not.toHaveProperty("delta_text");
    expect(payload).not.toHaveProperty("finish_reason");
  });

  it("ChatStreamStartPayload uses camelCase fields", () => {
    const payload: ChatStreamStartPayload = {
      conversationId: "c",
      attemptId: "a",
      messageId: "m",
    };
    expect(payload).toHaveProperty("conversationId");
    expect(payload).toHaveProperty("attemptId");
    expect(payload).not.toHaveProperty("conversation_id");
  });

  it("ConversationTerminatedPayload uses camelCase fields", () => {
    const payload: ConversationTerminatedPayload = {
      conversationId: "c",
      summary: "done",
      isFull: false,
    };
    expect(payload).toHaveProperty("conversationId");
    expect(payload).toHaveProperty("isFull");
    expect(payload).not.toHaveProperty("is_full");
  });

  it("ConversationCompactFailedPayload uses camelCase fields", () => {
    const payload: ConversationCompactFailedPayload = {
      conversationId: "c",
      error: "oom",
    };
    expect(payload).toHaveProperty("conversationId");
  });

  it("ConversationUnlockedPayload uses camelCase fields", () => {
    const payload: ConversationUnlockedPayload = {
      conversationId: "c",
    };
    expect(payload).toHaveProperty("conversationId");
    expect(Object.keys(payload)).toHaveLength(1);
  });

  it("TokenUsage uses camelCase fields", () => {
    const usage: TokenUsage = {
      promptTokens: 10,
      completionTokens: 20,
      totalTokens: 30,
      cacheReadTokens: 0,
      cacheWriteTokens: 0,
      contextWindowTokens: 0,
    };
    expect(usage).toHaveProperty("promptTokens");
    expect(usage).toHaveProperty("completionTokens");
    expect(usage).toHaveProperty("totalTokens");
    expect(usage).toHaveProperty("cacheReadTokens");
    expect(usage).toHaveProperty("cacheWriteTokens");
    expect(usage).toHaveProperty("contextWindowTokens");
    expect(usage).not.toHaveProperty("prompt_tokens");
    expect(usage).not.toHaveProperty("cache_read_tokens");
  });

  it("RetryNotification uses camelCase fields", () => {
    const retry: RetryNotification = {
      attempt: 1,
      maxAttempts: 3,
      delayMs: 500,
      message: "retrying",
    };
    expect(retry).toHaveProperty("delayMs");
    expect(retry).toHaveProperty("maxAttempts");
    expect(retry).not.toHaveProperty("delay_ms");
    expect(retry).not.toHaveProperty("max_attempts");
  });

  it("ChatRegeneratePayload uses camelCase fields", () => {
    const payload: ChatRegeneratePayload = {
      conversationId: "c",
      deletedMessageId: "m",
    };
    expect(payload).toHaveProperty("conversationId");
    expect(payload).toHaveProperty("deletedMessageId");
    expect(payload).not.toHaveProperty("conversation_id");
    expect(payload).not.toHaveProperty("deleted_message_id");
    expect(Object.keys(payload)).toHaveLength(2);
  });

  it("Provider uses camelCase fields", () => {
    const provider: Provider = {
      id: "p1",
      name: "OpenAI",
      type: "openai",
      baseUrl: "https://api.openai.com",
      apiKeyMasked: "sk-***",
      models: [
        {
          id: "gpt-4",
          name: "GPT-4",
          ownedBy: "openai",
          contextWindow: 8192,
        },
      ],
      defaultModel: "gpt-4",
      updatedAt: 1000,
    };
    expect(provider).toHaveProperty("baseUrl");
    expect(provider).toHaveProperty("apiKeyMasked");
    expect(provider).toHaveProperty("defaultModel");
    expect(provider).toHaveProperty("updatedAt");
    expect(provider).not.toHaveProperty("base_url");
    expect(provider).not.toHaveProperty("api_key_masked");
    expect(provider).not.toHaveProperty("default_model");
    expect(provider).not.toHaveProperty("updated_at");
  });

  it("SettingEventPayload uses camelCase fields", () => {
    const payload: SettingEventPayload = {
      action: SettingAction.ProviderSaved,
      id: "p1",
    };
    expect(payload).toHaveProperty("action");
    expect(payload).toHaveProperty("id");
    expect(payload).not.toHaveProperty("Id");
  });
});

describe("business type — field preservation", () => {
  it("Message preserves all fields", () => {
    const msg = sampleMessage;
    expect(msg.id).toBe("msg-1");
    expect(msg.conversationId).toBe("conv-1");
    expect(msg.role).toBe("user");
    expect(msg.content).toBe("hello");
    expect(msg.tokens).toBeNull();
    expect(msg.timestamp).toBe(1000);
    expect(msg.isPinned).toBe(false);
  });

  it("Message handles null/undefined for optional fields", () => {
    const msg = sampleMessage;
    expect(msg.parentId).toBeNull();
    expect(msg.reasoning).toBeUndefined();
  });

  it("Conversation preserves all fields", () => {
    const conv = sampleConversation;
    expect(conv.id).toBe("conv-1");
    expect(conv.title).toBe("Test");
    expect(conv.providerId).toBeNull();
    expect(conv.modelId).toBe("gpt-4");
    expect(conv.isPinned).toBe(false);
    expect(conv.isFull).toBe(false);
  });

  it("ConversationDetail maps nested structures", () => {
    const detail = sampleConversationDetail;
    expect(detail.conversation.id).toBe("conv-1");
    expect(detail.messages).toHaveLength(1);
    expect(detail.messages[0].id).toBe("msg-1");
    expect(detail.totalCount).toBe(1);
  });
});
