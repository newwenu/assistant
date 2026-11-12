import { tauriInvoke } from "@/services/tauriInvoke";
import { pluginCommand } from "@/services/featureIpc";
import { createLogger } from "@/services/logger";
import type { ConfGroup, ConfOf } from "./types/confRegistry";
import type { ConfSlot } from "./types/confSlot";

const log = createLogger("confRuntime");

const slots = new Map<ConfGroup, ConfSlot>();

export function registerConfSlot<G extends ConfGroup>(slot: ConfSlot<G>): void {
  if (slots.has(slot.group)) {
    if (import.meta.env.DEV) {
      throw new Error(`[conf] duplicate conf slot: ${slot.group}`);
    }
    log.error("duplicate conf slot, ignored", { group: slot.group });
    return;
  }
  slots.set(slot.group, slot);
}

export function getConfSlot(group: ConfGroup): ConfSlot | undefined {
  return slots.get(group);
}

export function isConfGroupAvailable(value: string): value is ConfGroup {
  return slots.has(value as ConfGroup);
}

export function getConfGroups(): readonly ConfGroup[] {
  return [...slots.keys()];
}

export async function fetchConf<G extends ConfGroup>(
  group: G,
): Promise<ConfOf<G> | null> {
  const slot = slots.get(group);
  if (!slot) return null;
  const command = slot.namespace
    ? pluginCommand(slot.namespace, slot.invoke)
    : slot.invoke;
  const raw = await tauriInvoke<ConfOf<G>>(command);
  return raw ?? null;
}
