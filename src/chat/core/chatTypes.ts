import { type ChatError } from "./chatErrors";

export interface Message {
  id: string;
  conversationId: string;
  role: string;
  content: string;
  reasoning?: string;
  toolCalls?: ToolCall[];
  tokens: number | null;
  timestamp: number;
  error?: ChatError | null;
  parentId?: string | null;
  isPinned?: boolean;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface Conversation {
  id: string;
  title: string;
  titleSource: string;
  providerId: string | null;
  modelId: string;
  systemPrompt: string | null;
  isPinned: boolean;
  sortOrder: number;
  generation: number;
  createdAt: number;
  updatedAt: number;
  isFull: boolean;
}

export interface TokenUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  cacheReadTokens?: number;
  cacheWriteTokens?: number;
  contextWindowTokens?: number;
}

export interface ContextUsage {
  usedTokens: number;
  maxTokens: number;
  percentage: number;
}
