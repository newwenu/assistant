import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import {
  dispatchProviderSaved,
  dispatchProviderDeleted,
} from "../settings/settingsDispatcher";

export interface ModelConfig {
  id: string;
  name: string;
  owned_by: string;
  context_window?: number;
}

export interface Provider {
  id: string;
  name: string;
  type: string;
  base_url: string;
  api_key_masked: string;
  models: string;
  default_model: string;
  updated_at: number;
}

export type ApiFormat = "openai" | "anthropic" | "local";

export interface ApiFormatInfo {
  format: ApiFormat;
  label: string;
  description: string;
  needsApiKey: boolean;
  keyPlaceholder?: string;
}

export const API_FORMATS: ApiFormatInfo[] = [
  {
    format: "openai",
    label: "OpenAI 兼容",
    description: "Chat Completions /v1 端点",
    needsApiKey: true,
    keyPlaceholder: "sk-...",
  },
  {
    format: "anthropic",
    label: "Anthropic 兼容",
    description: "Messages /v1 端点",
    needsApiKey: true,
    keyPlaceholder: "sk-ant-...",
  },
  {
    format: "local",
    label: "本地运行时",
    description: "Ollama / LM Studio 等本地服务",
    needsApiKey: false,
  },
];

export const API_FORMAT_MAP = new Map<ApiFormat, ApiFormatInfo>(
  API_FORMATS.map((f) => [f.format, f]),
);

export interface ProviderPreset {
  slug: string;
  label: string;
  format: ApiFormat;
  defaultBaseUrl: string;
}

export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    slug: "openai",
    label: "OpenAI",
    format: "openai",
    defaultBaseUrl: "https://api.openai.com/v1",
  },
  {
    slug: "deepseek",
    label: "DeepSeek",
    format: "openai",
    defaultBaseUrl: "https://api.deepseek.com/v1",
  },
  {
    slug: "groq",
    label: "Groq",
    format: "openai",
    defaultBaseUrl: "https://api.groq.com/openai/v1",
  },
  {
    slug: "openrouter",
    label: "OpenRouter",
    format: "openai",
    defaultBaseUrl: "https://openrouter.ai/api/v1",
  },
  {
    slug: "mistral",
    label: "Mistral",
    format: "openai",
    defaultBaseUrl: "https://api.mistral.ai/v1",
  },
  {
    slug: "zhipu",
    label: "智谱 GLM",
    format: "openai",
    defaultBaseUrl: "https://open.bigmodel.cn/api/paas/v4",
  },
  {
    slug: "qwen",
    label: "通义千问",
    format: "openai",
    defaultBaseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  },
  {
    slug: "anthropic",
    label: "Anthropic",
    format: "anthropic",
    defaultBaseUrl: "https://api.anthropic.com/v1",
  },
  {
    slug: "ollama",
    label: "Ollama",
    format: "local",
    defaultBaseUrl: "http://localhost:11434/v1",
  },
  {
    slug: "lmstudio",
    label: "LM Studio",
    format: "local",
    defaultBaseUrl: "http://localhost:1234/v1",
  },
];

export const PRESET_MAP = new Map<string, ProviderPreset>(
  PROVIDER_PRESETS.map((p) => [p.slug, p]),
);

export function getPresetByType(type: string): ProviderPreset | undefined {
  return PRESET_MAP.get(type);
}

export function getApiFormatByType(type: string): ApiFormat {
  const preset = PRESET_MAP.get(type);
  if (preset) return preset.format;
  return "openai";
}

interface ProviderState {
  providers: Provider[];
  loading: boolean;

  loadProviders: () => Promise<void>;
  saveProvider: (provider: Record<string, unknown>) => Promise<void>;
  deleteProvider: (id: string) => Promise<void>;
  fetchModels: (providerId: string) => Promise<ModelConfig[]>;
}

export const useProviderStore = create<ProviderState>((set) => ({
  providers: [],
  loading: false,

  loadProviders: async () => {
    set({ loading: true });
    try {
      const result = await invoke("list_providers");
      set({ providers: (result as Provider[]) || [], loading: false });
    } catch (e) {
      console.error("Failed to load providers:", e);
      set({ loading: false });
    }
  },

  saveProvider: async (provider: Record<string, unknown>) => {
    await dispatchProviderSaved(provider);
  },

  deleteProvider: async (id: string) => {
    await dispatchProviderDeleted(id);
  },

  fetchModels: async (providerId: string) => {
    const result = await invoke("list_models_cmd", { providerId });
    return (result as ModelConfig[]) || [];
  },
}));
