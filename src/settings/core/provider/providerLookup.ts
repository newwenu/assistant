import type {
  ApiFormat,
  ApiFormatInfo,
  ProviderPreset,
} from "@/settings/provider/types";
import {
  getAllProviderFormats,
  getProviderFormat,
  getProviderFormatByCustomType,
  providerFormatNeedsApiKey,
} from "./providerFormatRegistry";
import {
  getProviderPreset,
  getAllProviderPresets,
} from "./providerPresetRegistry";
import { createLogger } from "@/services/system/logger";
import "./providerDefinitions";

const log = createLogger("providerLookup");

export const API_FORMATS: ApiFormatInfo[] = getAllProviderFormats().map(
  (f) => ({
    format: f.id as ApiFormat,
    label: f.label,
    description: f.description,
    needsApiKey: f.needsApiKey,
    keyPlaceholder: f.keyPlaceholder,
    customType: f.customType,
  }),
);

export const API_FORMAT_MAP = new Map<ApiFormat, ApiFormatInfo>(
  API_FORMATS.map((f) => [f.format, f]),
);

export const PROVIDER_PRESETS: ProviderPreset[] = getAllProviderPresets().map(
  (p) => ({
    slug: p.slug,
    label: p.label,
    format: p.format as ApiFormat,
    defaultBaseUrl: p.defaultBaseUrl,
  }),
);

export const PRESET_MAP = new Map<string, ProviderPreset>(
  PROVIDER_PRESETS.map((p) => [p.slug, p]),
);

export function getPresetByType(type: string): ProviderPreset | undefined {
  const def = getProviderPreset(type);
  if (!def) return undefined;
  return {
    slug: def.slug,
    label: def.label,
    format: def.format as ApiFormat,
    defaultBaseUrl: def.defaultBaseUrl,
  };
}

export function getApiFormatByType(type: string): ApiFormat {
  const preset = PRESET_MAP.get(type);
  if (preset) return preset.format;
  const byCustomType = getProviderFormatByCustomType(type);
  if (byCustomType) return byCustomType.id as ApiFormat;
  log.warn("unknown provider type, falling back to default api format", {
    type,
  });
  return "openai";
}

/** 自定义 Provider 落库时的 type 值；保证 format 与后端 kind 分派一致 */
export function getFormatCustomType(format: string): string {
  return getProviderFormat(format)?.customType ?? "custom";
}

export function getFormatInfo(format: ApiFormat): ApiFormatInfo | undefined {
  const def = getProviderFormat(format);
  if (!def) return undefined;
  return {
    format: def.id as ApiFormat,
    label: def.label,
    description: def.description,
    needsApiKey: def.needsApiKey,
    keyPlaceholder: def.keyPlaceholder,
    customType: def.customType,
  };
}

export function formatNeedsApiKey(format: string): boolean {
  return providerFormatNeedsApiKey(format);
}
