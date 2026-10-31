import type { ConfSlot } from "@/settings/core/types/confSlot";
import { featureCommand } from "@/services/featureIpc";

export interface ProbeConf {
  probeFlag: boolean;
  probeText: string;
}

declare module "@/settings/core/types/confRegistry" {
  interface ConfTypeRegistry {
    probe: ProbeConf;
  }
}

export const probeConfSlot = {
  group: "probe",
  invoke: featureCommand({ ipc: { namespace: "probe" } }, "get_probe_conf"),
  pageId: "probe",
} satisfies ConfSlot<"probe">;
