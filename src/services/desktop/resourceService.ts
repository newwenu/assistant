import type {
  ProcessSnapshotDto,
  ResourceCapabilitiesDto,
} from "@/types/protocol.contract";
import { tauriInvoke as invoke } from "../system/tauriInvoke";

export const resourceService = {
  getResourceCapabilities: (): Promise<ResourceCapabilitiesDto> =>
    invoke<ResourceCapabilitiesDto>("get_resource_capabilities"),

  getProcessSnapshot: (): Promise<ProcessSnapshotDto> =>
    invoke<ProcessSnapshotDto>("get_process_snapshot"),
};
