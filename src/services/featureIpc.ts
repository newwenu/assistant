import { tauriInvoke } from "./tauriInvoke";
import type { FeatureModule } from "@/features/types";

export function featureCommand(
  module: Pick<FeatureModule, "ipc">,
  command: string,
): string {
  return module.ipc ? `plugin:${module.ipc.namespace}|${command}` : command;
}

export function featureInvoke(module: Pick<FeatureModule, "ipc">) {
  return <T = void>(command: string, args?: Record<string, unknown>) =>
    tauriInvoke<T>(featureCommand(module, command), args);
}
