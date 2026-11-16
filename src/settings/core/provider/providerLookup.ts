import type {
  ApiFormat,
  ApiFormatInfo,
  ProviderPreset,
} from "@/settings/provider/types";
import {
  getAllProviderFormats,
  getProviderFormat,
  providerFormatNeedsApiKey,
} from "./providerFormatRegistry";
import {
  getProviderPreset,
  getAllProviderPresets,
  getApiFormatByType as getApiFormatByTypeFromRegistry,
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
  const format = getApiFormatByTypeFromRegistry(type) as ApiFormat;
  if (!PRESET_MAP.has(type)) {
    log.warn("unknown provider type, falling back to default api format", {
      type,
      format,
    });
  }
  return format;
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
  };
}

export function formatNeedsApiKey(format: string): boolean {
  return providerFormatNeedsApiKey(format);
}
