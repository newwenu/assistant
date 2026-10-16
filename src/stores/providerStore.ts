import { create } from "zustand";
import { providerService } from "../services/providerService";
import {
  dispatchProviderSaved,
  dispatchProviderDeleted,
} from "../settings/core/settingsDispatcher";
import {
  getAllProviderFormats,
  getProviderFormat,
  providerFormatNeedsApiKey,
} from "../settings/core/provider/providerFormatRegistry";
import {
  getProviderPreset,
  getAllProviderPresets,
  getApiFormatByType as getApiFormatByTypeFromRegistry,
} from "../settings/core/provider/providerPresetRegistry";
import "../settings/core/provider/providerDefinitions";
export type {
  ModelConfig,
  Provider,
  ProviderInput,
  ApiFormat,
  ApiFormatInfo,
  ProviderPreset,
} from "@/settings/provider/types";
import type {
  ModelConfig,
  Provider,
  ProviderInput,
  ApiFormat,
  ApiFormatInfo,
  ProviderPreset,
} from "@/settings/provider/types";

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
  return getApiFormatByTypeFromRegistry(type) as ApiFormat;
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

interface ProviderState {
  providers: Provider[];
  loading: boolean;

  loadProviders: () => Promise<void>;
  saveProvider: (provider: ProviderInput) => Promise<void>;
  deleteProvider: (id: string) => Promise<void>;
  fetchModels: (providerId: string) => Promise<ModelConfig[]>;
}

export const useProviderStore = create<ProviderState>((set) => ({
  providers: [],
  loading: false,

  loadProviders: async () => {
    set({ loading: true });
    try {
      const providers = await providerService.listProviders();
      set({ providers: providers || [], loading: false });
    } catch (e) {
      console.error("Failed to load providers:", e);
      set({ loading: false });
    }
  },

  saveProvider: async (provider: ProviderInput) => {
    await dispatchProviderSaved(provider);
  },

  deleteProvider: async (id: string) => {
    await dispatchProviderDeleted(id);
  },

  fetchModels: async (providerId: string) => {
    const models = await providerService.listModels(providerId);
    return models || [];
  },
}));
