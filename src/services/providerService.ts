import { invoke } from "@tauri-apps/api/core";
import type {
  Provider,
  ProviderInput,
  ModelConfig,
} from "../settings/provider/types";

export const providerService = {
  listProviders: (): Promise<Provider[]> =>
    invoke<Provider[]>("list_providers").then((r) => r || []),

  listModels: (providerId: string): Promise<ModelConfig[]> =>
    invoke<ModelConfig[]>("list_models_cmd", { providerId }).then(
      (r) => r || [],
    ),

  previewModels: (
    providerType: string,
    baseUrl: string,
    apiKey: string,
  ): Promise<ModelConfig[]> =>
    invoke<ModelConfig[]>("preview_models_cmd", {
      providerType,
      baseUrl,
      apiKey,
    }).then((r) => r || []),

  saveProvider: (provider: ProviderInput): Promise<void> =>
    invoke("save_provider", { provider }),

  deleteProvider: (id: string): Promise<void> =>
    invoke("delete_provider", { id }),
};
