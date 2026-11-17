import type {
  ModelConfigDto,
  ProviderInputDto,
  ProviderListDto,
  ReasoningLevelsDto,
} from "../../types/protocol.contract";

/** 目录声明的推理可选项；缺失表示目录没有可用信息 */
export type ReasoningLevels = ReasoningLevelsDto;

export type ModelConfig = ModelConfigDto;

export type Provider = ProviderListDto;

export type ProviderInput = ProviderInputDto;

export type ApiFormat =
  "openai" | "responses" | "anthropic" | "gemini" | "local";

export interface ApiFormatInfo {
  format: ApiFormat;
  label: string;
  description: string;
  needsApiKey: boolean;
  keyPlaceholder?: string;
  customType: string;
}

export interface ProviderPreset {
  slug: string;
  label: string;
  format: ApiFormat;
  defaultBaseUrl: string;
}
