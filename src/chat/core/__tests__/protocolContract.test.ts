import { describe, it, expect } from "vitest";
import type {
  Message,
  Conversation,
  ConversationDetail,
  TokenUsage,
  RetryNotification,
  ChatDeltaPayload,
  ChatMessagesChangedPayload,
  ChatStreamStartPayload,
  ConversationTerminatedPayload,
  ConversationUnlockedPayload,
  ChatRegeneratePayload,
} from "../chatTypes";
import type { Provider, ModelConfig } from "../../../settings/provider/types";
import type { SettingEventPayload } from "../../../types/events";
import { SettingAction } from "../../../types/events";
import { makeMessage } from "../../../test-utils/messageFixtures";

const sampleMessage: Message = makeMessage("msg-1", {
  content: "hello",
  timestamp: 1000,
});

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
  modeId: null,
  workspaceRoot: "",
  toolConfig: null,
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
    expect(msg).not.toHaveProperty("conversation_id");
    expect(msg).not.toHaveProperty("parent_id");
    expect(msg).not.toHaveProperty("is_pinned");
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
      iteration: 1,
      deltaText: "hi",
      deltaReasoning: null,
      finishReason: null,
      usage: null,
      error: null,
      deltaRetry: null,
    };
    expect(payload).toHaveProperty("conversationId");
    expect(payload).toHaveProperty("attemptId");
    expect(payload).toHaveProperty("messageId");
    expect(payload).toHaveProperty("deltaText");
    expect(payload).toHaveProperty("deltaReasoning");
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
      phase: "starting",
      turnIndex: 1,
      iteration: 1,
    };
    expect(payload).toHaveProperty("conversationId");
    expect(payload).toHaveProperty("attemptId");
    expect(payload).toHaveProperty("turnIndex");
    expect(payload).toHaveProperty("iteration");
    expect(payload).not.toHaveProperty("conversation_id");
    expect(payload).not.toHaveProperty("turn_index");
  });

  it("ConversationTerminatedPayload uses camelCase fields", () => {
    const payload: ConversationTerminatedPayload = {
      conversationId: "c",
      isFull: false,
    };
    expect(payload).toHaveProperty("conversationId");
    expect(payload).toHaveProperty("isFull");
    expect(payload).not.toHaveProperty("is_full");
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
      cacheReadTokens: null,
      cacheWriteTokens: null,
      reasoningTokens: null,
      contextWindowTokens: 0,
    };
    expect(usage).toHaveProperty("promptTokens");
    expect(usage).toHaveProperty("completionTokens");
    expect(usage).toHaveProperty("totalTokens");
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
      errorCode: null,
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
          supportVision: false,
          inputModalities: [],
          reasoningLevels: null,
          vision: null,
          pdf: null,
          attachment: null,
        },
      ],
      defaultModel: "gpt-4",
      updatedAt: 1000,
      effortSupport: [
        {
          modelId: "gpt-4",
          expressible: ["low", "medium", "high"],
          unsupported: ["minimal"],
        },
      ],
    };
    expect(provider).toHaveProperty("baseUrl");
    expect(provider).toHaveProperty("apiKeyMasked");
    expect(provider).toHaveProperty("defaultModel");
    expect(provider).toHaveProperty("updatedAt");
    expect(provider).toHaveProperty("effortSupport");
    expect(provider).not.toHaveProperty("base_url");
    expect(provider).not.toHaveProperty("api_key_masked");
    expect(provider).not.toHaveProperty("default_model");
    expect(provider).not.toHaveProperty("updated_at");
  });

  /**
   * 验收 A10：含三态能力的 model 经「读取 → 写回」后字段不丢。
   * 这里用 JSON 往返模拟 wire：DTO 带 `deny_unknown_fields`，
   * 少注册一个键就会在写回时被整项丢弃。
   */
  it("Model capability facts survive a JSON round-trip", () => {
    const model: ModelConfig = {
      id: "gpt-4o",
      name: "GPT-4o",
      ownedBy: "openai",
      contextWindow: 128000,
      supportVision: true,
      inputModalities: ["text", "image"],
      reasoningLevels: null,
      vision: { support: "supported", source: "api" },
      pdf: { support: "unsupported", source: "catalog" },
      attachment: { support: "unknown", source: "fallback" },
    };

    const written = JSON.parse(JSON.stringify(model)) as ModelConfig;

    expect(written.vision).toEqual({ support: "supported", source: "api" });
    expect(written.pdf).toEqual({ support: "unsupported", source: "catalog" });
    expect(written.attachment).toEqual({
      support: "unknown",
      source: "fallback",
    });
    // 键恒在、值可为 null：不允许「缺失」与「null」两种形态并存
    expect(written).toHaveProperty("vision");
    expect(written).toHaveProperty("pdf");
    expect(written).toHaveProperty("attachment");
  });

  it("Legacy model keeps capability keys as explicit nulls", () => {
    const legacy: ModelConfig = {
      id: "gpt-4",
      name: "GPT-4",
      ownedBy: "openai",
      contextWindow: 8192,
      supportVision: false,
      inputModalities: [],
      reasoningLevels: null,
      vision: null,
      pdf: null,
      attachment: null,
    };
    const written = JSON.parse(JSON.stringify(legacy)) as ModelConfig;
    expect(written.vision).toBeNull();
    expect(written.pdf).toBeNull();
    expect(written.attachment).toBeNull();
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

  it("Message uses null for absent optional fields", () => {
    const msg = sampleMessage;
    expect(msg.parentId).toBeNull();
    expect(msg.reasoning).toBeNull();
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

  it("ChatMessagesChangedPayload reads seq from the wire payload", () => {
    const payload = JSON.parse(
      '{"conversationId":"c","changeKind":"branch_switched","messages":[],"activeBranchId":null,"seq":1}',
    ) as ChatMessagesChangedPayload;
    expect(payload.seq).toBe(1);
  });
});
