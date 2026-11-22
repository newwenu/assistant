import type { FeatureModule } from "../types";

export const clockModule: FeatureModule = {
  id: "clock",
  ipc: { namespace: "clock" },
};
