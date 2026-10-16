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
  models: ModelConfig[];
  default_model: string;
  updated_at: number;
}

export interface ProviderInput {
  id: string;
  name: string;
  type: string;
  base_url: string;
  api_key: string;
  api_key_changed?: boolean;
  models: string;
  default_model: string;
}

export type ApiFormat = "openai" | "anthropic" | "local";

export interface ApiFormatInfo {
  format: ApiFormat;
  label: string;
  description: string;
  needsApiKey: boolean;
  keyPlaceholder?: string;
}

export interface ProviderPreset {
  slug: string;
  label: string;
  format: ApiFormat;
  defaultBaseUrl: string;
}
