import { aggregate } from "./aggregate";
import type { AggregatedFeatures } from "./aggregate";
import type { FeatureModule } from "./types";
import { bindExtensionInput } from "@/settings/core/runtime/snapshot";
import { EMPTY_EXTENSION_INPUT } from "@/settings/core/types/snapshot";
import { registerConfSlot } from "@/settings/core/runtime/confRuntime";

let features: AggregatedFeatures = aggregate([]);

bindExtensionInput(EMPTY_EXTENSION_INPUT);

export function getFeatures(): AggregatedFeatures {
  return features;
}

export async function loadFeatures(): Promise<void> {
  const modules: FeatureModule[] = [];
  if (__VAULT_ENABLED__) {
    const { vaultModule } = await import("./vault/module");
    modules.push(vaultModule);
  }
  if (__TIMEKEEPER_ENABLED__) {
    const { timekeeperModule } = await import("./timekeeper/module");
    modules.push(timekeeperModule);
  }
  if (__SERIAL_DEBUG_ENABLED__) {
    const { serialDebugModule } = await import("./serial_debug/module");
    modules.push(serialDebugModule);
  }
  const { observationModule } = await import("./observation/module");
  modules.push(observationModule);
  const { orchestrationModule } = await import("./orchestration/module");
  modules.push(orchestrationModule);
  if (modules.length === 0) return;
  features = aggregate(modules);
  for (const slot of features.confs) registerConfSlot(slot);
  bindExtensionInput({ id: modules.map((m) => m.id).join("+"), ...features });
}
