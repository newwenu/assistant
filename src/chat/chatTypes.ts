import { type ChatError } from "./chatErrors";

export interface Message {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  reasoning?: string;
  toolCalls?: ToolCall[];
  tokens: number | null;
  timestamp: number;
  error?: ChatError | null;
  parent_id?: string | null;
  is_pinned?: boolean;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface Conversation {
  id: string;
  title: string;
  title_source: string;
  provider_id: string | null;
  model_id: string;
  system_prompt: string | null;
  is_pinned: boolean;
  sort_order: number;
  generation: number;
  created_at: number;
  updated_at: number;
}

export interface TokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  cache_read_tokens?: number;
  cache_write_tokens?: number;
  context_window_tokens?: number;
}

export interface ContextUsage {
  used_tokens: number;
  max_tokens: number;
  percentage: number;
}
