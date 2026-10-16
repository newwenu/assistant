import { describe, it, expect } from "vitest";
import type {
  MessageRow,
  ConversationRow,
  ConversationDetailRow,
  TokenUsageRow,
  RetryNotificationRow,
  ChatDeltaPayload,
  ChatStreamStartPayload,
  ConversationTerminatedPayload,
  ConversationCompactFailedPayload,
  ConversationUnlockedPayload,
  ChatRegeneratePayload,
} from "../protocol";
import type { Provider } from "../../../settings/provider/types";
import type { SettingEventPayload } from "../../../types/events";
import { SettingAction } from "../../../types/events";
import {
  rowToMessage,
  rowToConversation,
  rowToConversationDetail,
  messageToRow,
  conversationToRow,
} from "../protocolAdapter";

const sampleMessageRow: MessageRow = {
  id: "msg-1",
  conversation_id: "conv-1",
  role: "user",
  content: "hello",
  tokens: null,
  timestamp: 1000,
  parent_id: null,
  is_pinned: false,
  reasoning: null,
  tool_calls_json: null,
};

const sampleConversationRow: ConversationRow = {
  id: "conv-1",
  title: "Test",
  title_source: "placeholder",
  provider_id: null,
  model_id: "gpt-4",
  system_prompt: null,
  is_pinned: false,
  sort_order: 0,
  generation: 0,
  created_at: 1000,
  updated_at: 1000,
  is_full: false,
};

const sampleConversationDetailRow: ConversationDetailRow = {
  conversation: sampleConversationRow,
  messages: [sampleMessageRow],
  total_count: 1,
};

describe("protocol DTO — wire format contract", () => {
  it("MessageRow uses snake_case fields", () => {
    const row = sampleMessageRow;
    expect(row).toHaveProperty("conversation_id");
    expect(row).toHaveProperty("parent_id");
    expect(row).toHaveProperty("is_pinned");
    expect(row).toHaveProperty("tool_calls_json");
    expect(row).not.toHaveProperty("conversationId");
    expect(row).not.toHaveProperty("parentId");
    expect(row).not.toHaveProperty("isPinned");
    expect(row).not.toHaveProperty("toolCallsJson");
  });

  it("ConversationRow uses snake_case fields", () => {
    const row = sampleConversationRow;
    expect(row).toHaveProperty("provider_id");
    expect(row).toHaveProperty("title_source");
    expect(row).toHaveProperty("system_prompt");
    expect(row).toHaveProperty("is_pinned");
    expect(row).toHaveProperty("sort_order");
    expect(row).toHaveProperty("created_at");
    expect(row).toHaveProperty("updated_at");
    expect(row).toHaveProperty("is_full");
    expect(row).not.toHaveProperty("providerId");
    expect(row).not.toHaveProperty("titleSource");
    expect(row).not.toHaveProperty("isPinned");
    expect(row).not.toHaveProperty("createdAt");
  });

  it("ChatDeltaPayload uses snake_case fields", () => {
    const payload: ChatDeltaPayload = {
      conversation_id: "c",
      attempt_id: "a",
      message_id: "m",
      seq: 0,
      delta_text: "hi",
      delta_reasoning: undefined,
      delta_tool_call_id: undefined,
      delta_tool_call_name: undefined,
      delta_tool_call_arguments: undefined,
      finish_reason: undefined,
      usage: undefined,
      error: undefined,
      delta_retry: undefined,
    };
    expect(payload).toHaveProperty("conversation_id");
    expect(payload).toHaveProperty("attempt_id");
    expect(payload).toHaveProperty("message_id");
    expect(payload).toHaveProperty("delta_text");
    expect(payload).toHaveProperty("delta_reasoning");
    expect(payload).toHaveProperty("delta_tool_call_id");
    expect(payload).toHaveProperty("delta_tool_call_name");
    expect(payload).toHaveProperty("delta_tool_call_arguments");
    expect(payload).toHaveProperty("finish_reason");
    expect(payload).not.toHaveProperty("conversationId");
    expect(payload).not.toHaveProperty("deltaText");
    expect(payload).not.toHaveProperty("finishReason");
  });

  it("ChatStreamStartPayload uses snake_case fields", () => {
    const payload: ChatStreamStartPayload = {
      conversation_id: "c",
      attempt_id: "a",
      message_id: "m",
    };
    expect(payload).toHaveProperty("conversation_id");
    expect(payload).toHaveProperty("attempt_id");
    expect(payload).not.toHaveProperty("conversationId");
  });

  it("ConversationTerminatedPayload uses snake_case fields", () => {
    const payload: ConversationTerminatedPayload = {
      conversation_id: "c",
      summary: "done",
      is_full: false,
    };
    expect(payload).toHaveProperty("conversation_id");
    expect(payload).toHaveProperty("is_full");
    expect(payload).not.toHaveProperty("isFull");
  });

  it("ConversationCompactFailedPayload uses snake_case fields", () => {
    const payload: ConversationCompactFailedPayload = {
      conversation_id: "c",
      error: "oom",
    };
    expect(payload).toHaveProperty("conversation_id");
  });

  it("ConversationUnlockedPayload uses snake_case fields", () => {
    const payload: ConversationUnlockedPayload = {
      conversation_id: "c",
    };
    expect(payload).toHaveProperty("conversation_id");
    expect(Object.keys(payload)).toHaveLength(1);
  });

  it("TokenUsageRow uses snake_case fields", () => {
    const usage: TokenUsageRow = {
      prompt_tokens: 10,
      completion_tokens: 20,
      total_tokens: 30,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      context_window_tokens: 0,
    };
    expect(usage).toHaveProperty("prompt_tokens");
    expect(usage).toHaveProperty("completion_tokens");
    expect(usage).toHaveProperty("total_tokens");
    expect(usage).toHaveProperty("cache_read_tokens");
    expect(usage).toHaveProperty("cache_write_tokens");
    expect(usage).toHaveProperty("context_window_tokens");
    expect(usage).not.toHaveProperty("promptTokens");
    expect(usage).not.toHaveProperty("cacheReadTokens");
  });

  it("RetryNotificationRow uses snake_case fields", () => {
    const retry: RetryNotificationRow = {
      attempt: 1,
      max_attempts: 3,
      delay_ms: 500,
      message: "retrying",
    };
    expect(retry).toHaveProperty("delay_ms");
    expect(retry).toHaveProperty("max_attempts");
    expect(retry).not.toHaveProperty("delayMs");
    expect(retry).not.toHaveProperty("maxAttempts");
  });

  it("ChatRegeneratePayload uses snake_case fields", () => {
    const payload: ChatRegeneratePayload = {
      conversation_id: "c",
      deleted_message_id: "m",
    };
    expect(payload).toHaveProperty("conversation_id");
    expect(payload).toHaveProperty("deleted_message_id");
    expect(payload).not.toHaveProperty("conversationId");
    expect(payload).not.toHaveProperty("deletedMessageId");
    expect(Object.keys(payload)).toHaveLength(2);
  });

  it("Provider (ProviderListRow mirror) uses snake_case fields", () => {
    const provider: Provider = {
      id: "p1",
      name: "OpenAI",
      type: "openai",
      base_url: "https://api.openai.com",
      api_key_masked: "sk-***",
      models: [
        {
          id: "gpt-4",
          name: "GPT-4",
          owned_by: "openai",
          context_window: 8192,
        },
      ],
      default_model: "gpt-4",
      updated_at: 1000,
    };
    expect(provider).toHaveProperty("base_url");
    expect(provider).toHaveProperty("api_key_masked");
    expect(provider).toHaveProperty("default_model");
    expect(provider).toHaveProperty("updated_at");
    expect(provider).not.toHaveProperty("baseUrl");
    expect(provider).not.toHaveProperty("apiKeyMasked");
    expect(provider).not.toHaveProperty("defaultModel");
    expect(provider).not.toHaveProperty("updatedAt");
  });

  it("SettingEventPayload (SettingProviderChangedPayload mirror) uses snake_case fields", () => {
    const payload: SettingEventPayload = {
      action: SettingAction.ProviderSaved,
      id: "p1",
    };
    expect(payload).toHaveProperty("action");
    expect(payload).toHaveProperty("id");
    expect(payload).not.toHaveProperty("Id");
  });
});

describe("protocolAdapter — DTO ↔ business type mapping", () => {
  it("rowToMessage preserves all fields", () => {
    const msg = rowToMessage(sampleMessageRow);
    expect(msg.id).toBe("msg-1");
    expect(msg.conversationId).toBe("conv-1");
    expect(msg.role).toBe("user");
    expect(msg.content).toBe("hello");
    expect(msg.tokens).toBeNull();
    expect(msg.timestamp).toBe(1000);
    expect(msg.isPinned).toBe(false);
  });

  it("rowToMessage handles null → undefined for optional fields", () => {
    const msg = rowToMessage(sampleMessageRow);
    expect(msg.parentId).toBeUndefined();
    expect(msg.reasoning).toBeUndefined();
  });

  it("rowToMessage parses tool_calls_json", () => {
    const row: MessageRow = {
      ...sampleMessageRow,
      tool_calls_json: JSON.stringify([
        { id: "tc1", name: "search", arguments: "{}" },
      ]),
    };
    const msg = rowToMessage(row);
    expect(msg.toolCalls).toHaveLength(1);
    expect(msg.toolCalls![0].name).toBe("search");
  });

  it("rowToMessage returns undefined toolCalls for null tool_calls_json", () => {
    const msg = rowToMessage(sampleMessageRow);
    expect(msg.toolCalls).toBeUndefined();
  });

  it("rowToConversation preserves all fields", () => {
    const conv = rowToConversation(sampleConversationRow);
    expect(conv.id).toBe("conv-1");
    expect(conv.title).toBe("Test");
    expect(conv.providerId).toBeNull();
    expect(conv.modelId).toBe("gpt-4");
    expect(conv.isPinned).toBe(false);
    expect(conv.isFull).toBe(false);
  });

  it("rowToConversationDetail maps nested structures", () => {
    const detail = rowToConversationDetail(sampleConversationDetailRow);
    expect(detail.conversation.id).toBe("conv-1");
    expect(detail.messages).toHaveLength(1);
    expect(detail.messages[0].id).toBe("msg-1");
    expect(detail.totalCount).toBe(1);
  });

  it("messageToRow converts business Message back to DTO MessageRow", () => {
    const msg = rowToMessage(sampleMessageRow);
    const row = messageToRow(msg);
    expect(row.id).toBe("msg-1");
    expect(row.conversation_id).toBe("conv-1");
    expect(row.parent_id).toBeNull();
    expect(row.reasoning).toBeNull();
    expect(row.tool_calls_json).toBeNull();
  });

  it("messageToRow serializes toolCalls to tool_calls_json", () => {
    const row: MessageRow = {
      ...sampleMessageRow,
      tool_calls_json: JSON.stringify([
        { id: "tc1", name: "search", arguments: "{}" },
      ]),
    };
    const msg = rowToMessage(row);
    const back = messageToRow(msg);
    expect(back.tool_calls_json).not.toBeNull();
    const parsed = JSON.parse(back.tool_calls_json!) as Array<{
      name: string;
    }>;
    expect(parsed[0].name).toBe("search");
  });

  it("conversationToRow converts business Conversation back to DTO ConversationRow", () => {
    const conv = rowToConversation(sampleConversationRow);
    const row = conversationToRow(conv);
    expect(row.id).toBe("conv-1");
    expect(row.provider_id).toBeNull();
    expect(row.is_pinned).toBe(false);
  });

  it("round-trip: MessageRow → Message → MessageRow preserves data", () => {
    const original = sampleMessageRow;
    const msg = rowToMessage(original);
    const restored = messageToRow(msg);
    expect(restored.id).toBe(original.id);
    expect(restored.conversation_id).toBe(original.conversation_id);
    expect(restored.role).toBe(original.role);
    expect(restored.content).toBe(original.content);
    expect(restored.tokens).toBe(original.tokens);
    expect(restored.timestamp).toBe(original.timestamp);
  });

  it("round-trip: ConversationRow → Conversation → ConversationRow preserves data", () => {
    const original = sampleConversationRow;
    const conv = rowToConversation(original);
    const restored = conversationToRow(conv);
    expect(restored).toEqual(original);
  });
});
