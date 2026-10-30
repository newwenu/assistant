import { aggregate } from "./aggregate";
import type { AggregatedFeatures } from "./aggregate";

let features: AggregatedFeatures = aggregate([]);

export function getFeatures(): AggregatedFeatures {
  return features;
}

export async function loadFeatures(): Promise<void> {
  if (!__VAULT_ENABLED__) return;
  const { vaultModule } = await import("./vault/module");
  features = aggregate([vaultModule]);
}
