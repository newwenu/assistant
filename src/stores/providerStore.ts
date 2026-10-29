import { create } from "zustand";
import { providerService } from "../services/providerService";
import { createLogger } from "@/services/logger";
import {
  dispatchProviderSaved,
  dispatchProviderDeleted,
} from "../settings/core/settingsDispatcher";
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
} from "@/settings/provider/types";

interface ProviderState {
  providers: Provider[];
  loading: boolean;

  loadProviders: () => Promise<void>;
  saveProvider: (provider: ProviderInput) => Promise<void>;
  deleteProvider: (id: string) => Promise<void>;
  fetchModels: (providerId: string) => Promise<ModelConfig[]>;
}

const log = createLogger("providerStore");

export const useProviderStore = create<ProviderState>((set) => ({
  providers: [],
  loading: false,

  loadProviders: async () => {
    set({ loading: true });
    try {
      const providers = await providerService.listProviders();
      set({ providers: providers || [], loading: false });
    } catch (e) {
      log.error("Failed to load providers", e);
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
