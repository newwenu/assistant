import type { Provider } from "@/settings/provider/types";

export interface ProviderModelRef {
  providerId: string | null;
  modelId: string;
}

/** provider 声明的默认模型；缺失时退回第一个模型，两者都没有则为空串 */
export function resolveDefaultModel(provider: Provider): string {
  if (provider.defaultModel) return provider.defaultModel;
  return provider.models[0]?.id ?? "";
}

/**
 * 把可能失效的 provider / model 组合修正为可用组合：
 * provider 不存在时退回第一个 provider，模型不存在时退回该 provider 的默认模型。
 */
export function resolveProviderModel(
  providers: Provider[],
  providerId: string | null,
  modelId: string,
): ProviderModelRef {
  if (providers.length === 0) {
    return { providerId: null, modelId: "" };
  }
  const provider = providers.find((p) => p.id === providerId);
  if (!provider) {
    const fallback = providers[0];
    return { providerId: fallback.id, modelId: resolveDefaultModel(fallback) };
  }
  const modelExists = provider.models.some((m) => m.id === modelId);
  if (!modelExists) {
    return { providerId: provider.id, modelId: resolveDefaultModel(provider) };
  }
  return { providerId: provider.id, modelId };
}
