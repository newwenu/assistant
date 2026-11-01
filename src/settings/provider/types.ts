/** 目录声明的推理可选项；缺失表示目录没有可用信息 */
export interface ReasoningLevels {
  toggle: boolean;
  efforts: string[];
}

export interface ModelConfig {
  id: string;
  name: string;
  ownedBy: string;
  contextWindow?: number;
  supportVision?: boolean;
  /** 输入模态列表（"text"/"image"/"pdf"/"audio"/"video"）；存量数据可能缺失 */
  inputModalities?: string[];
  /** 推理档位；存量数据可能缺失 */
  reasoningLevels?: ReasoningLevels;
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
