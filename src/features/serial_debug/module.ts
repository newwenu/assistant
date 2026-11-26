import type { FeatureModule } from "../types";

export const serialDebugModule: FeatureModule = {
  id: "serial_debug",
  ipc: { namespace: "serial-debug" },
};
