import type { FeatureModule } from "../types";

export const timekeeperModule: FeatureModule = {
  id: "timekeeper",
  ipc: { namespace: "timekeeper" },
};
