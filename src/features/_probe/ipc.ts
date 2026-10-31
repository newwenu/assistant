import { featureInvoke } from "@/services/featureIpc";

const invoke = featureInvoke({ ipc: { namespace: "probe" } });

export function probeEcho(payload: string): Promise<string> {
  return invoke<string>("probe_echo", { payload });
}
