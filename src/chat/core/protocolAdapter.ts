/**
 * Protocol adapter: converts DTO layer (snake_case) to business layer (camelCase).
 * Backend mirror: src-tauri/src/protocol/chat_events.rs, chat_commands.rs
 *
 * DTO types use snake_case to match wire format.
 * Business types use camelCase per JS/TS ecosystem convention.
 * This adapter bridges the two layers.
 */

import type { Message, Conversation, ToolCall, TokenUsage } from "./chatTypes";
import type {
  MessageRow,
  ConversationRow,
  ConversationDetailRow,
  TokenUsageRow,
  TokenUsageSummaryRow,
} from "./protocol";

export function invokeList<TDto, TBusiness>(
  promise: Promise<unknown>,
  adapt: (dto: TDto) => TBusiness,
): Promise<TBusiness[]> {
  return promise.then((r) => ((r as TDto[]) || []).map(adapt));
}

export function invokeSingle<TDto, TBusiness>(
  promise: Promise<unknown>,
  adapt: (dto: TDto) => TBusiness,
): Promise<TBusiness | null> {
  return promise.then((r) => (r ? adapt(r as TDto) : null));
}

export function rowToMessage(row: MessageRow): Message {
  return {
    id: row.id,
    conversationId: row.conversation_id,
    role: row.role,
    content: row.content,
    reasoning: row.reasoning ?? undefined,
    toolCalls: parseToolCallsJson(row.tool_calls_json),
    tokens: row.tokens,
    timestamp: row.timestamp,
    parentId: row.parent_id ?? undefined,
    isPinned: row.is_pinned,
  };
}

export function rowToConversation(row: ConversationRow): Conversation {
  return {
    id: row.id,
    title: row.title,
    titleSource: row.title_source,
    providerId: row.provider_id,
    modelId: row.model_id,
    systemPrompt: row.system_prompt,
    isPinned: row.is_pinned,
    sortOrder: row.sort_order,
    generation: row.generation,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    isFull: row.is_full,
  };
}

export function rowToConversationDetail(row: ConversationDetailRow): {
  conversation: Conversation;
  messages: Message[];
  totalCount: number;
} {
  return {
    conversation: rowToConversation(row.conversation),
    messages: (row.messages || []).map(rowToMessage),
    totalCount: row.total_count,
  };
}

export function messageToRow(msg: Message): MessageRow {
  return {
    id: msg.id,
    conversation_id: msg.conversationId,
    role: msg.role,
    content: msg.content,
    tokens: msg.tokens,
    timestamp: msg.timestamp,
    parent_id: msg.parentId ?? null,
    is_pinned: msg.isPinned ?? false,
    reasoning: msg.reasoning ?? null,
    tool_calls_json:
      msg.toolCalls && msg.toolCalls.length > 0
        ? JSON.stringify(msg.toolCalls)
        : null,
  };
}

export function conversationToRow(conv: Conversation): ConversationRow {
  return {
    id: conv.id,
    title: conv.title,
    title_source: conv.titleSource,
    provider_id: conv.providerId,
    model_id: conv.modelId,
    system_prompt: conv.systemPrompt,
    is_pinned: conv.isPinned,
    sort_order: conv.sortOrder,
    generation: conv.generation,
    created_at: conv.createdAt,
    updated_at: conv.updatedAt,
    is_full: conv.isFull,
  };
}

function parseToolCallsJson(json: string | null): ToolCall[] | undefined {
  if (!json) return undefined;
  try {
    const parsed: unknown = JSON.parse(json);
    if (!Array.isArray(parsed)) return undefined;
    for (const item of parsed) {
      if (
        typeof item !== "object" ||
        item === null ||
        !("id" in item) ||
        !("name" in item)
      ) {
        return undefined;
      }
    }
    return parsed as ToolCall[];
  } catch {
    return undefined;
  }
}

export function rowToTokenUsage(row: TokenUsageRow): TokenUsage {
  return {
    promptTokens: row.prompt_tokens,
    completionTokens: row.completion_tokens,
    totalTokens: row.total_tokens,
    cacheReadTokens: row.cache_read_tokens,
    cacheWriteTokens: row.cache_write_tokens,
    contextWindowTokens: row.context_window_tokens,
  };
}

export function rowToTokenUsageSummary(row: TokenUsageSummaryRow) {
  return {
    totalTokens: row.total_tokens,
    contextWindowTokens: row.context_window_tokens,
    percentage: row.percentage,
  };
}