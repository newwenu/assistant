export interface ModelConfig {
  id: string;
  name: string;
  ownedBy: string;
  contextWindow?: number;
  supportVision?: boolean;
  /** 输入模态列表（"text"/"image"/"pdf"/"audio"/"video"）；存量数据可能缺失 */
  inputModalities?: string[];
}

export interface Provider {
  id: string;
  name: string;
  type: string;
  baseUrl: string;
  apiKeyMasked: string;
  models: ModelConfig[];
  defaultModel: string;
  updatedAt: number;
}

export interface ProviderInput {
  id: string;
  name: string;
  type: string;
  baseUrl: string;
  apiKey: string;
  apiKeyChanged?: boolean;
  models: string;
  defaultModel: string;
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
